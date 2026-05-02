use std::sync::Arc;
use serde::de::DeserializeOwned;
use serde::Serialize;
use worker::{Env, KvStore};

use crate::error::AppError;

pub struct KvCache {
    kv: KvStore,
}

impl KvCache {
    pub fn new(env: &Arc<Env>) -> Result<Self, AppError> {
        let kv = env.kv("FUFU_KV")?;
        Ok(Self { kv })
    }

    pub async fn get_str(&self, key: &str) -> Result<Option<String>, AppError> {
        Ok(self.kv.get(key).text().await?)
    }

    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, AppError> {
        let val: Option<String> = self.kv.get(key).text().await?;
        match val {
            Some(s) => Ok(Some(serde_json::from_str(&s)?)),
            None => Ok(None),
        }
    }

    pub async fn put_str(&self, key: &str, value: &str, ttl: u64) -> Result<(), AppError> {
        self.kv.put(key, value)?.expiration_ttl(ttl).execute().await?;
        Ok(())
    }

    pub async fn put_json<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl: u64,
    ) -> Result<(), AppError> {
        let json = serde_json::to_string(value)?;
        self.kv.put(key, json)?.expiration_ttl(ttl).execute().await?;
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<(), AppError> {
        self.kv.delete(key).await?;
        Ok(())
    }
}
