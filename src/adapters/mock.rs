use crate::{adapters, model};

pub struct MockClient {}

impl adapters::Object for MockClient {
    fn fs_put_object(
        &self,
        _bucket: &str,
        _key: &str,
        _body: Option<Vec<u8>>,
    ) -> Result<(), model::fs::FSError> {
        Ok(())
    }

    fn fs_list_objects(
        &self,
        _bucket: &str,
        _prefix: &str,
    ) -> Result<Vec<model::fs::FSObject>, model::fs::FSError> {
        Ok(Vec::new())
    }

    fn fs_download_object(
        &self,
        _bucket: &str,
        _key: &str,
        _range: Option<(u64, u64)>,
    ) -> Result<Option<Vec<u8>>, model::fs::FSError> {
        Ok(Some(Vec::new()))
    }
}
