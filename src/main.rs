use tracing::{info, span, Level};

mod adapters;
mod fs;
mod fuse;
mod model;
mod util;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().json().init();

    let span = span!(Level::INFO, "main", context="main");
    let _e = span.enter();
    info!("called");

    let matches = clap::Command::new("objectfs")
        .arg(clap::Arg::new("BUCKET").required(true).index(1))
        .arg(clap::Arg::new("MOUNT_POINT").required(true).index(2))
        .get_matches();

    let bucket = matches.get_one::<String>("BUCKET").unwrap(); // TODO check if bucket exists
    let mountpoint = matches.get_one::<String>("MOUNT_POINT").unwrap();
    info!(bucket=bucket, mountpoint=mountpoint, "args");
    
    let mut options = vec![fuser::MountOption::FSName("objectfs".to_string())];
    options.push(fuser::MountOption::AutoUnmount);
    options.push(fuser::MountOption::AllowRoot);

    let config = util::poll::poll_until_ready(aws_config::load_from_env());
    let client = aws_sdk_s3::Client::new(&config);

    let fs = fs::ObjectFS::new(Box::new(client), bucket);
    fuser::mount2(fs, mountpoint, &options).unwrap();
}
