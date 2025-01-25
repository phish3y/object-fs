pub mod gcs;
pub mod mock;
pub mod s3;

use crate::model;

pub trait Object {
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

    fn fs_download_object(
        &self,
        bucket: &str,
        key: &str,
        range: Option<(u64, u64)>,
    ) -> Result<Option<Vec<u8>>, model::fs::FSError>;
}
