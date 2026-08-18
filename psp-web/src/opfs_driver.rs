use std::cell::RefCell;
use std::sync::Arc;

use async_trait::async_trait;
use js_sys::{Array, Function, Uint8Array};
use psp_db::{DbDriver, DbError, DbRow, DbValue};
use wasm_bindgen::{JsCast, JsValue};

thread_local! {
    pub(crate) static SQL_EXEC: RefCell<Option<Function>> = const { RefCell::new(None) };
    pub(crate) static SQL_QUERY: RefCell<Option<Function>> = const { RefCell::new(None) };
}

/// Real web driver: calls the worker's synchronous sqlite-wasm bridge.
pub struct OpfsSqlDriver;

fn to_js_params(params: &[DbValue]) -> Array {
    let arr = Array::new();
    for value in params {
        let js = match value {
            DbValue::Null => JsValue::NULL,
            DbValue::Integer(i) => JsValue::from_f64(*i as f64),
            DbValue::Real(r) => JsValue::from_f64(*r),
            DbValue::Text(t) => JsValue::from_str(t),
            DbValue::Blob(b) => Uint8Array::from(b.as_slice()).into(),
        };
        arr.push(&js);
    }
    arr
}

fn js_to_dbvalue(js: &JsValue) -> DbValue {
    if js.is_null() || js.is_undefined() {
        DbValue::Null
    } else if let Some(f) = js.as_f64() {
        // JS numbers are f64; integers come back whole. Keep whole numbers as
        // Integer so INTEGER columns decode exactly; DbRow::get_i64 also
        // tolerates a whole Real as a safety net.
        if f.fract() == 0.0 {
            DbValue::Integer(f as i64)
        } else {
            DbValue::Real(f)
        }
    } else if let Some(s) = js.as_string() {
        DbValue::Text(s)
    } else if let Some(arr) = js.dyn_ref::<Uint8Array>() {
        DbValue::Blob(arr.to_vec())
    } else {
        DbValue::Null
    }
}

fn err(context: &str, e: JsValue) -> DbError {
    DbError::Backend(format!(
        "{context}: {}",
        e.as_string().unwrap_or_else(|| format!("{e:?}"))
    ))
}

#[async_trait]
impl DbDriver for OpfsSqlDriver {
    async fn execute(&self, sql: &str, params: &[DbValue]) -> Result<u64, DbError> {
        let f = SQL_EXEC
            .with(|c| c.borrow().clone())
            .ok_or_else(|| DbError::Backend("sql bridge not set".into()))?;
        let n = f
            .call2(
                &JsValue::NULL,
                &JsValue::from_str(sql),
                &to_js_params(params),
            )
            .map_err(|e| err("execute", e))?;
        Ok(n.as_f64().unwrap_or(0.0) as u64)
    }

    async fn query(&self, sql: &str, params: &[DbValue]) -> Result<Vec<DbRow>, DbError> {
        let f = SQL_QUERY
            .with(|c| c.borrow().clone())
            .ok_or_else(|| DbError::Backend("sql bridge not set".into()))?;
        let result = f
            .call2(
                &JsValue::NULL,
                &JsValue::from_str(sql),
                &to_js_params(params),
            )
            .map_err(|e| err("query", e))?;
        let rows = Array::from(&result);
        let mut out = Vec::with_capacity(rows.length() as usize);
        for row in rows.iter() {
            // Each row is an object {col: value}. Keys → cols, values → DbValue.
            let obj: js_sys::Object = row.unchecked_into();
            let keys = js_sys::Object::keys(&obj);
            let cols: Vec<String> = keys.iter().filter_map(|k| k.as_string()).collect();
            let cols = Arc::new(cols);
            let mut vals = Vec::with_capacity(cols.len());
            for key in keys.iter() {
                let v = js_sys::Reflect::get(&obj, &key).map_err(|e| err("row read", e))?;
                vals.push(js_to_dbvalue(&v));
            }
            out.push(DbRow::from_parts(cols.clone(), vals));
        }
        Ok(out)
    }
}
