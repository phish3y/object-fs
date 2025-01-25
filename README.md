# object-fs [![Rust Report Card](https://rust-reportcard.xuri.me/badge/github.com/phish3y/object-fs)](https://rust-reportcard.xuri.me/report/github.com/phish3y/object-fs) ![CI Status](https://github.com/phish3y/object-fs/actions/workflows/tests.yaml/badge.svg)


FUSE filesystem abstraction over object storages: Amazon S3, Google Cloud Storage

## Usage
##### AWS
[Setup AWS credentials](https://docs.aws.amazon.com/cli/latest/userguide/cli-chap-configure.html)
```sh
./objectfs s3://<bucket-name> <mount-point>
```
##### GCP
[Setup GCP credentials](https://cloud.google.com/sdk/docs/initializing)

```sh
GOOGLE_APPLICATION_CREDENTIALS="$HOME/.config/gcloud/application_default_credentials.json" \
./objectfs gs://<bucket-name> <mount-point>
```
