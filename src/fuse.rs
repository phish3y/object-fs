use std::{
    ffi::OsStr,
    time::{Duration, SystemTime},
};

use fuser::{
    FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, ReplyWrite, Request,
};
use libc::{EIO, ENOENT};

use crate::{fs, model};

const TTL: Duration = Duration::new(0, 0);
const KEEP_FILE: &str = ".keep";

impl Filesystem for fs::ObjectFS {
    fn init(
        &mut self,
        _req: &Request<'_>,
        _config: &mut fuser::KernelConfig,
    ) -> Result<(), libc::c_int> {
        log::info!("`init` called");

        let mut lock_ino_to_node = match self.ino_to_node.lock() {
            Err(err) => {
                log::error!("`init` failed to acquire `ino_to_node` guard: {}", err);
                return Err(-1);
            }
            Ok(guard) => guard,
        };

        let res = self.client.fs_put_object(&self.bucket, KEEP_FILE, None);

        match res {
            Err(err) => {
                log::error!("`init` failed to put_object: {}", err);
                return Err(-1);
            }
            Ok(_) => {}
        }

        let prefix = "";
        let res = self.client.fs_list_objects(&self.bucket, prefix);

        let objects = match res {
            Err(err) => {
                log::error!("`init` failed to list_objects at: {}", err);
                return Err(-1);
            }
            Ok(objects) => objects,
        };

        for obj in objects {
            let key = if obj.key.ends_with('/') {
                let key = format!("{}{}", obj.key, KEEP_FILE);
                let res = self.client.fs_put_object(&self.bucket, &key, None);

                match res {
                    Err(err) => {
                        log::error!("`init` failed to put_object: {}", err);
                        return Err(-1);
                    }
                    Ok(_) => (),
                };

                key
            } else {
                obj.key
            };

            self.index_object(
                &mut lock_ino_to_node,
                &model::fs::FSObject {
                    key,
                    size: obj.size,
                    modified_time: obj.modified_time,
                },
            );
        }

        return Ok(());
    }

    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        log::debug!("`lookup` parent: {}, name: {:?}", parent, name);

        let lock_ino_to_node = match self.ino_to_node.lock() {
            Err(err) => {
                log::error!("`lookup` failed to acquire `ino_to_node` guard: {}", err);
                reply.error(EIO);
                return;
            }
            Ok(guard) => guard,
        };

        if parent == self.get_root_attr(&lock_ino_to_node).ino && name == "/" {
            reply.entry(&TTL, &self.get_root_attr(&lock_ino_to_node), 0);
            return;
        }

        let parent_node = match lock_ino_to_node.get(&parent) {
            None => {
                log::error!("`lookup` failed to find parent ino: {}", parent);
                reply.error(ENOENT);
                return;
            }
            Some(pn) => pn,
        };

        let key = if parent_node.attr.ino == self.get_root_attr(&lock_ino_to_node).ino {
            name.to_string_lossy().to_string()
        } else {
            format!("{}/{}", parent_node.key, name.to_string_lossy())
        };

        let mut attr = None;
        for child in self.get_children(&lock_ino_to_node, parent) {
            if child.key == key {
                attr = Some(child.attr);
            }
        }

        if let Some(a) = attr {
            reply.entry(&TTL, &a, 0);
        } else {
            reply.error(ENOENT);
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, fh: Option<u64>, reply: ReplyAttr) {
        log::debug!("`getattr` ino: {}, fh: {}", ino, fh.unwrap_or(0));

        let lock_ino_to_node = match self.ino_to_node.lock() {
            Err(err) => {
                log::error!("`getattr` failed to acquire `ino_to_node` guard: {}", err);
                reply.error(EIO);
                return;
            }
            Ok(guard) => guard,
        };

        if ino == self.get_root_attr(&lock_ino_to_node).ino {
            reply.attr(&TTL, &self.get_root_attr(&lock_ino_to_node));
            return;
        }

        let node = match lock_ino_to_node.get(&ino) {
            None => {
                log::error!("`getattr` failed to find ino: {}", ino);
                reply.error(ENOENT);
                return;
            }
            Some(n) => n,
        };

        log::debug!(
            "`getattr` parent: {}, ino: {}, key: {}",
            node.parent,
            node.attr.ino,
            node.key
        );
        reply.attr(&TTL, &node.attr);
    }

    fn mknod(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        _rdev: u32,
        reply: ReplyEntry,
    ) {
        log::debug!(
            "`mknod` parent: {}, name: {:?}, mode: {}",
            parent,
            name,
            mode
        );

        if (mode & libc::S_IFMT) != libc::S_IFREG {
            reply.error(libc::EOPNOTSUPP);
            return;
        }

        let mut lock_ino_to_node = match self.ino_to_node.lock() {
            Err(err) => {
                log::error!("`mknod` failed to acquire `ino_to_node` guard: {}", err);
                reply.error(EIO);
                return;
            }
            Ok(guard) => guard,
        };

        let parent_node = match lock_ino_to_node.get(&parent) {
            None => {
                log::error!("`mknod`failed to find parent ino: {}", parent);
                reply.error(ENOENT);
                return;
            }
            Some(pn) => pn,
        };

        let key = if parent_node.attr.ino == self.get_root_attr(&lock_ino_to_node).ino {
            name.to_string_lossy().to_string()
        } else {
            format!("{}/{}", parent_node.key, name.to_string_lossy())
        };

        match self.client.fs_put_object(&self.bucket, &key, None) {
            Err(err) => {
                log::error!("`mknod` failed to put_object: {}", err);
                reply.error(EIO);
                return;
            }
            Ok(_) => (),
        }

        let new_node = self.index_file(
            &mut lock_ino_to_node,
            &model::fs::FSObject {
                key,
                size: 0,
                modified_time: SystemTime::now(),
            },
            parent,
        );

        reply.entry(&TTL, &new_node.attr, 0);
    }

    fn mkdir(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        umask: u32,
        reply: ReplyEntry,
    ) {
        log::debug!(
            "`mkdir` parent: {}, name: {:?}, mode: {}, umask: {}",
            parent,
            name,
            mode,
            umask
        );

        let mut lock_ino_to_node = match self.ino_to_node.lock() {
            Err(err) => {
                log::error!("`mkdir` failed to acquire `ino_to_node` guard: {}", err);
                reply.error(EIO);
                return;
            }
            Ok(guard) => guard,
        };

        let parent_node = match lock_ino_to_node.get(&parent) {
            None => {
                log::error!("`mkdir` failed to find parent ino: {}", parent);
                reply.error(ENOENT);
                return;
            }
            Some(pn) => pn,
        };

        let key = if parent_node.attr.ino == self.get_root_attr(&lock_ino_to_node).ino {
            format!("{}/{}", name.to_string_lossy(), KEEP_FILE)
        } else {
            format!(
                "{}/{}/{}",
                parent_node.key,
                name.to_string_lossy(),
                KEEP_FILE
            )
        };

        match self.client.fs_put_object(&self.bucket, &key, None) {
            Err(err) => {
                log::error!("`mkdir` failed to put_object: {}", err);
                reply.error(EIO);
                return;
            }
            Ok(_) => (),
        }

        let new_node = self.index_directory(
            &mut lock_ino_to_node,
            &model::fs::FSObject {
                key,
                size: 0,
                modified_time: SystemTime::now(),
            },
            parent,
        );

        reply.entry(&TTL, &new_node.attr, 0);
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        log::debug!(
            "`read` ino: {}, fh: {}, offset: {}, size: {}",
            ino,
            fh,
            offset,
            size
        );

        let lock_ino_to_node = match self.ino_to_node.lock() {
            Err(err) => {
                log::error!("`read` failed to acquire `ino_to_node` guard: {}", err);
                reply.error(EIO);
                return;
            }
            Ok(guard) => guard,
        };

        let node = match lock_ino_to_node.get(&ino) {
            None => {
                log::error!("`read` failed to find ino: {}", ino);
                reply.error(ENOENT);
                return;
            }
            Some(n) => n,
        };

        let maybe_bytes = match self.client.fs_download_object(
            &self.bucket,
            &node.key,
            Some((offset as u64, (offset as u64 + size as u64))),
        ) {
            Err(err) => {
                log::error!("`read` failed to download_object: {}", err);
                reply.error(EIO);
                return;
            }
            Ok(mb) => mb,
        };

        let bytes = match maybe_bytes {
            None => {
                log::warn!("`write` object not found: {}", node.key);
                reply.error(ENOENT);
                return;
            }
            Some(b) => b,
        };

        reply.data(&bytes)
    }

    fn write(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        log::debug!(
            "`write` ino: {}, fh: {}, offset: {}, len: {}",
            ino,
            fh,
            offset,
            data.len()
        );

        let mut lock_ino_to_node = match self.ino_to_node.lock() {
            Err(err) => {
                log::error!("`write` failed to acquire `ino_to_node` guard: {}", err);
                reply.error(EIO);
                return;
            }
            Ok(guard) => guard,
        };

        let mut node = match lock_ino_to_node.remove(&ino) {
            None => {
                log::error!("`write` failed to find ino: {}", ino);
                reply.error(ENOENT);
                return;
            }
            Some(n) => n,
        };

        let maybe_bytes = match self
            .client
            .fs_download_object(&self.bucket, &node.key, None)
        {
            Err(err) => {
                log::error!("`write` failed to download_object: {}", err);
                reply.error(EIO);
                return;
            }
            Ok(mb) => mb,
        };

        let mut bytes = match maybe_bytes {
            None => {
                log::warn!("`write` object not found: {}", node.key);
                reply.error(ENOENT);
                return;
            }
            Some(b) => b,
        };

        let end_offset = offset as usize + data.len();
        if end_offset > bytes.len() {
            bytes.resize(end_offset, 0);
        }
        bytes[offset as usize..end_offset].copy_from_slice(data);

        match self
            .client
            .fs_put_object(&self.bucket, &node.key, Some(bytes))
        {
            Err(err) => {
                log::error!("`write` failed to put_object: {}", err);
                reply.error(EIO);
                return;
            }
            Ok(_) => (),
        }

        node.attr.size = data.len() as u64;
        lock_ino_to_node.insert(node.attr.ino, node.clone());

        reply.written(data.len() as u32);
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        log::debug!("`readdir` ino: {}, fh: {}, offset: {}", ino, fh, offset);
        let lock_ino_to_node = match self.ino_to_node.lock() {
            Err(err) => {
                log::error!("`readdir` failed to acquire `ino_to_node` guard: {}", err);
                reply.error(EIO);
                return;
            }
            Ok(guard) => guard,
        };

        let mut entries = vec![
            (
                self.get_root_attr(&lock_ino_to_node).ino,
                FileType::Directory,
                ".".to_string(),
            ),
            (
                self.get_root_attr(&lock_ino_to_node).ino,
                FileType::Directory,
                "..".to_string(),
            ),
        ];

        for child in self.get_children(&lock_ino_to_node, ino) {
            entries.push((child.attr.ino, child.attr.kind, child.name));
        }

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            let next_offset = (i + 1) as i64;
            if reply.add(entry.0, next_offset, entry.1, entry.2.clone()) {
                break;
            }
        }

        reply.ok();
    }
}
