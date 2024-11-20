use std::time::{Duration, SystemTime};

use google_cloud_storage::http::objects::{
    list::ListObjectsRequest, 
    upload::{Media, UploadObjectRequest, UploadType}
};

use crate::{adapters, model};

impl adapters::adapter::ObjectAdapter for google_cloud_storage::client::Client {

    fn fs_put_object(
        &self, 
        bucket: &str, 
        key: &str
    ) -> Result<(), model::fs::FSError> {
        let req = UploadObjectRequest {
            bucket: bucket.to_string(),
            ..Default::default()
        };
        
        let res = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.upload_object(
                    &req, 
                    "".as_bytes(), 
                    &UploadType::Simple(Media::new(key.to_string()))
                ).await
            })
        });

        match res {
            Err(err) => {
                return Err(model::fs::FSError{
                    message: format!("failed to put_object at: {}, {}", key, err.to_string())
                });
            }
            Ok(_) => ()
        }

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
            let req = ListObjectsRequest {
                bucket: bucket.to_string(),
                prefix: Some(prefix.to_string()),
                page_token: continuation_token.clone(),
                ..Default::default()
            };

            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    self.list_objects(&req).await
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

            if let Some(objs) = lo.items {
                for obj in objs {
                    let modified_time = SystemTime::UNIX_EPOCH + 
                        Duration::from_secs(
                            obj.updated.unwrap_or(time::OffsetDateTime::now_utc()).unix_timestamp() as u64
                        );

                    objects.push(model::fs::FSObject{
                        key: obj.name,
                        size: obj.size,
                        modified_time
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
}