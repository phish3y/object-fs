use aws_sdk_s3::Client;
use clap::{Arg, Command};
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, Request
};
use libc::ENOENT;
use std::time::{Duration, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(1); // 1 second

struct ObjectFS {
    client: Client,
    root_attr: FileAttr,
    fioc_file_attr: FileAttr,
}

impl ObjectFS {

    fn new(client: Client) -> Self {
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };

        let root_attr = FileAttr {
            ino: 1,
            size: 0,
            blocks: 0,
            atime: UNIX_EPOCH,
            mtime: UNIX_EPOCH,
            ctime: UNIX_EPOCH,
            crtime: UNIX_EPOCH,
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 2,
            uid,
            gid,
            rdev: 0,
            flags: 0,
            blksize: 512,
        };

        let fioc_file_attr = FileAttr {
            ino: 2,
            size: 0,
            blocks: 1,
            atime: UNIX_EPOCH, // 1970-01-01 00:00:00
            mtime: UNIX_EPOCH,
            ctime: UNIX_EPOCH,
            crtime: UNIX_EPOCH,
            kind: FileType::RegularFile,
            perm: 0o644,
            nlink: 1,
            uid,
            gid,
            rdev: 0,
            flags: 0,
            blksize: 512,
        };

        Self {
            client,
            root_attr,
            fioc_file_attr,
        }
    }
}

impl Filesystem for ObjectFS {

    // fn lookup(
    //     &mut self, 
    //     _req: &Request, 
    //     parent: u64, 
    //     name: &OsStr, 
    //     reply: ReplyEntry
    // ) {
    //     if parent == 1 && name.to_str() == Some("fioc") {
    //         reply.entry(&TTL, &self.fioc_file_attr, 0);
    //     } else {
    //         reply.error(ENOENT);
    //     }
    // }

    fn getattr(
        &mut self, 
        _: &Request, 
        ino: u64, 
        fh: Option<u64>, 
        rep: ReplyAttr
    ) {
        log::debug!("`getattr` ino: {}, fh: {}", ino, fh.unwrap_or(0));

        match ino {
            1 => rep.attr(&TTL, &self.root_attr),
            2 => { 
                rep.attr(&TTL, &self.fioc_file_attr) 
            },
            _ => rep.error(ENOENT),
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
    ) {}

    fn readdir(
        &mut self,
        _req: &Request,
        _: u64,
        _fh: u64,
        _: i64,
        _: ReplyDirectory,
    ) {}
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
    let mut options = vec![MountOption::FSName("fioc".to_string())];
    options.push(MountOption::AutoUnmount);
    options.push(MountOption::AllowRoot);    

    let config = aws_config::load_from_env().await;
    let client = aws_sdk_s3::Client::new(&config);

    let fs = ObjectFS::new(client);
    fuser::mount2(fs, mountpoint, &options).unwrap();
}
