use crate::error::{ApiError, ApiResult};
use serde::de::DeserializeOwned;
use worker::wasm_bindgen::JsValue;
use worker::{D1Database, D1PreparedStatement};

pub fn n(v: i64) -> JsValue {
    JsValue::from_f64(v as f64)
}

pub fn s(v: &str) -> JsValue {
    JsValue::from_str(v)
}

pub struct Db(pub D1Database);

impl Db {
    pub fn stmt(&self, sql: &str, args: &[JsValue]) -> ApiResult<D1PreparedStatement> {
        Ok(self.0.prepare(sql).bind(args)?)
    }

    pub async fn all<T: DeserializeOwned>(&self, sql: &str, args: &[JsValue]) -> ApiResult<Vec<T>> {
        Ok(self.stmt(sql, args)?.all().await?.results::<T>()?)
    }

    pub async fn one<T: DeserializeOwned>(
        &self,
        sql: &str,
        args: &[JsValue],
    ) -> ApiResult<Option<T>> {
        Ok(self.all::<T>(sql, args).await?.into_iter().next())
    }

    pub async fn run(&self, sql: &str, args: &[JsValue]) -> ApiResult<usize> {
        let result = self.stmt(sql, args)?.run().await?;
        Ok(changes(&result))
    }

    pub async fn batch(&self, stmts: Vec<D1PreparedStatement>) -> ApiResult<Vec<usize>> {
        let results = self.0.batch(stmts).await?;
        for r in &results {
            if let Some(e) = r.error() {
                return Err(ApiError::internal(e));
            }
        }
        Ok(results.iter().map(changes).collect())
    }
}

fn changes(r: &worker::D1Result) -> usize {
    r.meta().ok().flatten().and_then(|m| m.changes).unwrap_or(0)
}
