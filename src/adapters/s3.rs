use std::time::{Duration, SystemTime};

use aws_sdk_s3::primitives::ByteStream;

use crate::{adapters, model, util};

impl adapters::Object for aws_sdk_s3::Client {
    fn fs_put_object(
        &self,
        bucket: &str,
        key: &str,
        body: Option<Vec<u8>>,
    ) -> Result<(), model::fs::FSError> {
        let mut req = self.put_object().bucket(bucket).key(key);

        if body.is_some() {
            req = req.body(ByteStream::from(body.unwrap()));
        }

        util::poll::poll_until_ready_error(req.send()).map_err(|err| model::fs::FSError {
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
            let mut req = self.list_objects_v2().bucket(bucket).prefix(prefix);

            if let Some(tok) = continuation_token {
                req = req.continuation_token(tok);
            }

            let lo = util::poll::poll_until_ready_error(req.send()).map_err(|err| {
                model::fs::FSError {
                    message: format!("failed to list_objects at: {}, {}", prefix, err.to_string()),
                }
            })?;

            for o in lo.contents() {
                let key = o.key().unwrap_or("").to_string();
                let size = o.size().unwrap_or(0);
                let secs = if o.last_modified().is_some() {
                    o.last_modified().unwrap().secs()
                } else {
                    0
                };
                let nanos = if o.last_modified().is_some() {
                    o.last_modified().unwrap().subsec_nanos()
                } else {
                    0
                };
                let modified_time = SystemTime::UNIX_EPOCH + Duration::new(secs as u64, nanos);

                objects.push(model::fs::FSObject {
                    key,
                    size,
                    modified_time,
                });
            }

            continuation_token = lo.next_continuation_token().map(|tok| tok.to_string());
            if continuation_token.is_none() {
                break;
            }
        }

        Ok(objects)
    }

    fn fs_download_object(
        &self,
        bucket: &str,
        key: &str,
        range: Option<(u64, u64)>,
    ) -> Result<Option<Vec<u8>>, model::fs::FSError> {
        let mut req = self.get_object().bucket(bucket).key(key);

        if range.is_some() {
            req = req.range(format!("bytes={}-{}", range.unwrap().0, range.unwrap().1));
        }

        let o = match util::poll::poll_until_ready_error(req.send()) {
            Err(err) => {
                if let Some(svc_err) = err.as_service_error() {
                    if svc_err.is_no_such_key() {
                        return Ok(None);
                    }
                }

                return Err(model::fs::FSError {
                    message: format!("failed to get_object: {}, {}", key, err.to_string()),
                });
            }
            Ok(o) => o,
        };

        let bytes = util::poll::poll_until_ready_error(o.body.collect()).map_err(|err| {
            model::fs::FSError {
                message: format!("failed to collect body: {}, {}", key, err.to_string()),
            }
        })?;

        Ok(Some(bytes.into_bytes().to_vec()))
    }

    fn fs_bucket_exists(&self, bucket: &str) -> Result<bool, model::fs::FSError> {
        match util::poll::poll_until_ready_error(self.head_bucket().bucket(bucket).send()) {
            Err(err) => {
                if let Some(svc_err) = err.as_service_error() {
                    if svc_err.is_not_found() {
                        return Ok(false);
                    }
                }

                Err(model::fs::FSError {
                    message: format!("failed to head_bucket: {}, {}", bucket, err.to_string()),
                })
            }
            Ok(_) => Ok(true),
        }
    }
}
