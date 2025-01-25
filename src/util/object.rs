use crate::model::fs::FSError;

pub enum Provider {
    AWS,
    GCS,
}

impl Provider {
    pub fn is_aws(&self) -> bool {
        matches!(self, Provider::AWS)
    }

    pub fn _is_gcs(&self) -> bool {
        matches!(self, Provider::GCS)
    }
}

pub fn parse_provider_from_uri(bucket_uri: &str) -> Result<Provider, FSError> {
    return if bucket_uri.starts_with("s3://") {
        Ok(Provider::AWS)
    } else if bucket_uri.starts_with("gs://") {
        Ok(Provider::GCS)
    } else {
        Err(FSError {
            message: format!("failed to parse provider of: {}", bucket_uri),
        })
    };
}

pub fn parse_bucket_from_uri(bucket_uri: &str) -> &str {
    bucket_uri.split_once("://").map(|(_, rest)| rest).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_provider() {
        assert!(matches!(parse_provider_from_uri("s3://bucket"), Ok(Provider::AWS)));
        assert!(matches!(parse_provider_from_uri("gs://bucket"), Ok(Provider::GCS)));
        assert!(matches!(parse_provider_from_uri("ftp://bucket"), Err(_)));
    }

    #[test]
    fn test_parse_bucket() {
        assert!(matches!(parse_bucket_from_uri("s3://bucket"), "bucket"));
        assert!(matches!(parse_bucket_from_uri("gs://bucket"), "bucket"));
        assert!(matches!(parse_bucket_from_uri("bucket"), ""));
    }
}