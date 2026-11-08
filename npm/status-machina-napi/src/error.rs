//! Retain the exact JavaScript exception object until it becomes the public
//! cause.
use std::cell::RefCell;
use std::fmt;

use napi::{
    Error,
    Status,
};

use crate::host::JsHost;

pub(crate) type CoreError = status_machina_bridge::error::Error<JsHost>;
#[derive(Debug)]
pub(crate) struct JsCause {
    message: String,
    original: RefCell<Option<Error>>,
}
impl JsCause {
    pub(crate) fn new(error: Error) -> Self {
        Self {
            message: error.reason.clone(),
            original: RefCell::new(Some(error)),
        }
    }
}
impl fmt::Display for JsCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for JsCause {}
pub(crate) fn translate(error: CoreError) -> Error {
    let metadata = serde_json::to_string(&error.failure())
        .expect("typed native failure metadata is serializable");
    let mut translated = Error::new(Status::GenericFailure, metadata);
    if let Some(source) = error
        .cause()
        .and_then(|e| e.cause())
        .and_then(|e| e.downcast_ref::<JsCause>())
    {
        translated.set_cause(
            source
                .original
                .borrow_mut()
                .take()
                .expect("native exception is translated exactly once"),
        );
    }
    translated
}
pub(crate) fn encode<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("typed native result metadata is serializable")
}
