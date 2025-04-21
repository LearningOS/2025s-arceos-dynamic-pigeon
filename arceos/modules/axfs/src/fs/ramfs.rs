use alloc::sync::Arc;
use axfs_ramfs::DirNode;
use axfs_vfs::{VfsError, VfsNodeOps, VfsNodeRef, VfsNodeType, VfsOps, VfsResult};
use alloc::vec;

pub use axfs_ramfs::RamFileSystem;

use crate::root::{create_file, lookup};

pub struct RamFs {
    file_sys: RamFileSystem,
}

struct DirWrapper {
    dir: Arc<dyn VfsNodeOps>,
}

impl RamFs {
    pub fn new() -> Self {
        let file_sys = RamFileSystem::new();
        let myfs = RamFs { file_sys };
        myfs
    }
}

impl VfsOps for RamFs {
    fn mount(&self, path: &str, mount_point: VfsNodeRef) -> VfsResult {
        self.file_sys.mount(path, mount_point)
    }

    fn root_dir(&self) -> VfsNodeRef {
        Arc::new(DirWrapper {
            dir: self.file_sys.root_dir(),
        })
    }
}
impl VfsNodeOps for DirWrapper {
    fn lookup(self: Arc<Self>, path: &str) -> VfsResult<VfsNodeRef> {
        let node = self.dir.clone().lookup(path)?;
        if node.get_attr()?.is_dir() {
            return Ok(Arc::new(DirWrapper { dir: node }));
        }
        Ok(node)
    }

    fn create(&self, path: &str, ty: VfsNodeType) -> VfsResult {
        self.dir.create(path, ty)
    }
    fn rename(&self, src_path: &str, dst_path: &str) -> VfsResult {
        let src = self.dir.clone().lookup(src_path)?;
        let dst = create_file(None, dst_path)?;
        self.dir.remove(src_path)?;
        let len = src.get_attr()?.size() as usize;
        let mut buf = vec![0u8; len];
        src.read_at(0, &mut buf)?;
        dst.write_at(0, &buf)?;
        Ok(())
    }
}
