use aws_sdk_s3::{operation::head_object::HeadObjectOutput, primitives::DateTime};
use clap::{Arg, Command};
use fuser::{
    FileAttr, 
    Filesystem, 
    MountOption, 
    Request
};
use std::{
    collections::HashMap, 
    sync::Mutex, 
    time::{Duration, SystemTime}
};

mod model;

const TTL: Duration = Duration::new(0, 0); 
const ROOT_INO: u64 = 1;
const KEEP_FILE: &str = ".keep";


struct ObjectFS<'a> {
    client: &'a dyn model::s3::ObjectFSS3,
    bucket: String,
    current_ino: Mutex<u64>,
    ino_to_node: Mutex<HashMap<u64, model::fs::Node>>,
    key_to_node: Mutex<HashMap<String, model::fs::Node>>
}

impl<'a> ObjectFS<'a> {

    fn new(client: &'a dyn model::s3::ObjectFSS3, bucket: &str) -> Self {
        let mut ino_to_node = HashMap::new();
        let root_node = model::fs::Node {
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
        let mut cur_ino = self.current_ino.lock().unwrap(); // TODO
        *cur_ino += 1;

        return *cur_ino;
    }

    fn index_file_path(&self, path: &str, size: Option<i64>, modified_time: Option<DateTime>) {
        let size = size.unwrap_or(0) as u64; // TODO
        let secs = if modified_time.is_some() {
            modified_time.unwrap().secs()
        } else {
            0
        };
        let nanos = if modified_time.is_some() {
            modified_time.unwrap().subsec_nanos()
        } else {
            0
        };
        let atime = SystemTime::UNIX_EPOCH + Duration::new(secs as u64, nanos);

        let ino = self.next_ino();
        let attr = FileAttr {
            ino,
            size,
            blksize: 0, // TODO
            blocks: 0, // TODO
            atime,
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
        };

        let mut parent = self.get_parent(path);
        while let Some(p) = parent {
            println!("{}", p);



            parent = self.get_parent(&p);
        }
    }

    fn index_file(
        &self, 
        path: &str, 
        size: Option<i64>, 
        modified_time: Option<DateTime>, 
        parent: u64
    ) -> Result<(), i32> {
        let size = size.unwrap_or(0) as u64;

        let secs = if modified_time.is_some() {
            modified_time.unwrap().secs()
        } else {
            0
        };
        let nanos = if modified_time.is_some() {
            modified_time.unwrap().subsec_nanos()
        } else {
            0
        };
        let atime = SystemTime::UNIX_EPOCH + Duration::new(secs as u64, nanos);

        let ino = self.next_ino();

        let node = model::fs::Node {
            attr: FileAttr {
                ino,
                size,
                blksize: 0, // TODO
                blocks: 0, // TODO
                atime,
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
            key: path.to_string(),
            parent
        };

        self.ino_to_node
            .lock()
            .unwrap()
            .insert(ino, node.clone());

        self.key_to_node
            .lock()
            .unwrap()
            .insert(path.to_string(), node);

        Ok(())
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
        let client = model::s3::MockS3Client{};
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
    fn test_index_file_path() {
        let client = model::s3::MockS3Client{};
        let fs = ObjectFS::new(&client, "dummy-bucket");

        let cases = vec![
            "file",
            "folder/file",
            "folder/subfolder/file",
        ];

        for input in cases {
            // fs.index_file_path(input);
        }
    }

    #[test]
    fn test_index_file() {
        let client = model::s3::MockS3Client{};
        let fs = ObjectFS::new(&client, "dummy-bucket");

        let cases = vec![
            ("/file", Some(10), 10, Some(DateTime::from_secs(1_695_084_900)), 1),
            ("folder/file", None, 0, None, 5),
            ("folder/subfolder/file", Some(0), 0, Some(DateTime::from_secs(0)), 7)
        ];

        for (path, size, expected_size, modified_time, parent) in cases {
            let fs = ObjectFS::new(&client, "dummy-bucket");
            fs.index_file(path, size, modified_time, parent).unwrap();
            
            let guard = fs.ino_to_node
                .lock()
                .unwrap();
            let result = guard.get(&2).unwrap();
            assert_eq!(result.parent, parent, "failed on parent for case: {}", path);
            assert_eq!(result.key, path, "failed on key for case: {}", path);
            assert_eq!(result.attr.ino, 2, "failed on ino for case: {}", path);
            assert_eq!(result.attr.size, expected_size, "failed on size for case: {}", path);
            println!("{:?}", result.attr.atime);
        }
    }

    #[test]
    fn test_get_parent() {
        let client = model::s3::MockS3Client{};
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

        let put_res = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.client
                    .put_object(&self.bucket, KEEP_FILE)
                    .await
            })
        });

        match put_res {
            Err(err) => {
                log::error!("`init` failed to put_object: {}", err);
                return Err(-1);
            }
            Ok(_) => {}
        }

        
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let mut con_token: Option<String> = None;
                
                loop {
                    let res = self.client
                        .list_objects_v2(&self.bucket, "", con_token)
                        .await;

                    let lo = match res {
                        Err(err) => {
                            log::error!("`init` failed to list_objects: {}", err);
                            return Err(-1);
                        }
                        Ok(lo) => lo
                    };

                    for obj in lo.contents() {
                        if let Some(key) = obj.key() {
                            let res = self.client
                                .head_object(&self.bucket, key)
                                .await;

                            let ho = match res {
                                Err(err) => {
                                    log::error!("`init` failed to head_object: {}, {}", key, err);
                                    return Err(-1);
                                }
                                Ok(ho) => ho
                            };

                            log::info!("{}", key);
                            log::info!("{:?}", ho.content_type());
                            if ho.content_type().unwrap_or("").contains("application/x-directory") {
                                let keep = format!("{}{}", key, KEEP_FILE);
                                match self.client.put_object(&self.bucket, &keep).await {
                                    Err(err) => {
                                        log::error!("`init` failed to put_object: {}, {}", keep, err);
                                        return Err(-1); 
                                    }
                                    Ok(_) => ()
                                }

                                


                            } else {

                            }

                        }
                    }

                    con_token = lo.next_continuation_token().map(|tok| tok.to_string());
                    if con_token.is_none() {
                        break;
                    }
                }

                Ok(())
            })
        })?;

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

#[::tokio::main]
async fn main() {
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

    let config = aws_config::load_from_env().await;
    let client = aws_sdk_s3::Client::new(&config);

    let fs = ObjectFS::new(&client, "fuse-tmp");
    fuser::mount2(fs, mountpoint, &options).unwrap();
}
