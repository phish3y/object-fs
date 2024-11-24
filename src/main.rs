use clap::{Arg, Command};
use fs::ObjectFS;
use fuser::MountOption;

mod adapters;
mod fs;
mod fuse;
mod model;
mod util;

fn main() {
    let matches = Command::new("objectfs")
        .arg(Arg::new("MOUNT_POINT").required(true).index(1))
        .get_matches();

    env_logger::init();

    let mountpoint = matches.get_one::<String>("MOUNT_POINT").unwrap();
    let mut options = vec![MountOption::FSName("objectfs".to_string())];
    options.push(MountOption::AutoUnmount);
    options.push(MountOption::AllowRoot);

    let config = util::poll::poll_until_ready(aws_config::load_from_env());
    let client = aws_sdk_s3::Client::new(&config);

    let fs = ObjectFS::new(Box::new(client), "fuse-tmp");
    fuser::mount2(fs, mountpoint, &options).unwrap();
}
