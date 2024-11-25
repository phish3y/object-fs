use crate::model;

pub trait ObjectAdapter {
    fn fs_put_object(
        &self,
        bucket: &str,
        key: &str,
        body: Option<Vec<u8>>,
    ) -> Result<(), model::fs::FSError>;

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
    ) -> Result<Option<Vec<u8>>, model::fs::FSError>;
}
