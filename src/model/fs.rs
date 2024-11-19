use fuser::FileAttr;

#[derive(Clone, Debug)]
pub struct Node {
    pub attr: FileAttr,
    pub key: String,
    pub parent: u64,
}