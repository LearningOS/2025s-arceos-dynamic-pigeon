//! Allocator algorithm in lab.

#![no_std]
#![allow(unused_variables)]


use allocator::{BaseAllocator, ByteAllocator, AllocResult};
use core::ptr::NonNull;
use core::alloc::Layout;
use allocator::AllocError::NoMemory;

pub struct LabByteAllocator {
    pool_start: usize,
    even_start: usize,
    odd_start: usize,
    cur_loop: usize,
    size: usize,
}

impl LabByteAllocator {
    pub const fn new() -> Self {
        Self {
            pool_start: 0,
            even_start: 0,
            odd_start: 0,
            cur_loop: 0,
            size: 0,
        }
    }

    fn next_loop(&mut self, size: usize) -> usize {
        let lop = size - self.cur_loop;
        if lop.is_power_of_two() {
            return self.cur_loop;
        }
        self.cur_loop += 1;
        self.cur_loop
    }
}

impl BaseAllocator for LabByteAllocator {
    fn init(&mut self, start: usize, size: usize) {
        self.pool_start = start;
        self.even_start = start + size / 2;
        self.odd_start = start + size / 2;
        self.size = size;
    }
    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        self.size += size;
        Ok(())
    }
}

impl ByteAllocator for LabByteAllocator {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        if layout.align() == 8 {
            let s = self.pool_start;
            if s > self.odd_start {
                return Err(NoMemory);
            }
            return Ok(NonNull::new(s as *mut u8).unwrap());
        }
        let pref = self.cur_loop;
        let lop = self.next_loop(layout.size());
        if pref != lop {
            self.even_start = self.pool_start + self.size / 2;
            self.odd_start = self.even_start;
        }
        if lop >= 3_00000 {
            return Err(NoMemory);
        }
        let size = layout.size() - lop;
        let st = if size == 0 {0} else {size.ilog2()};
        let s = if lop % 2 == 0 {
            let ret = self.even_start;
            if ret > self.pool_start + self.size {
                return Err(NoMemory);
            }
            self.even_start += size;
            ret
        } else {
            self.odd_start
        };
        Ok(NonNull::new(s as *mut u8).unwrap())
    }
    fn dealloc(&mut self, pos: NonNull<u8>, layout: Layout) {
    }
    fn total_bytes(&self) -> usize {
        self.size
    }
    fn used_bytes(&self) -> usize {
        unimplemented!();
    }
    fn available_bytes(&self) -> usize {
        unimplemented!();
    }
}