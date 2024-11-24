use std::time::{Duration, SystemTime};

use google_cloud_storage::http::objects::{
    download::Range,
    get::GetObjectRequest,
    list::ListObjectsRequest,
    upload::{Media, UploadObjectRequest, UploadType},
};

use crate::{adapters, model, util};

impl adapters::adapter::ObjectAdapter for google_cloud_storage::client::Client {
    fn fs_put_object(&self, bucket: &str, key: &str) -> Result<(), model::fs::FSError> {
        let req = UploadObjectRequest {
            bucket: bucket.to_string(),
            ..Default::default()
        };

        util::poll::poll_until_ready_error(self.upload_object(
            &req,
            "".as_bytes(),
            &UploadType::Simple(Media::new(key.to_string())),
        ))
        .map_err(|err| model::fs::FSError {
            message: format!("failed to put_object at: {}, {}", key, err.to_string()),
        })?;

        Ok(())
    }

    fn fs_list_objects(
        &self,
        bucket: &str,
        prefix: &str,
    ) -> Result<Vec<model::fs::FSObject>, model::fs::FSError> {
        let mut objects = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let req = ListObjectsRequest {
                bucket: bucket.to_string(),
                prefix: Some(prefix.to_string()),
                page_token: continuation_token.clone(),
                ..Default::default()
            };

            let lo =
                util::poll::poll_until_ready_error(self.list_objects(&req)).map_err(|err| {
                    model::fs::FSError {
                        message: format!(
                            "failed to list_objects at: {}, {}",
                            prefix,
                            err.to_string()
                        ),
                    }
                })?;

            if let Some(objs) = lo.items {
                for obj in objs {
                    let modified_time = SystemTime::UNIX_EPOCH
                        + Duration::from_secs(
                            obj.updated
                                .unwrap_or(time::OffsetDateTime::now_utc())
                                .unix_timestamp() as u64,
                        );

                    objects.push(model::fs::FSObject {
                        key: obj.name,
                        size: obj.size,
                        modified_time,
                    });
                }
            }

            continuation_token = lo.next_page_token;
            if continuation_token.is_none() {
                break;
            }
        }

        Ok(objects)
    }

    fn fs_head_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<Option<model::fs::FSObject>, model::fs::FSError> {
        let req = GetObjectRequest {
            bucket: bucket.to_string(),
            object: key.to_string(),
            ..Default::default()
        };

        let o = match util::poll::poll_until_ready_error(self.get_object(&req)) {
            Err(google_cloud_storage::http::Error::Response(err)) => {
                if err.code == 404 {
                    return Ok(None);
                }

                return Err(model::fs::FSError {
                    message: format!("failed to get_object: {}, {}", key, err.to_string()),
                });
            }
            Err(err) => {
                return Err(model::fs::FSError {
                    message: format!("failed to get_object: {}, {}", key, err.to_string()),
                });
            }
            Ok(o) => o,
        };

        let modified_time = SystemTime::UNIX_EPOCH
            + Duration::from_secs(
                o.updated
                    .unwrap_or(time::OffsetDateTime::now_utc())
                    .unix_timestamp() as u64,
            );

        Ok(Some(model::fs::FSObject {
            key: o.name,
            size: o.size,
            modified_time,
        }))
    }

    fn fs_download_object(&self, bucket: &str, key: &str) -> Result<Vec<u8>, model::fs::FSError> {
        let req = GetObjectRequest {
            bucket: bucket.to_string(),
            object: key.to_string(),
            ..Default::default()
        };

        let bytes =
            util::poll::poll_until_ready_error(self.download_object(&req, &Range::default()))
                .map_err(|err| model::fs::FSError {
                    message: format!("failed to get_object: {}, {}", key, err.to_string()),
                })?;

        Ok(bytes)
    }
}
