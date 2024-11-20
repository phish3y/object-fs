use std::time::{Duration, SystemTime};

use crate::{adapters, model};

impl adapters::adapter::ObjectAdapter for aws_sdk_s3::Client {

    fn fs_put_object(
        &self, 
        bucket: &str, 
        key: &str
    ) -> Result<(), model::fs::FSError> {
        let req = self.put_object()
            .bucket(bucket)
            .key(key);

        let res = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                req.send().await
            })
        });

        match res {
            Err(err) => {
                return Err(model::fs::FSError{
                    message: format!("failed to put_object at: {}, {}", key, err.to_string())
                });
            }
            Ok(_) => ()
        };

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

            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    req.send().await
                })
            });

            let lo = match res {
                Err(err) => {
                    return Err(model::fs::FSError{
                        message: format!("failed to list_objects at: {}, {}", prefix, err.to_string())
                    });
                }
                Ok(lo) => lo
            };

            for obj in lo.contents() {
                let key = obj.key()
                    .unwrap_or("")
                    .to_string();
                let size = obj.size()
                    .unwrap_or(0);
                let secs = if obj.last_modified().is_some() {
                    obj.last_modified()
                        .unwrap()
                        .secs()
                } else {
                    0
                };
                let nanos = if obj.last_modified().is_some() {
                    obj.last_modified()
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

        return Ok(objects);
    }
}
