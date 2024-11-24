use std::time::SystemTime;

use crate::{adapters, model};

pub struct MockClient {}

impl adapters::adapter::ObjectAdapter for MockClient {
    fn fs_put_object(&self, _bucket: &str, _key: &str) -> Result<(), model::fs::FSError> {
        Ok(())
    }

    fn fs_list_objects(
        &self,
        _bucket: &str,
        _prefix: &str,
    ) -> Result<Vec<model::fs::FSObject>, model::fs::FSError> {
        Ok(Vec::new())
    }

    fn fs_head_object(
        &self,
        _bucket: &str,
        key: &str,
    ) -> Result<Option<model::fs::FSObject>, model::fs::FSError> {
        Ok(Some(model::fs::FSObject {
            key: key.to_string(),
            size: 0,
            modified_time: SystemTime::now(),
        }))
    }

    fn fs_download_object(&self, _bucket: &str, _key: &str) -> Result<Vec<u8>, model::fs::FSError> {
        Ok(Vec::new())
    }
}
