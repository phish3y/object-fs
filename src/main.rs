use google_cloud_storage::client::{Client, ClientConfig};
use tracing::{info, span, Level};

mod adapters;
mod fs;
mod fuse;
mod model;
mod util;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().json().init();

    let span = span!(Level::INFO, "main", context = "main");
    let _e = span.enter();
    info!("called");

    let matches = clap::Command::new("objectfs")
        .arg(clap::Arg::new("BUCKET_URI").required(true).index(1))
        .arg(clap::Arg::new("MOUNT_POINT").required(true).index(2))
        .get_matches();

    let bucket_uri = matches.get_one::<String>("BUCKET_URI").unwrap(); // TODO check if bucket exists
    let mountpoint = matches.get_one::<String>("MOUNT_POINT").unwrap();
    info!(bucket_uri = bucket_uri, mountpoint = mountpoint, "args");

    let mut options = vec![fuser::MountOption::FSName("objectfs".to_string())];
    options.push(fuser::MountOption::AutoUnmount);
    options.push(fuser::MountOption::AllowRoot);

    let provider = util::object::parse_provider_from_uri(bucket_uri).unwrap();

    let client: Box<dyn adapters::Object> = if provider.is_aws() {
        info!(client="aws");
        let config = util::poll::poll_until_ready(aws_config::load_from_env());
        Box::new(aws_sdk_s3::Client::new(&config))
    } else {
        info!(client="gcs");
        let config = util::poll::poll_until_ready_error(ClientConfig::default().with_auth()).unwrap();
        Box::new(Client::new(config))
    };

    let bucket = util::object::parse_bucket_from_uri(bucket_uri);
    info!(bucket=bucket);
    let fs = fs::ObjectFS::new(client, bucket);
    fuser::mount2(fs, mountpoint, &options).unwrap();
}
