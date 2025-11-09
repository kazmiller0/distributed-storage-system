use super::error::MPTError;
use super::node::Database;
use rocksdb::{DB, Options};
use std::path::Path;

pub struct RocksDbAdapter {
    db: DB,
}

impl RocksDbAdapter {
    pub fn open(path: &Path) -> Result<Self, MPTError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path)
            .map_err(|e| MPTError::DatabaseError(e.to_string()))?;
        Ok(Self { db })
    }
}

impl Database for RocksDbAdapter {
    fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>, MPTError> {
        match self.db.get(key) {
            Ok(Some(v)) => Ok(Some(v)),
            Ok(None) => Ok(None),
            Err(e) => Err(MPTError::DatabaseError(e.to_string())),
        }
    }

    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), MPTError> {
        self.db
            .put(key, value)
            .map_err(|e| MPTError::DatabaseError(e.to_string()))
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), MPTError> {
        self.db
            .delete(key)
            .map_err(|e| MPTError::DatabaseError(e.to_string()))
    }
}

/// 内存数据库（用于测试）
pub struct MemoryDatabase {
    data: std::collections::HashMap<Vec<u8>, Vec<u8>>,
}

impl MemoryDatabase {
    pub fn new() -> Self {
        Self {
            data: std::collections::HashMap::new(),
        }
    }
}

impl Default for MemoryDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl Database for MemoryDatabase {
    fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>, MPTError> {
        Ok(self.data.get(key).cloned())
    }

    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), MPTError> {
        self.data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), MPTError> {
        self.data.remove(key);
        Ok(())
    }
}
