use std::{future::{self, Future}, pin::Pin};

use aws_sdk_s3::{
    error::SdkError, 
    operation::{
        head_object::{HeadObjectError, HeadObjectOutput}, 
        list_objects_v2::{ListObjectsV2Error, ListObjectsV2Output}, 
        put_object::{PutObjectError, PutObjectOutput}
    }
};

pub trait ObjectFSS3 {

    fn put_object(
        &self, 
        bucket: &str, 
        key: &str
    ) -> Pin<Box<dyn Future<Output = Result<PutObjectOutput, SdkError<PutObjectError>>> + Send>>;

    fn list_objects_v2(
        &self,
        bucket: &str,
        prefix: &str,
        continuation_token: Option<String>
    ) -> Pin<Box<dyn Future<Output = Result<ListObjectsV2Output, SdkError<ListObjectsV2Error>>> + Send>>;

    fn head_object(
        &self,
        bucket: &str,
        key: &str
    ) -> Pin<Box<dyn Future<Output = Result<HeadObjectOutput, SdkError<HeadObjectError>>> + Send>>;
}

impl ObjectFSS3 for aws_sdk_s3::Client {

    fn put_object(
        &self, 
        bucket: 
        &str, 
        key: &str
    ) -> Pin<Box<dyn Future<Output = Result<PutObjectOutput, SdkError<PutObjectError>>> + Send>> {
        Box::pin(
            self.put_object()
                .bucket(bucket)
                .key(key)
                .send()
        )
    }

    fn list_objects_v2(
            &self,
            bucket: &str,
            prefix: &str,
            continuation_token: Option<String>
        ) -> Pin<Box<dyn Future<Output = Result<ListObjectsV2Output, SdkError<ListObjectsV2Error>>> + Send>> {
        let mut req = self.list_objects_v2()
            .bucket(bucket)
            .prefix(prefix);

        if let Some(token) = continuation_token {
            req = req.continuation_token(token);
        }

        Box::pin(
            req.send()
        )
    }

    fn head_object(
            &self,
            bucket: &str,
            key: &str
        ) -> Pin<Box<dyn Future<Output = Result<HeadObjectOutput, SdkError<HeadObjectError>>> + Send>> {
        Box::pin(
            self.head_object()
                .bucket(bucket)
                .key(key)
                .send()
        )
    }
}

pub struct MockS3Client{}

impl ObjectFSS3 for MockS3Client {

    fn put_object(
        &self, 
        _bucket: &str, 
        _key: &str
    ) -> Pin<Box<dyn Future<Output = Result<PutObjectOutput, SdkError<PutObjectError>>> + Send>> {
        Box::pin(
            future::ready(
                Ok(PutObjectOutput::builder().build())
            )
        )
    }

    fn list_objects_v2(
            &self,
            _bucket: &str,
            _prefix: &str,
            _continuation_token: Option<String>
        ) -> Pin<Box<dyn Future<Output = Result<ListObjectsV2Output, SdkError<ListObjectsV2Error>>> + Send>> {
        Box::pin(
            future::ready(
                Ok(ListObjectsV2Output::builder().build())
            )
        )
    }

    fn head_object(
            &self,
            _bucket: &str,
            _key: &str
        ) -> Pin<Box<dyn Future<Output = Result<HeadObjectOutput, SdkError<HeadObjectError>>> + Send>> {
        Box::pin(
            future::ready(
                Ok(HeadObjectOutput::builder().build())
            )
        )
    }
}
