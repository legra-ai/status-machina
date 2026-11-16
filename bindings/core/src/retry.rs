//! Native retry policies and correlated waits through the kernel's
//! `RetryPolicy` and `RetryWaits`.
//!
//! Durations and civil instants cross as decimal whole seconds plus
//! nanoseconds, so neither language loses precision through JSON numbers. The
//! bridge decides which native values the kernel types can represent; the
//! kernel decides every policy, schedule, replacement, matching and
//! consumption rule. Each language interns operation identities and keeps
//! payloads keyed by the kernel-assigned generation; nothing is scheduled.
mod failure;
mod ids;
mod policy;
mod time;
mod waits;

pub use failure::Failure;
pub use policy::Policy;
pub use time::{
    DurationValue,
    InstantValue,
};
pub use waits::{
    Correlation,
    Intent,
    Waits,
};
