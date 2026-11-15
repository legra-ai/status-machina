//! Native composition through the kernel's `InstanceRegistry`, `Composition`,
//! `CapabilitySnapshot` and `BoundComposition`.
//!
//! Each language keeps a type-qualified table of native instance addresses and
//! lowers routes, batches, observations and deliveries to indexes into it. The
//! kernel decides every registration, routing, binding and propagation rule;
//! the bridge only resolves its results back to those indexes.
mod failure;
mod propagation;
mod report;
pub mod spec;
mod topology;

pub use failure::{
    Failure,
    Fault,
};
pub use propagation::{
    Propagated,
    limits,
};
pub use report::{
    Item,
    Report,
};
pub use topology::{
    Snapshot,
    Topology,
};
