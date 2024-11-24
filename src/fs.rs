use std::{collections::HashMap, sync::Mutex, time::SystemTime};

use fuser::FileAttr;

use crate::{adapters, model};

const ROOT_INO: u64 = 1;

pub struct ObjectFS {
    pub client: Box<dyn adapters::adapter::ObjectAdapter>,
    pub bucket: String,
    pub current_ino: Mutex<u64>,
    pub ino_to_node: Mutex<HashMap<u64, model::fs::FSNode>>,
    pub key_to_node: Mutex<HashMap<String, model::fs::FSNode>>,
}

impl ObjectFS {
    pub fn new(client: Box<dyn adapters::adapter::ObjectAdapter>, bucket: &str) -> Self {
        let mut ino_to_node = HashMap::new();
        let root_node = model::fs::FSNode {
            attr: FileAttr {
                ino: ROOT_INO,
                size: 0,
                blksize: 0,
                blocks: 0,
                atime: SystemTime::now(),
                mtime: SystemTime::now(),
                ctime: SystemTime::now(),
                crtime: SystemTime::now(),
                kind: fuser::FileType::Directory,
                perm: 0o755,
                nlink: 2,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
            },
            key: "".to_string(),
            parent: 0,
        };

        ino_to_node.insert(ROOT_INO, root_node.clone());

        let mut key_to_node = HashMap::new();
        key_to_node.insert("".to_string(), root_node);

        Self {
            client,
            bucket: bucket.to_string(),
            current_ino: ROOT_INO.into(),
            ino_to_node: Mutex::new(ino_to_node),
            key_to_node: Mutex::new(key_to_node),
        }
    }

    fn next_ino(&self) -> u64 {
        let mut cur_ino = self
            .current_ino
            .lock()
            .expect("failed to acquire `current_ino` guard");
        *cur_ino += 1;

        return *cur_ino;
    }

    pub fn index_object(&self, object: &model::fs::FSObject) {
        let mut components = Vec::new();
        let mut maybe_component = Some(object.key.clone());
        while let Some(component) = maybe_component {
            components.push(component.clone());
            maybe_component = self.get_parent(&component);
        }

        components.reverse();

        let mut parent_ino = ROOT_INO;
        for component in components {
            parent_ino = if component == object.key {
                self.index_file(
                    &model::fs::FSObject {
                        key: component,
                        size: object.size,
                        modified_time: object.modified_time,
                    },
                    parent_ino,
                )
            } else {
                self.index_directory(
                    &model::fs::FSObject {
                        key: component,
                        size: object.size,
                        modified_time: object.modified_time,
                    },
                    parent_ino,
                )
            }
        }
    }

    fn index_file(&self, object: &model::fs::FSObject, parent: u64) -> u64 {
        if self
            .key_to_node
            .lock()
            .expect("failed to acquire `key_to_node` guard")
            .get(&object.key)
            .is_some()
        {
            return self
                .key_to_node
                .lock()
                .expect("failed to acquire `key_to_node` guard")
                .get(&object.key)
                .unwrap()
                .attr
                .ino;
        }

        let ino = self.next_ino();
        let node = model::fs::FSNode {
            attr: FileAttr {
                ino,
                size: object.size as u64,
                blksize: 0, // TODO
                blocks: 0,  // TODO
                atime: object.modified_time,
                mtime: SystemTime::now(),
                ctime: SystemTime::now(),
                crtime: SystemTime::now(),
                kind: fuser::FileType::RegularFile,
                perm: 0o755,
                nlink: 1,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
            },
            key: object.key.clone(),
            parent,
        };

        self.ino_to_node
            .lock()
            .expect("failed to acquire `ino_to_node` guard")
            .insert(ino, node.clone());

        self.key_to_node
            .lock()
            .expect("failed to acquire `key_to_node` guard")
            .insert(object.key.clone(), node);

        return ino;
    }

    fn index_directory(&self, object: &model::fs::FSObject, parent: u64) -> u64 {
        let key = if object.key.ends_with('/') {
            &object.key[..object.key.len() - 1]
        } else {
            &object.key
        };

        if self
            .key_to_node
            .lock()
            .expect("failed to acquire `key_to_node` guard")
            .get(key)
            .is_some()
        {
            return self
                .key_to_node
                .lock()
                .expect("failed to acquire `key_to_node` guard")
                .get(key)
                .unwrap()
                .attr
                .ino;
        }

        let ino = self.next_ino();
        let node = model::fs::FSNode {
            attr: FileAttr {
                ino,
                size: object.size as u64,
                blksize: 0, // TODO
                blocks: 0,  // TODO
                atime: object.modified_time,
                mtime: SystemTime::now(),
                ctime: SystemTime::now(),
                crtime: SystemTime::now(),
                kind: fuser::FileType::Directory,
                perm: 0o755,
                nlink: 1,
                uid: 0,
                gid: 0,
                rdev: 0,
                flags: 0,
            },
            key: object.key.clone(),
            parent,
        };

        self.ino_to_node
            .lock()
            .expect("failed to acquire `ino_to_node` lock")
            .insert(ino, node.clone());

        self.key_to_node
            .lock()
            .expect("failed to acquire `key_to_node` lock")
            .insert(object.key.clone(), node);

        return ino;
    }

    fn get_parent(&self, path: &str) -> Option<String> {
        let path = if path.ends_with('/') {
            &path[..path.len() - 1]
        } else {
            path
        };

        match path.rfind('/') {
            Some(pos) if pos > 0 => Some(path[..pos].to_string()),
            _ => None,
        }
    }

    pub fn get_root_attr(&self) -> FileAttr {
        self.ino_to_node
            .lock()
            .expect("failed to acquire `ino_to_node` guard")
            .get(&0)
            .expect("no root file attribute found")
            .attr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_ino() {
        let client = adapters::mock::MockClient {};
        let fs = ObjectFS::new(Box::new(client), "dummy-bucket");

        let cases = vec![2, 3];

        for expected in cases {
            let result = fs.next_ino();
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_index_object() {
        let client = adapters::mock::MockClient {};
        let fs = ObjectFS::new(Box::new(client), "dummy-bucket");

        let cases = vec![
            ("file", 10, SystemTime::now(), 2),
            ("folder/file", 5, SystemTime::now(), 4),
            ("folder/subfolder/file", 5, SystemTime::now(), 6),
        ];

        for (key, size, modified_time, expected_count) in cases {
            fs.index_object(&model::fs::FSObject {
                key: key.to_string(),
                size,
                modified_time,
            });
            let result = fs.key_to_node.lock().unwrap();

            assert_eq!(
                result.keys().len(),
                expected_count,
                "failed index count for case: {}",
                key
            );
        }
    }

    #[test]
    fn test_index_file() {
        let cases = vec![
            ("/file", 10, SystemTime::now(), 1),
            ("folder/file", 0, SystemTime::now(), 5),
            ("folder/subfolder/file", 0, SystemTime::UNIX_EPOCH, 7),
        ];

        for (key, size, modified_time, parent) in cases {
            let fs = ObjectFS::new(Box::new(adapters::mock::MockClient {}), "dummy-bucket");

            let ino = fs.index_file(
                &model::fs::FSObject {
                    key: key.to_string(),
                    size,
                    modified_time,
                },
                parent,
            );

            let guard = fs.ino_to_node.lock().unwrap();
            let result = guard.get(&2).unwrap();

            assert_eq!(ino, 2, "failed on `ino` for case: {}", key);
            assert_eq!(result.attr.ino, 2, "failed on `attr.ino` for case: {}", key);
            assert_eq!(
                result.parent, parent,
                "failed on `parent` for case: {}",
                key
            );
            assert_eq!(result.key, key, "failed on `key` for case: {}", key);
            assert_eq!(
                result.attr.size, size as u64,
                "failed on `attr.size` for case: {}",
                key
            );
            assert_eq!(
                result.attr.atime, modified_time,
                "failed on `attr.atime` for case: {}",
                key
            );
        }
    }

    #[test]
    fn test_index_directory() {
        let cases = vec![
            ("folder", SystemTime::UNIX_EPOCH, 1),
            ("folder/", SystemTime::now(), 5),
            ("folder/subfolder/", SystemTime::UNIX_EPOCH, 7),
        ];

        for (key, modified_time, parent) in cases {
            let fs = ObjectFS::new(Box::new(adapters::mock::MockClient {}), "dummy-bucket");

            let ino = fs.index_directory(
                &model::fs::FSObject {
                    key: key.to_string(),
                    size: 0,
                    modified_time,
                },
                parent,
            );

            let guard = fs.ino_to_node.lock().unwrap();
            let result = guard.get(&2).unwrap();

            assert_eq!(ino, 2, "failed on `ino` for case: {}", key);
            assert_eq!(result.attr.ino, 2, "failed on `attr.ino` for case: {}", key);
            assert_eq!(
                result.parent, parent,
                "failed on `parent` for case: {}",
                key
            );
            assert_eq!(result.key, key, "failed on `key` for case: {}", key);
            assert_eq!(
                result.attr.atime, modified_time,
                "failed on `attr.atime` for case: {}",
                key
            );
        }
    }

    #[test]
    fn test_get_parent() {
        let client = adapters::mock::MockClient {};
        let fs = ObjectFS::new(Box::new(client), "dummy-bucket");

        let cases = vec![
            ("folder/file", Some("folder".to_string())),
            (
                "folder/subfolder/file",
                Some("folder/subfolder".to_string()),
            ),
            ("file", None),
            ("folder/", None),
        ];

        for (input, expected) in cases {
            let result = fs.get_parent(input);
            assert_eq!(result, expected, "failed for case: {}", input);
        }
    }

    #[test]
    fn test_get_root_attr() {
        let client = adapters::mock::MockClient {};
        let fs = ObjectFS::new(Box::new(client), "dummy-bucket");

        let root_attr = fs.get_root_attr();

        assert_eq!(root_attr.ino, 1, "expected root attr ino to be 1");
    }
}
