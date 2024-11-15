use aws_sdk_s3::Client;
use clap::{Arg, Command};
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request
};
use libc::ENOENT;
use std::{collections::HashMap, ffi::OsStr, sync::Mutex, time::{Duration, SystemTime}};

const TTL: Duration = Duration::from_secs(1); 
const ROOT_INO: u64 = 1;

struct ObjectFS {
    client: Client,
    current_ino: Mutex<u64>,
    ino_to_path: Mutex<HashMap<u64, String>>,
    // path_to_ino: Mutex<HashMap<String, u64>>
}

impl ObjectFS {

    fn new(client: Client) -> Self {
        let mut ino_to_path = HashMap::new();
        ino_to_path.insert(ROOT_INO, "".to_string());
        Self {
            client,
            current_ino: ROOT_INO.into(),
            ino_to_path: Mutex::new(ino_to_path),
            // path_to_ino: Mutex::new(HashMap::new())
        }
    }
}

impl ObjectFS {

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
        _: &Request, 
        parent: u64, 
        name: &OsStr, 
        rep: ReplyEntry
    ) {
        log::debug!("`lookup` parent: {}, name: {:?}", parent, name);

        if parent == ROOT_INO && name == "/" {
            rep.entry(&TTL, &self.root_attr(), 0);
            return;
        }

        if let Some(path) = self.ino_to_path.lock().unwrap().get(&parent) { // TODO
            let key = format!(
                "{}{}", // TODO only supports root
                path, 
                name.to_str().unwrap(), // TODO
            );

            log::debug!("`lookup` key: {}", key);
            let head_res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    self.client.head_object()
                        .bucket("fuse-tmp")
                        .key(&key)
                        .send()
                        .await
                        .unwrap() // TODO
                })
            });

            // TODO check if 404

            let next_ino = self.next_ino();
            let size = head_res.content_length().unwrap() as u64; // TODO
            let secs = head_res.last_modified.unwrap().secs(); // TODO
            let nanos = head_res.last_modified.unwrap().subsec_nanos(); // TODO
            let atime = SystemTime::UNIX_EPOCH + Duration::new(secs as u64, nanos);

            let fa = FileAttr {
                ino: next_ino,
                size,
                blksize: 0,
                blocks: (size + 511) / 512,
                atime,
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
            };

            self.ino_to_path.lock().unwrap().insert(next_ino, key); // TODO, also: need inserted?
            rep.entry(&TTL, &fa, 0); // TODO generation?
        } else {
            // TODO
            log::warn!("`lookup` i should not yet be here");
            rep.error(ENOENT);
        }
    }

    fn getattr(
        &mut self, 
        _: &Request, 
        ino: u64, 
        fh: Option<u64>, 
        rep: ReplyAttr
    ) {
        log::debug!("`getattr` ino: {}, fh: {}", ino, fh.unwrap_or(0));

        if let Some(path) = self.ino_to_path.lock().unwrap().get(&ino) { // TODO
            let head_res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    self.client.head_object()
                        .bucket("fuse-tmp") // TODO
                        .key(path) // TODO only supports root
                        .send()
                        .await
                        .unwrap() // TODO
                })
            });

            // TODO check if 404

            let next_ino = self.next_ino();
            let size = head_res.content_length().unwrap() as u64; // TODO
            let secs = head_res.last_modified.unwrap().secs(); // TODO
            let nanos = head_res.last_modified.unwrap().subsec_nanos(); // TODO
            let atime = SystemTime::UNIX_EPOCH + Duration::new(secs as u64, nanos);

            let fa = FileAttr {
                ino: next_ino,
                size,
                blksize: 0,
                blocks: (size + 511) / 512,
                atime,
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
            };

            self.ino_to_path.lock().unwrap().insert(next_ino, path.to_string()); // TODO, also: need inserted?
            rep.attr(&TTL, &fa);
        } else {
            // TODO
            log::warn!("`getattr` i should not yet be here");
            rep.error(ENOENT);
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        _: u64,
        _fh: u64,
        _: i64,
        _size: u32,
        _flags: i32,
        _lock: Option<u64>,
        _: ReplyData,
    ) {
        log::debug!("`read`");
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        log::debug!("`readdir` ino: {}, offset: {}", ino, offset);

        if ino != 1 {
            reply.error(ENOENT); //TODO
            return;
        }

        let lo_res = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.client.list_objects_v2()
                    .bucket("tmp-fuse") // TODO
                    .prefix("") // TODO
                    .delimiter("/")
                    .send()
                    .await
                    .unwrap() // TODO
            })
        });

        let mut entries = vec![
            (1, FileType::Directory, ".".to_string()),
            (1, FileType::Directory, "..".to_string()),
        ];

        for obj in lo_res.contents() {
            let key = obj.key.clone().unwrap(); // TODO
            entries.push((self.next_ino(), FileType::RegularFile, key));
        }

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
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

    let fs = ObjectFS::new(client);
    fuser::mount2(fs, mountpoint, &options).unwrap();
}
