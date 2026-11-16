//! Kernel retry policies and wait sets; wake payloads and identities stay in
//! JavaScript.
use std::cell::RefCell;

use napi_derive::napi;
use status_machina_bridge::retry::{
    Failure,
    Policy,
    Waits,
};

use crate::error::encode;
use crate::write;

fn failure(failure: Failure) -> napi::Error {
    napi::Error::new(napi::Status::GenericFailure, failure.encode())
}

/// One kernel-validated `RetryPolicy`.
#[napi]
pub struct NativeRetryPolicy {
    policy: Policy,
}
#[napi]
impl NativeRetryPolicy {
    #[napi(constructor)]
    pub fn new(source: String) -> napi::Result<Self> {
        Ok(Self {
            policy: Policy::parse(&source).map_err(failure)?,
        })
    }
    #[napi]
    pub fn schedule(&self, source: String) -> napi::Result<String> {
        Ok(encode(&self.policy.schedule(&source).map_err(failure)?))
    }
}

/// One instance's kernel `RetryWaits`.
#[napi]
pub struct NativeRetryWaits {
    waits: RefCell<Waits>,
}
#[napi]
impl NativeRetryWaits {
    /// `None` is a capacity JavaScript cannot represent as `u32`.
    #[napi(constructor)]
    pub fn new(capacity: Option<u32>) -> napi::Result<Self> {
        Ok(Self {
            waits: RefCell::new(
                Waits::try_new(capacity.map(|value| value as usize)).map_err(failure)?,
            ),
        })
    }
    /// An independent copy of the kernel value, for candidate snapshots.
    #[napi]
    pub fn duplicate(&self) -> napi::Result<NativeRetryWaits> {
        Ok(Self {
            waits: RefCell::new(write(&self.waits)?.clone()),
        })
    }
    #[napi]
    pub fn schedule(&self, source: String) -> napi::Result<String> {
        write(&self.waits)?.schedule(&source).map_err(failure)
    }
    #[napi]
    pub fn pending(&self) -> napi::Result<String> {
        Ok(encode(&write(&self.waits)?.pending()))
    }
    #[napi]
    pub fn get(&self, operation: Option<u32>) -> napi::Result<String> {
        Ok(encode(&write(&self.waits)?.get(operation)))
    }
    #[napi]
    pub fn mark_registered(&self, source: String) -> napi::Result<bool> {
        write(&self.waits)?
            .mark_registered(&source)
            .map_err(failure)
    }
    #[napi]
    pub fn is_due(&self, source: String) -> napi::Result<bool> {
        write(&self.waits)?.is_due(&source).map_err(failure)
    }
    #[napi]
    pub fn consume(&self, source: String) -> napi::Result<String> {
        Ok(encode(
            &write(&self.waits)?.consume(&source).map_err(failure)?,
        ))
    }
    #[napi]
    pub fn cancel(&self, operation: Option<u32>) -> napi::Result<String> {
        Ok(encode(&write(&self.waits)?.cancel(operation)))
    }
}
