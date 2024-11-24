use crate::model;

pub trait ObjectAdapter {
    fn fs_put_object(&self, bucket: &str, key: &str) -> Result<(), model::fs::FSError>;

    fn fs_list_objects(
        &self,
        bucket: &str,
        prefix: &str,
    ) -> Result<Vec<model::fs::FSObject>, model::fs::FSError>;

    fn fs_head_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<Option<model::fs::FSObject>, model::fs::FSError>;

    fn fs_download_object(
        &self,
        bucket: &str,
        key: &str,
        range: Option<(u64, u64)>,
    ) -> Result<Vec<u8>, model::fs::FSError>;
}
