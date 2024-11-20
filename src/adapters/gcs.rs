use crate::{adapters, model};

impl adapters::adapter::ObjectAdapter for google_cloud_storage::client::Client {

    fn put_object(
        &self, 
        bucket: &str, 
        key: &str
    ) -> Result<(), model::fs::FSError> {
        // TODO
        Ok(())
    }

    fn list_objects(
        &self,
        bucket: &str,
        prefix: &str
    ) -> Result<Vec<model::fs::FSObject>, model::fs::FSError> {
        // TODO
        Ok(Vec::new())
    }
}