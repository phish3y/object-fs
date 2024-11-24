#[derive(Clone, Debug)]
pub struct FSNode {
    pub attr: fuser::FileAttr,
    pub key: String,
    pub parent: u64,
}

pub struct FSObject {
    pub key: String,
    pub size: i64,
    pub modified_time: std::time::SystemTime,
}

#[derive(Debug)]
pub struct FSError {
    pub message: String,
}

impl core::fmt::Display for FSError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for FSError {
    fn description(&self) -> &str {
        &self.message
    }
}
