#![no_std]

use allocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator, PageAllocator};
use core::{
    alloc::Layout,
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

#[inline]
fn align_up(addr: usize, align: usize) -> usize {
    // align 必须是 2 的幂
    debug_assert!(align.is_power_of_two());
    (addr + align - 1) & !(align - 1)
}

// 向下对齐辅助函数
#[inline]
fn align_down(addr: usize, align: usize) -> usize {
    // align 必须是 2 的幂
    debug_assert!(align.is_power_of_two());
    addr & !(align - 1)
}

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area. (注意: 简单的 bump 分配器通常不这样释放)
/// For pages area, it will never be freed!
///
/// 使用 AtomicUsize 实现内部可变性，假设它可能被共享。
pub struct EarlyAllocator<const PAGE: usize> {
    start: AtomicUsize,
    end: AtomicUsize,
    b_pos: AtomicUsize, // 字节分配指针
    p_pos: AtomicUsize, // 页分配指针
    count: AtomicUsize, // 字节分配计数
}

impl<const PAGE: usize> EarlyAllocator<PAGE> {
    /// 创建一个新的、未初始化的 EarlyAllocator。
    pub const fn new() -> Self {
        Self {
            start: AtomicUsize::new(0),
            end: AtomicUsize::new(0),
            b_pos: AtomicUsize::new(0),
            p_pos: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    /// 检查分配器是否已初始化。
    fn is_initialized(&self) -> bool {
        self.start.load(Ordering::Relaxed) != 0 && self.end.load(Ordering::Relaxed) != 0
    }
}

impl<const PAGE: usize> BaseAllocator for EarlyAllocator<PAGE> {
    fn init(&mut self, start: usize, size: usize) {
        let end = start.checked_add(size).expect("Allocator range overflow");
        assert!(start < end, "start address must be less than end address");
        self.start.store(start, Ordering::Relaxed);
        self.end.store(end, Ordering::Relaxed);
        self.b_pos.store(start, Ordering::Relaxed);
        self.p_pos.store(end, Ordering::Relaxed);
        self.count.store(0, Ordering::Relaxed);
    }

    fn add_memory(&mut self, _start: usize, _size: usize) -> AllocResult {
        // 这个简单的分配器管理单个连续区域。
        // 不支持添加更多内存。
        Err(AllocError::InvalidParam)
    }
}

impl<const PAGE: usize> ByteAllocator for EarlyAllocator<PAGE> {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        if !self.is_initialized() {
            return Err(AllocError::InvalidParam);
        }
        if layout.size() == 0 {
            return Err(AllocError::InvalidParam);
        }

        if !layout.align().is_power_of_two() {
            return Err(AllocError::InvalidParam);
        }

        let mut current_b_pos = self.b_pos.load(Ordering::Relaxed);

        loop {
            let aligned_b_pos = align_up(current_b_pos, layout.align());
            let new_b_pos = aligned_b_pos.checked_add(layout.size());
            let current_p_pos = self.p_pos.load(Ordering::Relaxed);

            match new_b_pos {
                Some(next_b_pos) if next_b_pos <= current_p_pos => {
                    match self.b_pos.compare_exchange(
                        current_b_pos,
                        next_b_pos,
                        Ordering::SeqCst, // 使用 SeqCst 以确保安全
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => {
                            // 分配成功
                            self.count.fetch_add(1, Ordering::Relaxed);
                            return Ok(NonNull::new(aligned_b_pos as *mut u8).unwrap());
                        }
                        Err(actual_b_pos) => {
                            current_b_pos = actual_b_pos;
                        }
                    }
                }
                _ => {
                    return Err(AllocError::NoMemory);
                }
            }
        }
    }

    fn dealloc(&mut self, pos: NonNull<u8>, layout: Layout) {
        if !self.is_initialized() {
            return; // 或者 panic?
        }
        let prev_count = self.count.fetch_sub(1, Ordering::Relaxed);

        if prev_count == 0 {
            self.count.store(0, Ordering::Relaxed);
        }
    }

    fn total_bytes(&self) -> usize {
        if !self.is_initialized() {
            return 0;
        }
        self.end
            .load(Ordering::Relaxed)
            .saturating_sub(self.start.load(Ordering::Relaxed))
    }

    fn used_bytes(&self) -> usize {
        if !self.is_initialized() {
            return 0;
        }
        self.b_pos
            .load(Ordering::Relaxed)
            .saturating_sub(self.start.load(Ordering::Relaxed))
    }

    fn available_bytes(&self) -> usize {
        if !self.is_initialized() {
            return 0;
        }
        let b = self.b_pos.load(Ordering::Relaxed);
        let p = self.p_pos.load(Ordering::Relaxed);
        p.saturating_sub(b) // 使用 saturating_sub 防止下溢
    }
}

impl<const PAGE: usize> PageAllocator for EarlyAllocator<PAGE> {
    const PAGE_SIZE: usize = PAGE;

    fn alloc_pages(&mut self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        if !self.is_initialized() {
            return Err(AllocError::InvalidParam);
        }
        if num_pages == 0 {
            return Err(AllocError::InvalidParam);
        }

        // 计算对齐值，必须是 2 的幂
        let align = 1usize
            .checked_shl(align_pow2 as u32)
            .ok_or(AllocError::InvalidParam)?;
        // 确保对齐至少是 PAGE_SIZE 且是 2 的幂
        let align = align.max(Self::PAGE_SIZE);
        if !align.is_power_of_two() {
            return Err(AllocError::InvalidParam);
        }

        let size = num_pages
            .checked_mul(Self::PAGE_SIZE)
            .ok_or(AllocError::NoMemory)?; // 检查溢出

        let mut current_p_pos = self.p_pos.load(Ordering::Relaxed);
        let start_limit = self.start.load(Ordering::Relaxed); // 获取内存区域的开始边界

        loop {
            let potential_start = current_p_pos.checked_sub(size);
            let current_b_pos = self.b_pos.load(Ordering::Relaxed); // 在检查前获取最新的 b_pos

            match potential_start {
                Some(start_addr) => {
                    let aligned_start = align_down(start_addr, align);

                    if aligned_start >= current_b_pos && aligned_start >= start_limit {
                        match self.p_pos.compare_exchange(
                            current_p_pos,
                            aligned_start,
                            Ordering::SeqCst,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => {
                                return Ok(aligned_start);
                            }
                            Err(actual_p_pos) => {
                                current_p_pos = actual_p_pos;
                            }
                        }
                    } else {
                        return Err(AllocError::NoMemory);
                    }
                }
                None => {
                    // 减法下溢
                    return Err(AllocError::NoMemory);
                }
            }
        }
    }

    fn dealloc_pages(&mut self, pos: usize, num_pages: usize) {
        // 此分配器中从不释放页分配。
    }

    fn total_pages(&self) -> usize {
        if !self.is_initialized() {
            return 0;
        }
        self.total_bytes() / Self::PAGE_SIZE
    }

    fn used_pages(&self) -> usize {
        if !self.is_initialized() {
            return 0;
        }
        self.end
            .load(Ordering::Relaxed)
            .saturating_sub(self.p_pos.load(Ordering::Relaxed))
            / Self::PAGE_SIZE
    }

    fn available_pages(&self) -> usize {
        if !self.is_initialized() {
            return 0;
        }
        // 可用空间向下对齐到页面大小
        align_down(self.available_bytes(), Self::PAGE_SIZE) / Self::PAGE_SIZE
    }
}
