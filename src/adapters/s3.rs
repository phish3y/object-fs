use std::time::{Duration, SystemTime};

use crate::{adapters, model, util};

impl adapters::adapter::ObjectAdapter for aws_sdk_s3::Client {

    fn fs_put_object(
        &self, 
        bucket: &str, 
        key: &str
    ) -> Result<(), model::fs::FSError> {
        let req = self.put_object()
            .bucket(bucket)
            .key(key);

        util::fs::poll_until_ready_error(
            req.send()
        ).map_err(|err|
            model::fs::FSError{
                message: format!("failed to put_object at: {}, {}", key, err.to_string())
            }
        )?;

        Ok(())
    }

    fn fs_list_objects(
        &self,
        bucket: &str,
        prefix: &str
    ) -> Result<Vec<model::fs::FSObject>, model::fs::FSError> {
        let mut objects = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let mut req = self.list_objects_v2()
                .bucket(bucket)
                .prefix(prefix);

            if let Some(tok) = continuation_token {
                req = req.continuation_token(tok);
            }

            let lo = util::fs::poll_until_ready_error(
                req.send()
            ).map_err(|err|
                model::fs::FSError{
                    message: format!("failed to list_objects at: {}, {}", prefix, err.to_string())
                }
            )?;

            for o in lo.contents() {
                let key = o.key()
                    .unwrap_or("")
                    .to_string();
                let size = o.size()
                    .unwrap_or(0);
                let secs = if o.last_modified().is_some() {
                    o.last_modified()
                        .unwrap()
                        .secs()
                } else {
                    0
                };
                let nanos = if o.last_modified().is_some() {
                    o.last_modified()
                    .unwrap()
                    .subsec_nanos()
                } else {
                    0
                };
                let modified_time = SystemTime::UNIX_EPOCH + Duration::new(secs as u64, nanos);

                objects.push(
                    model::fs::FSObject{
                        key,
                        size,
                        modified_time,
                    }
                );
            }

            continuation_token = lo.next_continuation_token()
                .map(|tok| tok.to_string());
            if continuation_token.is_none() {
                break;
            }
        }

        Ok(objects)
    }

    fn fs_head_object(
        &self,
        bucket: &str,
        key: &str
    ) -> Result<model::fs::FSObject, model::fs::FSError> {
        let req = self.head_object()
            .bucket(bucket)
            .key(key);

        let ho = util::fs::poll_until_ready_error(
            req.send()
        ).map_err(|err|
            model::fs::FSError{
                message: format!("failed to head_object: {}, {}", key, err.to_string())
            }
        )?;

        let secs = if ho.last_modified().is_some() {
            ho.last_modified()
                .unwrap()
                .secs()
        } else {
            0
        };
        let nanos = if ho.last_modified().is_some() {
            ho.last_modified()
            .unwrap()
            .subsec_nanos()
        } else {
            0
        };
        let modified_time = SystemTime::UNIX_EPOCH + Duration::new(secs as u64, nanos);

        Ok(
            model::fs::FSObject{
                key: key.to_string(),
                size: ho.content_length().unwrap_or(0),
                modified_time
            }
        )
    }

    fn fs_download_object(
        &self,
        bucket: &str,
        key: &str
    ) -> Result<Vec<u8>, model::fs::FSError> {
        let req = self.get_object()
            .bucket(bucket)
            .key(key);

        let o = util::fs::poll_until_ready_error(
            req.send()
        ).map_err(|err| 
            model::fs::FSError{
                message: format!("failed to get_object: {}, {}", key, err.to_string())
            }
        )?;
    
        let bytes = util::fs::poll_until_ready_error(
            o.body.collect()
        ).map_err(|err|
            model::fs::FSError{
                message: format!("failed to collect body: {}, {}", key, err.to_string())
            }
        )?;

        Ok(bytes.into_bytes().to_vec())
    }
}
