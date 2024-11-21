use clap::{Arg, Command};
use fuser::{
    FileAttr, 
    Filesystem, 
    MountOption, 
    Request
};
use std::{
    collections::HashMap, sync::Mutex, time::{Duration, SystemTime}
};

mod adapters;
mod model;
mod util;

const TTL: Duration = Duration::new(0, 0); 
const ROOT_INO: u64 = 1;
const KEEP_FILE: &str = ".keep";

struct ObjectFS<'a> {
    client: &'a dyn adapters::adapter::ObjectAdapter,
    bucket: String,
    current_ino: Mutex<u64>,
    ino_to_node: Mutex<HashMap<u64, model::fs::FSNode>>,
    key_to_node: Mutex<HashMap<String, model::fs::FSNode>>
}

impl<'a> ObjectFS<'a> {

    fn new(client: &'a dyn adapters::adapter::ObjectAdapter, bucket: &str) -> Self {
        let mut ino_to_node = HashMap::new();
        let root_node = model::fs::FSNode {
            attr: FileAttr {
                ino: ROOT_INO,
                size: 0,
                blksize: 0,
                blocks: 0,
                atime: SystemTime::now(),
                mtime: SystemTime::now(),
                ctime: SystemTime::now(),
                crtime: SystemTime::now(),
                kind: fuser::FileType::Directory,
                perm: 0o755,
                nlink: 2,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
            },
            key: "".to_string(),
            parent: 0
        };

        ino_to_node.insert(ROOT_INO, root_node.clone());
        
        let mut key_to_node = HashMap::new();
        key_to_node.insert("".to_string(), root_node);

        Self {
            client,
            bucket: bucket.to_string(),
            current_ino: ROOT_INO.into(),
            ino_to_node: Mutex::new(ino_to_node),
            key_to_node: Mutex::new(key_to_node)
        }
    }

    fn next_ino(&self) -> u64 {
        let mut cur_ino = self.current_ino
            .lock()
            .expect("failed to acquire `current_ino` guard");
        *cur_ino += 1;

        return *cur_ino;
    }

    fn index_object(
        &self, 
        object: &model::fs::FSObject
    ) {
        let mut components = Vec::new();
        let mut maybe_component = Some(object.key.clone());
        while let Some(component) = maybe_component {
            components.push(component.clone());
            maybe_component = self.get_parent(&component);
        }

        components.reverse();

        let mut parent_ino = ROOT_INO;
        for component in components {
            parent_ino = if component == object.key {
                self.index_file(
                    &model::fs::FSObject {
                        key: component,
                        size: object.size,
                        modified_time: object.modified_time,
                    },
                    parent_ino
                )
            } else {
                self.index_directory(
                    &model::fs::FSObject {
                        key: component,
                        size: object.size,
                        modified_time: object.modified_time,
                    },
                    parent_ino
                )
            }
        }
    }

    fn index_file(
        &self, 
        object: &model::fs::FSObject, 
        parent: u64
    ) -> u64 {
        if self.key_to_node
            .lock()
            .expect("failed to acquire `key_to_node` guard")
            .get(&object.key).is_some() {
            return self.key_to_node
                .lock()
                .expect("failed to acquire `key_to_node` guard")
                .get(&object.key)
                .unwrap()
                .attr
                .ino;
        }

        let ino = self.next_ino();
        let node = model::fs::FSNode {
            attr: FileAttr {
                ino,
                size: object.size as u64,
                blksize: 0, // TODO
                blocks: 0, // TODO
                atime: object.modified_time,
                mtime: SystemTime::now(),
                ctime: SystemTime::now(),
                crtime: SystemTime::now(),
                kind: fuser::FileType::RegularFile,
                perm: 0o755,
                nlink: 1,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
            },
            key: object.key.clone(),
            parent
        };

        self.ino_to_node
            .lock()
            .expect("failed to acquire `ino_to_node` guard")
            .insert(ino, node.clone());

        self.key_to_node
            .lock()
            .expect("failed to acquire `key_to_node` guard")
            .insert(object.key.clone(), node);

        return ino;
    }

    fn index_directory(
        &self, 
        object: &model::fs::FSObject, 
        parent: u64
    ) -> u64 {
        let key = if object.key.ends_with('/') {
            &object.key[..object.key.len() - 1]
        } else {
            &object.key
        };

        if self.key_to_node
            .lock()
            .expect("failed to acquire `key_to_node` guard")
            .get(key).is_some() {
            return self.key_to_node
                .lock()
                .expect("failed to acquire `key_to_node` guard")
                .get(key)
                .unwrap()
                .attr
                .ino;
        }

        let ino = self.next_ino();
        let node = model::fs::FSNode {
            attr: FileAttr {
                ino,
                size: object.size as u64,
                blksize: 0, // TODO
                blocks: 0, // TODO
                atime: object.modified_time,
                mtime: SystemTime::now(),
                ctime: SystemTime::now(),
                crtime: SystemTime::now(),
                kind: fuser::FileType::Directory,
                perm: 0o755,
                nlink: 1,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
            },
            key: object.key.clone(),
            parent
        };

        self.ino_to_node
            .lock()
            .expect("failed to acquire `ino_to_node` lock")
            .insert(ino, node.clone());

        self.key_to_node
            .lock()
            .expect("failed to acquire `key_to_node` lock")
            .insert(object.key.clone(), node);

        return ino;
    }


    fn get_parent(&self, path: &str) -> Option<String> {
        let path = if path.ends_with('/') {
            &path[..path.len() - 1]
        } else {
            path
        };

        match path.rfind('/') {
            Some(pos) if pos > 0 => Some(path[..pos].to_string()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_ino() {
        let client = adapters::mock::MockS3Client{};
        let fs = ObjectFS::new(&client, "dummy-bucket");

        let cases = vec![
            2, 3
        ];

        for expected in cases {
            let result = fs.next_ino();
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_index_object() {
        let client = adapters::mock::MockS3Client{};
        let fs = ObjectFS::new(&client, "dummy-bucket");

        let cases = vec![
            ("file", 10, SystemTime::now(), 2),
            ("folder/file", 5, SystemTime::now(), 4),
            ("folder/subfolder/file", 5, SystemTime::now(), 6)
        ];

        for (key, size, modified_time, expected_count) in cases {
            
            fs.index_object(
                &model::fs::FSObject {
                    key: key.to_string(),
                    size,
                    modified_time
                }
            );
            let result = fs.key_to_node.lock().unwrap();
            
            assert_eq!(
                result.keys().len(), 
                expected_count, 
                "failed index count for case: {}", key
            );
        }
    }

    #[test]
    fn test_index_file() {
        let client = adapters::mock::MockS3Client{};

        let cases = vec![
            (
                "/file", 
                10, 
                SystemTime::now(),
                1
            ),
            (
                "folder/file", 
                0, 
                SystemTime::now(), 
                5
            ),
            (
                "folder/subfolder/file", 
                0, 
                SystemTime::UNIX_EPOCH, 
                7
            )
        ];

        for (key, size, modified_time, parent) in cases {
            let fs = ObjectFS::new(&client, "dummy-bucket");
            
            let ino = fs.index_file(
                &model::fs::FSObject {
                    key: key.to_string(),
                    size,
                    modified_time
                }, 
                parent
            );
            
            let guard = fs.ino_to_node
                .lock()
                .unwrap();
            let result = guard.get(&2)
                .unwrap();

            assert_eq!(
                ino, 
                2, 
                "failed on `ino` for case: {}", key
            );
            assert_eq!(
                result.attr.ino, 
                2, 
                "failed on `attr.ino` for case: {}", key
            );
            assert_eq!(
                result.parent, 
                parent, 
                "failed on `parent` for case: {}", key
            );
            assert_eq!(
                result.key, 
                key, 
                "failed on `key` for case: {}", 
                key
            );
            assert_eq!(
                result.attr.size, 
                size as u64, "failed on `attr.size` for case: {}", key
            );
            assert_eq!(
                result.attr.atime, 
                modified_time, 
                "failed on `attr.atime` for case: {}", 
                key
            );
        }
    }

    #[test]
    fn test_index_directory() {
        let client = adapters::mock::MockS3Client{};

        let cases = vec![
            (
                "folder", 
                SystemTime::UNIX_EPOCH,
                1
            ),
            (
                "folder/", 
                SystemTime::now(), 
                5
            ),
            (
                "folder/subfolder/", 
                SystemTime::UNIX_EPOCH, 
                7
            )
        ];

        for (key, modified_time, parent) in cases {
            let fs = ObjectFS::new(&client, "dummy-bucket");
            
            let ino = fs.index_directory(
                &model::fs::FSObject {
                    key: key.to_string(),
                    size: 0,
                    modified_time
                }, 
                parent
            );
            
            let guard = fs.ino_to_node
                .lock()
                .unwrap();
            let result = guard.get(&2)
                .unwrap();

            assert_eq!(
                ino, 
                2, 
                "failed on `ino` for case: {}", key
            );
            assert_eq!(
                result.attr.ino, 
                2, 
                "failed on `attr.ino` for case: {}", key
            );
            assert_eq!(
                result.parent, 
                parent, 
                "failed on `parent` for case: {}", key
            );
            assert_eq!(
                result.key, 
                key, 
                "failed on `key` for case: {}", key
            );
            assert_eq!(
                result.attr.atime, 
                modified_time, 
                "failed on `attr.atime` for case: {}", key
            );
        }
    }

    #[test]
    fn test_get_parent() {
        let client = adapters::mock::MockS3Client{};
        let fs = ObjectFS::new(&client, "dummy-bucket");

        let cases = vec![
            ("folder/file", Some("folder".to_string())),
            ("folder/subfolder/file", Some("folder/subfolder".to_string())),
            ("file", None),
            ("folder/", None),
        ];

        for (input, expected) in cases {
            let result = fs.get_parent(input);
            assert_eq!(result, expected, "failed for case: {}", input);
        }
    }
}

impl Filesystem for ObjectFS<'_> {

    fn init(
        &mut self, 
        _req: &Request<'_>, 
        _config: &mut fuser::KernelConfig
    ) -> Result<(), libc::c_int> {
        log::info!("`init` called");

        let res = self.client
            .fs_put_object(&self.bucket, KEEP_FILE);


        match res {
            Err(err) => {
                log::error!("`init` failed to put_object: {}", err);
                return Err(-1);
            }
            Ok(_) => {}
        }
        

        let prefix = "";
        let res = self.client
            .fs_list_objects(&self.bucket, prefix);

        let objects = match res {
            Err(err) => {
                log::error!("`init` failed to list_objects at: {}", err);
                return Err(-1);
            }
            Ok(objects) => objects
        };

        for obj in objects {
            let key = if obj.key.ends_with('/') {
                let key = format!("{}{}", obj.key, KEEP_FILE);
                let res = self.client.fs_put_object(&self.bucket, &key);

                match res {
                    Err(err) => {
                        log::error!("`init` failed to put_object: {}", err);
                        return Err(-1);
                    }
                    Ok(_) => ()
                };

                key
            } else {
                obj.key
            };

            self.index_object(&model::fs::FSObject {
                key,
                size: obj.size,
                modified_time: obj.modified_time
            });
        }
    
        return Ok(());
    }

    // fn lookup(
    //     &mut self, 
    //     _req: &Request, 
    //     parent: u64, 
    //     name: &OsStr, 
    //     reply: ReplyEntry
    // ) {
    //     log::debug!("`lookup` parent: {}, name: {:?}", parent, name);

    //     if parent == ROOT_INO && name == "/" {
    //         reply.entry(&TTL, &self.root_attr(), 0);
    //         return;
    //     }

    //     let mut lock_key_to_ino = self.key_to_ino
    //         .lock()
    //         .unwrap(); // TODO

    //     let lock_ino_to_key = self.ino_to_key
    //         .lock()
    //         .unwrap(); // TODO

    //     if let Some(key) = lock_ino_to_key.get(&parent) {
    //         // TODO only supports root
    //         let key = format!(
    //             "{}{}", 
    //             key, 
    //             name.to_string_lossy()
    //         );

    //         let head_res = tokio::task::block_in_place(|| {
    //             tokio::runtime::Handle::current().block_on(async {
    //                 self.client.head_object()
    //                     .bucket(&self.bucket)
    //                     .key(&key)
    //                     .send()
    //                     .await
    //             })
    //         });

    //         let head = match head_res {
    //             Err(err) => {   
    //                 if let Some(svc_err) = err.as_service_error() {
    //                     if svc_err.is_not_found() {
    //                         log::warn!("`lookup` not found: {}", &key);
    //                         reply.error(ENOENT);
    //                     } else {
    //                         log::error!("`lookup` failed to head_object: {}", err);
    //                         reply.error(EIO);
    //                     }
    //                 }

    //                 return;
    //             }
    //             Ok(head) => head
    //         };

    //         let ino = if let Some(ino) = lock_key_to_ino.get(&key) {
    //             *ino
    //         } else {
    //             let next_ino = self.next_ino();
    //             lock_key_to_ino.insert(key.clone(), next_ino);

    //             next_ino
    //         };

    //         let size = head.content_length().unwrap() as u64; // TODO
    //         let secs = head.last_modified
    //             .unwrap() // TODO
    //             .secs();
    //         let nanos = head.last_modified.
    //             unwrap() // TODO
    //             .subsec_nanos();
    //         let atime = SystemTime::UNIX_EPOCH + Duration::new(secs as u64, nanos);

    //         let fa = FileAttr {
    //             ino,
    //             size,
    //             blksize: 0,
    //             blocks: (size + 511) / 512,
    //             atime,
    //             mtime: SystemTime::now(),
    //             ctime: SystemTime::now(),
    //             crtime: SystemTime::now(),
    //             kind: fuser::FileType::RegularFile,
    //             perm: 0o755,
    //             nlink: 2,
    //             uid: 0,
    //             gid: 0,
    //             rdev: 0,
    //             flags: 0,
    //         };
            
    //         reply.entry(&TTL, &fa, 0); // TODO generation?
    //     } else {
    //         // TODO
    //         log::error!("`lookup` i should not yet be here");
    //         reply.error(ENOENT);
    //     }
    // }

    // fn getattr(
    //     &mut self, 
    //     _req: &Request, 
    //     ino: u64, 
    //     fh: Option<u64>, 
    //     reply: ReplyAttr
    // ) {
    //     log::debug!("`getattr` ino: {}, fh: {}", ino, fh.unwrap_or(0));

    //     if ino == ROOT_INO {
    //         reply.attr(&TTL, &self.root_attr());
    //         return;
    //     }

    //     let lock_ino_to_key = self.ino_to_key
    //         .lock() 
    //         .unwrap(); // TODO


    //     let mut lock_key_to_ino = self.key_to_ino
    //         .lock()
    //         .unwrap(); // TODO

    //     if let Some(key) = lock_ino_to_key.get(&ino) { 
    //         let head_res = tokio::task::block_in_place(|| {
    //             tokio::runtime::Handle::current().block_on(async {
    //                 self.client.head_object()
    //                     .bucket(&self.bucket)
    //                     .key(key) // TODO only supports root
    //                     .send()
    //                     .await
    //             })
    //         });

    //         let head = match head_res {
    //             Err(err) => {   
    //                 if let Some(svc_err) = err.as_service_error() {
    //                     if svc_err.is_not_found() {
    //                         log::warn!("`getattr` not found: {}", &key);
    //                         reply.error(ENOENT);
    //                     } else {
    //                         log::error!("`getattr` failed to head_object: {}", err);
    //                         reply.error(EIO);
    //                     }
    //                 }

    //                 return;
    //             }
    //             Ok(head) => head
    //         };

    //         let ino = if let Some(ino) = lock_key_to_ino.get(key) {
    //             *ino
    //         } else {
    //             let next_ino = self.next_ino();
    //             lock_key_to_ino.insert(key.clone(), next_ino);

    //             next_ino
    //         };

    //         let size = head.content_length().unwrap() as u64; // TODO
    //         let secs = head.last_modified
    //             .unwrap() // TODO
    //             .secs();
    //         let nanos = head.last_modified
    //             .unwrap() // TODO
    //             .subsec_nanos(); 
    //         let atime = SystemTime::UNIX_EPOCH + Duration::new(secs as u64, nanos);

    //         let fa = FileAttr {
    //             ino,
    //             size,
    //             blksize: 0,
    //             blocks: (size + 511) / 512,
    //             atime,
    //             mtime: SystemTime::now(),
    //             ctime: SystemTime::now(),
    //             crtime: SystemTime::now(),
    //             kind: fuser::FileType::RegularFile,
    //             perm: 0o755,
    //             nlink: 2,
    //             uid: 0,
    //             gid: 0,
    //             rdev: 0,
    //             flags: 0,
    //         };

    //         reply.attr(&TTL, &fa);
    //     } else {
    //         // TODO
    //         log::error!("`getattr` i should not yet be here");
    //         reply.error(ENOENT);
    //     }
    // }


    // fn mknod(
    //     &mut self,
    //     _req: &Request,
    //     parent: u64,
    //     name: &OsStr,
    //     mode: u32,
    //     _umask: u32,
    //     _rdev: u32,
    //     reply: ReplyEntry,
    // ) {
    //     log::debug!("`mknod` parent: {}, name: {:?}, mode: {}", parent, name, mode);

    //     if (mode & libc::S_IFMT) != libc::S_IFREG { // TODO support link
    //         reply.error(libc::EOPNOTSUPP);
    //         return;
    //     }

    //     let mut lock_ino_to_key = self.ino_to_key
    //         .lock()
    //         .unwrap(); // TODO

    //     let mut lock_key_to_ino = self.key_to_ino
    //         .lock()
    //         .unwrap(); // TODO

    //     let key = match lock_ino_to_key.get(&parent) {
    //         None => {
    //             // TODO shouldn't ever get here right now
    //             log::error!("`mknod` failed to find parent key for ino: {}", parent);
    //             reply.error(ENOENT);
    //             return;
    //         }
    //         Some(key) => key
    //     };

    //     let key = format!("{}{}", key, name.to_string_lossy()); // TODO only suppots root

    //     let put_res = tokio::task::block_in_place(|| {
    //         tokio::runtime::Handle::current().block_on(async {
    //             self.client.put_object()
    //                 .bucket(&self.bucket)
    //                 .key(&key)
    //                 .send()
    //                 .await
    //         })
    //     });

    //     match put_res {
    //         Err(err) => {
    //             log::error!("`mknod` failed to put_object: {}", err);
    //             reply.error(EIO);
    //             return;
    //         }
    //         Ok(_) => {
    //             let next_ino = self.next_ino();
    //             let fa = FileAttr {
    //                 ino: next_ino,
    //                 size: 0,
    //                 blksize: 0,
    //                 blocks: 0,
    //                 atime: SystemTime::now(),
    //                 mtime: SystemTime::now(),
    //                 ctime: SystemTime::now(),
    //                 crtime: SystemTime::now(),
    //                 kind: fuser::FileType::RegularFile,
    //                 perm: 0o755,
    //                 nlink: 2,
    //                 uid: 0,
    //                 gid: 0,
    //                 rdev: 0,
    //                 flags: 0,
    //             };

    //             lock_ino_to_key.insert(next_ino, key.clone());
    //             lock_key_to_ino.insert(key, next_ino);

    //             reply.entry(&TTL, &fa, 0);
    //         }
    //     }
    // }
    
    // fn mkdir(
    //     &mut self,
    //     _req: &Request<'_>,
    //     parent: u64,
    //     name: &OsStr,
    //     mode: u32,
    //     umask: u32,
    //     reply: ReplyEntry,
    // ) {
    //     log::debug!("`mkdir` parent: {}, name: {:?}, mode: {}, umask: {}", parent, name, mode, umask);

    //     let mut lock_ino_to_key = self.ino_to_key
    //         .lock() 
    //         .unwrap(); // TODO

        
    //     let mut lock_key_to_ino = self.key_to_ino
    //         .lock()
    //         .unwrap(); // TODO

    //     let key = match lock_ino_to_key.get(&parent) {
    //         Some(key) => key,
    //         None => {
    //             log::error!("`mkdir` no entry for ino: {}", parent);
    //             reply.error(ENOENT);
    //             return;
    //         }
    //     };

    //     let key = format!("{}{}/{}", key, name.to_string_lossy(), KEEP_FILE);

    //     let put_res = tokio::task::block_in_place(|| {
    //         tokio::runtime::Handle::current().block_on(async {
    //             self.client.put_object()
    //                 .bucket(&self.bucket)
    //                 .key(&key)
    //                 .send()
    //                 .await
    //         })
    //     });

    //     match put_res {
    //         Err(err) => {
    //             log::error!("`mkdir` failed to put_object: {}", err);
    //             reply.error(EIO);
    //             return;
    //         }
    //         Ok(_) => {
    //             let next_ino = self.next_ino();
    //             let fa = FileAttr {
    //                 ino: next_ino,
    //                 size: 0,
    //                 blksize: 0,
    //                 blocks: 0,
    //                 atime: SystemTime::now(),
    //                 mtime: SystemTime::now(),
    //                 ctime: SystemTime::now(),
    //                 crtime: SystemTime::now(),
    //                 kind: fuser::FileType::RegularFile,
    //                 perm: 0o755,
    //                 nlink: 2,
    //                 uid: 0,
    //                 gid: 0,
    //                 rdev: 0,
    //                 flags: 0,
    //             };

    //             lock_ino_to_key.insert(next_ino, key.clone());
    //             lock_key_to_ino.insert(key, next_ino);

    //             reply.entry(&TTL, &fa, 0);
    //         }
    //     }
    // }

    // fn read(
    //     &mut self,
    //     _req: &Request,
    //     ino: u64,
    //     fh: u64,
    //     offset: i64,
    //     size: u32,
    //     _flags: i32,
    //     _lock_owner: Option<u64>,
    //     reply: ReplyData,
    // ) {
    //     log::debug!("`read` ino: {}, fh: {}, offset: {}, size: {}", ino, fh, offset, size);

    //     let lock_ino_to_key = self.ino_to_key
    //         .lock() 
    //         .unwrap(); // TODO

    //     let key = match lock_ino_to_key.get(&ino) {
    //         Some(key) => key,
    //         None => {
    //             log::error!("`read` no entry for ino: {}", ino);
    //             reply.error(ENOENT);
    //             return;
    //         }
    //     };

    //     let range = format!("bytes={}-{}", offset, offset + size as i64 - 1);

    //     let buffer = tokio::task::block_in_place(|| {
    //         tokio::runtime::Handle::current().block_on(async {
    //             let get_res = self.client.get_object()
    //                 .bucket(&self.bucket)
    //                 .key(key)
    //                 .range(&range)
    //                 .send()
    //                 .await
    //                 .unwrap(); // TODO
                
    //             get_res.body
    //                 .collect()
    //                 .await
    //                 .unwrap() // TODO
    //         })
    //     });

    //     reply.data(&buffer.into_bytes())
    // }

    // fn write(
    //     &mut self,
    //     _req: &Request<'_>,
    //     ino: u64,
    //     fh: u64,
    //     offset: i64,
    //     data: &[u8],
    //     _write_flags: u32,
    //     _flags: i32,
    //     _lock_owner: Option<u64>,
    //     reply: ReplyWrite,
    // ) {
    //     log::debug!("`write` ino: {}, fh: {}, offset: {}, len: {}", ino, fh, offset, data.len());

    //     let lock_ino_to_key = self.ino_to_key
    //         .lock()
    //         .unwrap(); // TODO
    
    //     let key = match lock_ino_to_key.get(&ino) {
    //         None => {
    //             log::error!("`write` no entry for ino: {}", ino);
    //             reply.error(ENOENT);
    //             return;
    //         }
    //         Some(key) => key
    //     };

    //     let get_res = tokio::task::block_in_place(|| {
    //         tokio::runtime::Handle::current().block_on(async {
    //             self.client.get_object()
    //                 .bucket(&self.bucket)
    //                 .key(key)
    //                 .send()
    //                 .await
    //         })
    //     });
        
    //     let mut existing_data = match get_res {
    //         Err(err) => {   
    //             if let Some(svc_err) = err.as_service_error() {
    //                 if !svc_err.is_no_such_key() {
    //                     log::error!("`write` failed to get_object: {}", err);
    //                     reply.error(EIO);
    //                     return;
    //                 }
    //             }

    //             Vec::new()
    //         }
    //         Ok(get) => {
    //             tokio::task::block_in_place(|| {
    //                 tokio::runtime::Handle::current().block_on(async {
    //                     let body = get.body.collect().await;
    //                     body.map_or_else(
    //                         |err| {
    //                             log::warn!("`write` failed to collect body: {}", err);
    //                             Vec::new()
    //                         },
    //                         |data| data.to_vec()
    //                     )
    //                 })
    //             })
    //         }
    //     };

    //     let end_offset = offset as usize + data.len();
    //     if end_offset > existing_data.len() {
    //         existing_data.resize(end_offset, 0);
    //     }
    //     existing_data[offset as usize..end_offset].copy_from_slice(data);

    //     tokio::task::block_in_place(|| {
    //         tokio::runtime::Handle::current().block_on(async {
    //             self.client.put_object()
    //                 .bucket(&self.bucket)
    //                 .key(key)
    //                 .body(ByteStream::from(existing_data))
    //                 .send()
    //                 .await
    //                 .unwrap() // TODO
    //         })
    //     });

    //     reply.written(data.len() as u32);
    // }

    // fn readdir(
    //     &mut self,
    //     _req: &Request,
    //     ino: u64,
    //     fh: u64,
    //     offset: i64,
    //     mut reply: ReplyDirectory,
    // ) {
    //     log::debug!("`readdir` ino: {}, fh: {}, offset: {}", ino, fh, offset);

    //     let mut lock_key_to_ino = self.key_to_ino
    //         .lock()
    //         .unwrap(); // TODO

    //     let mut lock_ino_to_key = self.ino_to_key
    //         .lock()
    //         .unwrap(); // TODO
        
    //     let prefix = lock_ino_to_key.get(&ino)
    //         .unwrap(); // TODO

    //     let lo_res = tokio::task::block_in_place(|| {
    //         tokio::runtime::Handle::current().block_on(async {
    //             self.client.list_objects_v2()
    //                 .bucket(&self.bucket)
    //                 .prefix(prefix)
    //                 .send()
    //                 .await
    //                 .unwrap() // TODO
    //         })
    //     });

    //     let mut entries = vec![
    //         (ROOT_INO, FileType::Directory, ".".to_string()),
    //         (ROOT_INO, FileType::Directory, "..".to_string()),
    //     ];

    //     for obj in lo_res.contents() {
    //         let key = obj.key.clone()
    //             .unwrap(); // TODO

    //         if let Some(ino) = lock_key_to_ino.get(&key) { // TODO shadow
    //             entries.push((*ino, FileType::RegularFile, key));
    //         } else {
    //             let next_ino = self.next_ino();
    //             lock_ino_to_key.insert(next_ino, key.clone());
    //             lock_key_to_ino.insert(key.clone(), next_ino);

    //             entries.push((next_ino, FileType::RegularFile, key));
    //         }
    //     }

    //     for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
    //         let next_offset = (i + 1) as i64;
    //         if reply.add(entry.0, next_offset, entry.1, entry.2.clone()) {
    //             break;
    //         }
    //     }

    //     reply.ok();
    // }
}

fn main() {
    let matches = Command::new("objectfs")
        .arg(
            Arg::new("MOUNT_POINT")
                .required(true)
                .index(1)
        )
        .get_matches();

    env_logger::init();

    let mountpoint = matches.get_one::<String>("MOUNT_POINT").unwrap();
    let mut options = vec![MountOption::FSName("objectfs".to_string())];
    options.push(MountOption::AutoUnmount);
    options.push(MountOption::AllowRoot);    

    let config = util::fs::poll_until_ready(aws_config::load_from_env());
    let client = aws_sdk_s3::Client::new(&config);

    let fs = ObjectFS::new(&client, "fuse-tmp");
    fuser::mount2(fs, mountpoint, &options).unwrap();
}
