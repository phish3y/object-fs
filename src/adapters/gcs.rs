use std::time::{Duration, SystemTime};

use google_cloud_storage::http::objects::{
    download::Range, get::GetObjectRequest, list::ListObjectsRequest, upload::{Media, UploadObjectRequest, UploadType}
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
        
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.upload_object(
                    &req, 
                    "".as_bytes(), 
                    &UploadType::Simple(Media::new(key.to_string()))
                ).await
            })
        }).map_err(|err|
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
            let req = ListObjectsRequest {
                bucket: bucket.to_string(),
                prefix: Some(prefix.to_string()),
                page_token: continuation_token.clone(),
                ..Default::default()
            };

            let lo = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    self.list_objects(&req).await
                })
            }).map_err(|err|
                model::fs::FSError{
                    message: format!("failed to list_objects at: {}, {}", prefix, err.to_string())
                }
            )?;

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

    fn fs_head_object(
        &self,
        bucket: &str,
        key: &str
    ) -> Result<model::fs::FSObject, model::fs::FSError> {
        let req = GetObjectRequest{
            bucket: bucket.to_string(),
            object: key.to_string(),
            ..Default::default()
        };

        let o = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.get_object(&req).await
            })
        }).map_err(|err|
            model::fs::FSError{
                message: format!("failed to get_object: {}, {}", key, err.to_string())
            }
        )?;

        let modified_time = SystemTime::UNIX_EPOCH + 
        Duration::from_secs(
            o.updated.unwrap_or(time::OffsetDateTime::now_utc()).unix_timestamp() as u64
        );

        Ok(
            model::fs::FSObject{
                key: o.name,
                size: o.size,
                modified_time
            }
        )
    }

    fn fs_download_object(
            &self,
            bucket: &str,
            key: &str
        ) -> Result<Vec<u8>, model::fs::FSError> {
        let req = GetObjectRequest{
            bucket: bucket.to_string(),
            object: key.to_string(),
            ..Default::default()
        };

        let bytes = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.download_object(&req, &Range::default()).await
            })
        }).map_err(|err|
            model::fs::FSError{
                message: format!("failed to get_object: {}, {}", key, err.to_string())
            }
        )?;

        Ok(bytes)
    }
}