use aws_sdk_s3::{primitives::ByteStream, Client};
use clap::{Arg, Command};
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, ReplyWrite, Request
};
use libc::{EIO, ENOENT};
use std::{collections::HashMap, ffi::OsStr, sync::Mutex, time::{Duration, SystemTime}};

const TTL: Duration = Duration::new(0, 0); 
const ROOT_INO: u64 = 1;

struct ObjectFS {
    client: Client,
    bucket: String,
    current_ino: Mutex<u64>,
    ino_to_key: Mutex<HashMap<u64, String>>,
    key_to_ino: Mutex<HashMap<String, u64>>
}

impl ObjectFS {

    fn new(client: Client, bucket: &str) -> Self {
        let mut ino_to_key = HashMap::new();
        ino_to_key.insert(ROOT_INO, "".to_string());
        
        let mut key_to_ino = HashMap::new();
        key_to_ino.insert("".to_string(), ROOT_INO);

        Self {
            client,
            bucket: bucket.to_string(),
            current_ino: ROOT_INO.into(),
            ino_to_key: Mutex::new(ino_to_key),
            key_to_ino: Mutex::new(key_to_ino)
        }
    }

    fn init_inos(&self) {
        let lo_res = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.client.list_objects_v2()
                    .bucket(&self.bucket)
                    .prefix("") // TODO
                    .send()
                    .await
                    .unwrap() // TODO
            })
        });

        for obj in lo_res.contents() {
            let key = obj.key.clone()
                .unwrap(); // TODO

            let next_ino = self.next_ino();
            self.ino_to_key
                .lock()
                .unwrap() // TODO
                .insert(next_ino, key.clone());
            self.key_to_ino
                .lock()
                .unwrap() // TODO
                .insert(key.clone(), next_ino);
        }
    }

    fn next_ino(&self) -> u64 {
        let mut cur_ino = self.current_ino.lock().unwrap(); // TODO
        *cur_ino += 1;

        return *cur_ino;
    }

    fn root_attr(&self) -> FileAttr {
        FileAttr {
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
        }
    }
}

impl Filesystem for ObjectFS {

    fn lookup(
        &mut self, 
        _req: &Request, 
        parent: u64, 
        name: &OsStr, 
        reply: ReplyEntry
    ) {
        log::debug!("`lookup` parent: {}, name: {:?}", parent, name);

        if parent == ROOT_INO && name == "/" {
            reply.entry(&TTL, &self.root_attr(), 0);
            return;
        }

        let mut lock_key_to_ino = self.key_to_ino
            .lock()
            .unwrap(); // TODO

        let lock_ino_to_key = self.ino_to_key
            .lock()
            .unwrap(); // TODO

        if let Some(key) = lock_ino_to_key.get(&parent) {
            // TODO only supports root
            let key = format!(
                "{}{}", 
                key, 
                name.to_string_lossy()
            );

            let head_res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    self.client.head_object()
                        .bucket(&self.bucket)
                        .key(&key)
                        .send()
                        .await
                })
            });

            let head = match head_res {
                Err(err) => {   
                    if let Some(svc_err) = err.as_service_error() {
                        if svc_err.is_not_found() {
                            log::warn!("`lookup` not found: {}", &key);
                            reply.error(ENOENT);
                        } else {
                            log::error!("`lookup` failed to head_object: {}", err);
                            reply.error(EIO);
                        }
                    }

                    return;
                }
                Ok(head) => head
            };

            let ino = if let Some(ino) = lock_key_to_ino.get(&key) {
                *ino
            } else {
                let next_ino = self.next_ino();
                lock_key_to_ino.insert(key.clone(), next_ino);

                next_ino
            };

            let size = head.content_length().unwrap() as u64; // TODO
            let secs = head.last_modified
                .unwrap() // TODO
                .secs();
            let nanos = head.last_modified.
                unwrap() // TODO
                .subsec_nanos();
            let atime = SystemTime::UNIX_EPOCH + Duration::new(secs as u64, nanos);

            let fa = FileAttr {
                ino,
                size,
                blksize: 0,
                blocks: (size + 511) / 512,
                atime,
                mtime: SystemTime::now(),
                ctime: SystemTime::now(),
                crtime: SystemTime::now(),
                kind: fuser::FileType::RegularFile,
                perm: 0o755,
                nlink: 2,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
            };
            
            reply.entry(&TTL, &fa, 0); // TODO generation?
        } else {
            // TODO
            log::error!("`lookup` i should not yet be here");
            reply.error(ENOENT);
        }
    }

    fn getattr(
        &mut self, 
        _req: &Request, 
        ino: u64, 
        fh: Option<u64>, 
        reply: ReplyAttr
    ) {
        log::debug!("`getattr` ino: {}, fh: {}", ino, fh.unwrap_or(0));

        if ino == ROOT_INO {
            reply.attr(&TTL, &self.root_attr());
            return;
        }

        let lock_ino_to_key = self.ino_to_key
            .lock() 
            .unwrap(); // TODO


        let mut lock_key_to_ino = self.key_to_ino
            .lock()
            .unwrap(); // TODO

        if let Some(key) = lock_ino_to_key.get(&ino) { 
            let head_res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    self.client.head_object()
                        .bucket(&self.bucket)
                        .key(key) // TODO only supports root
                        .send()
                        .await
                })
            });

            let head = match head_res {
                Err(err) => {   
                    if let Some(svc_err) = err.as_service_error() {
                        if svc_err.is_not_found() {
                            log::warn!("`getattr` not found: {}", &key);
                            reply.error(ENOENT);
                        } else {
                            log::error!("`getattr` failed to head_object: {}", err);
                            reply.error(EIO);
                        }
                    }

                    return;
                }
                Ok(head) => head
            };

            let ino = if let Some(ino) = lock_key_to_ino.get(key) {
                *ino
            } else {
                let next_ino = self.next_ino();
                lock_key_to_ino.insert(key.clone(), next_ino);

                next_ino
            };

            let size = head.content_length().unwrap() as u64; // TODO
            let secs = head.last_modified
                .unwrap() // TODO
                .secs();
            let nanos = head.last_modified
                .unwrap() // TODO
                .subsec_nanos(); 
            let atime = SystemTime::UNIX_EPOCH + Duration::new(secs as u64, nanos);

            let fa = FileAttr {
                ino,
                size,
                blksize: 0,
                blocks: (size + 511) / 512,
                atime,
                mtime: SystemTime::now(),
                ctime: SystemTime::now(),
                crtime: SystemTime::now(),
                kind: fuser::FileType::RegularFile,
                perm: 0o755,
                nlink: 2,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
            };

            reply.attr(&TTL, &fa);
        } else {
            // TODO
            log::error!("`lookup` i should not yet be here");
            reply.error(ENOENT);
        }
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
        log::debug!("`mknod` parent: {}, name: {:?}, mode: {}", parent, name, mode);

        if (mode & libc::S_IFMT) != libc::S_IFREG { // TODO support link
            reply.error(libc::EOPNOTSUPP);
            return;
        }

        let mut lock_ino_to_key = self.ino_to_key
            .lock()
            .unwrap(); // TODO

        let mut lock_key_to_ino = self.key_to_ino
            .lock()
            .unwrap(); // TODO

        let key = match lock_ino_to_key.get(&parent) {
            None => {
                // TODO shouldn't ever get here right now
                log::error!("`mknod` failed to find parent key for ino: {}", parent);
                reply.error(ENOENT);
                return;
            }
            Some(key) => key
        };

        let key = format!("{}{}", key, name.to_string_lossy()); // TODO only suppots root

        let put_res = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.client.put_object()
                    .bucket(&self.bucket)
                    .key(&key)
                    .send()
                    .await
            })
        });

        match put_res {
            Err(err) => {
                log::error!("`mknod` failed to put_object: {}", err);
                reply.error(EIO);
                return;
            }
            Ok(_) => {
                let next_ino = self.next_ino();
                let fa = FileAttr {
                    ino: next_ino,
                    size: 0,
                    blksize: 0,
                    blocks: 0,
                    atime: SystemTime::now(),
                    mtime: SystemTime::now(),
                    ctime: SystemTime::now(),
                    crtime: SystemTime::now(),
                    kind: fuser::FileType::RegularFile,
                    perm: 0o755,
                    nlink: 2,
                    uid: 0,
                    gid: 0,
                    rdev: 0,
                    flags: 0,
                };

                lock_ino_to_key.insert(next_ino, key.clone());
                lock_key_to_ino.insert(key, next_ino);

                reply.entry(&TTL, &fa, 0);
            }
        }
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
        log::debug!("`read` ino: {}, fh: {}, offset: {}, size: {}", ino, fh, offset, size);

        let lock_ino_to_key = self.ino_to_key
            .lock() 
            .unwrap(); // TODO

        let key = match lock_ino_to_key.get(&ino) {
            Some(key) => key,
            None => {
                log::error!("no entry for ino: {}", ino);
                reply.error(ENOENT);
                return;
            }
        };

        let range = format!("bytes={}-{}", offset, offset + size as i64 - 1);

        let buffer = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let get_res = self.client.get_object()
                    .bucket(&self.bucket)
                    .key(key)
                    .range(&range)
                    .send()
                    .await
                    .unwrap(); // TODO
                
                get_res.body
                    .collect()
                    .await
                    .unwrap() // TODO
            })
        });

        reply.data(&buffer.into_bytes())
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
        log::debug!("`write` ino: {}, fh: {}, offset: {}, len: {}", ino, fh, offset, data.len());

        let lock_ino_to_key = self.ino_to_key
            .lock()
            .unwrap(); // TODO
    
        let key = match lock_ino_to_key.get(&ino) {
            None => {
                log::error!("no entry for ino: {}", ino);
                reply.error(ENOENT);
                return;
            }
            Some(key) => key
        };

        let get_res = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.client.get_object()
                    .bucket(&self.bucket)
                    .key(key)
                    .send()
                    .await
            })
        });
        
        let mut existing_data = match get_res {
            Err(err) => {   
                if let Some(svc_err) = err.as_service_error() {
                    if !svc_err.is_no_such_key() {
                        log::error!("`write` failed to get_object: {}", err);
                        reply.error(EIO);
                        return;
                    }
                }

                Vec::new()
            }
            Ok(get) => {
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        let body = get.body.collect().await;
                        body.map_or_else(
                            |err| {
                                log::warn!("`write` failed to collect body: {}", err);
                                Vec::new()
                            },
                            |data| data.to_vec()
                        )
                    })
                })
            }
        };

        let end_offset = offset as usize + data.len();
        if end_offset > existing_data.len() {
            existing_data.resize(end_offset, 0);
        }
        existing_data[offset as usize..end_offset].copy_from_slice(data);

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.client.put_object()
                    .bucket(&self.bucket)
                    .key(key)
                    .body(ByteStream::from(existing_data))
                    .send()
                    .await
                    .unwrap() // TODO
            })
        });

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

        let mut lock_key_to_ino = self.key_to_ino
            .lock()
            .unwrap(); // TODO

        let mut lock_ino_to_key = self.ino_to_key
            .lock()
            .unwrap(); // TODO
        
        let prefix = lock_ino_to_key.get(&ino)
            .unwrap(); // TODO

        let lo_res = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.client.list_objects_v2()
                    .bucket(&self.bucket)
                    .prefix(prefix)
                    .send()
                    .await
                    .unwrap() // TODO
            })
        });

        let mut entries = vec![
            (ROOT_INO, FileType::Directory, ".".to_string()),
            (ROOT_INO, FileType::Directory, "..".to_string()),
        ];

        for obj in lo_res.contents() {
            let key = obj.key.clone()
                .unwrap(); // TODO

            if let Some(ino) = lock_key_to_ino.get(&key) { // TODO shadow
                entries.push((*ino, FileType::RegularFile, key));
            } else {
                let next_ino = self.next_ino();
                lock_ino_to_key.insert(next_ino, key.clone());
                lock_key_to_ino.insert(key.clone(), next_ino);

                entries.push((next_ino, FileType::RegularFile, key));
            }
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

    let fs = ObjectFS::new(client, "fuse-tmp");
    fs.init_inos();

    fuser::mount2(fs, mountpoint, &options).unwrap();
}
