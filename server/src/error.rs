use serde_json::{json, Value};
use worker::{console_error, Response};

#[derive(Debug)]
pub struct ApiError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
    pub extra: Option<Value>,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        ApiError {
            status,
            code,
            message: message.into(),
            extra: None,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, "bad_request", message)
    }

    pub fn unauthorized() -> Self {
        Self::new(401, "unauthorized", "missing or invalid token")
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, "not_found", message)
    }

    pub fn method_not_allowed() -> Self {
        Self::new(405, "method_not_allowed", "method not allowed")
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(409, code, message)
    }

    pub fn too_large(message: impl Into<String>) -> Self {
        Self::new(413, "too_large", message)
    }

    pub fn rate_limited() -> Self {
        Self::new(429, "rate_limited", "too many requests, try again soon")
    }

    pub fn internal(detail: impl std::fmt::Display) -> Self {
        console_error!("internal error: {detail}");
        Self::new(500, "internal", "something went wrong")
    }

    pub fn with_extra(mut self, extra: Value) -> Self {
        self.extra = Some(extra);
        self
    }

    pub fn body(&self) -> Value {
        let mut body = json!({ "error": self.code, "message": self.message });
        if let (Some(Value::Object(extra)), Value::Object(map)) = (&self.extra, &mut body) {
            for (k, v) in extra {
                map.insert(k.clone(), v.clone());
            }
        }
        body
    }

    pub fn into_response(self) -> worker::Result<Response> {
        Ok(Response::from_json(&self.body())?.with_status(self.status))
    }
}

impl From<worker::Error> for ApiError {
    fn from(e: worker::Error) -> Self {
        ApiError::internal(e)
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        ApiError::bad_request(format!("invalid json: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_fields_are_merged_beside_the_error() {
        let e = ApiError::conflict("rev_mismatch", "stale").with_extra(json!({ "rev": 4 }));
        let b = e.body();
        assert_eq!(e.status, 409);
        assert_eq!(b["error"], "rev_mismatch");
        assert_eq!(b["rev"], 4);
        assert_eq!(b["message"], "stale");
    }

    #[test]
    fn statuses_match_their_meaning() {
        assert_eq!(ApiError::bad_request("x").status, 400);
        assert_eq!(ApiError::unauthorized().status, 401);
        assert_eq!(ApiError::not_found("x").status, 404);
        assert_eq!(ApiError::too_large("x").status, 413);
        assert_eq!(ApiError::rate_limited().status, 429);
    }
}
