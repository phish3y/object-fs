use crate::model;

pub trait ObjectAdapter {

    fn put_object(
        &self, 
        bucket: &str, 
        key: &str
    ) -> Result<(), model::fs::FSError>;

    fn list_objects(
        &self,
        bucket: &str,
        prefix: &str
    ) -> Result<Vec<model::fs::FSObject>, model::fs::FSError>;
}