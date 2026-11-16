//! Native scenario replay through the kernel's `ScenarioLimits`,
//! `ScenarioSet`, `ScenarioAssertion` and `ScenarioReplay`.
//!
//! Each language keeps its scenario identities, events, payloads, facts and
//! assertion callbacks. Scenarios cross as positions, events and states as
//! definition-scoped indexes and values as arena keys. The kernel decides
//! every limit, set bound, assertion order, dispatch and trace rule; the
//! bridge only resolves its report back to those indexes.
mod failure;
mod ids;
mod limits;
mod replay;
mod report;
mod set;

pub use failure::{
    Failure,
    Fault,
};
pub use limits::{
    Limits,
    check_assertion,
};
pub use report::Report;
pub use set::check;
