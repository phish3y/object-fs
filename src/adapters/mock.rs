use crate::{adapters, model};

pub struct MockS3Client{}

impl adapters::adapter::ObjectAdapter for MockS3Client {

    fn put_object(
        &self, 
        _bucket: &str, 
        _key: &str
    ) -> Result<(), model::fs::FSError> {
        Ok(())
    }

    fn list_objects(
        &self,
        _bucket: &str,
        _prefix: &str
    ) -> Result<Vec<model::fs::FSObject>, model::fs::FSError> {
        Ok(Vec::new())
    }
}
