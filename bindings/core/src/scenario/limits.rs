//! Kernel-validated replay limits and assertion predicates.
use serde::{
    Deserialize,
    Serialize,
};
use status_machina::{
    ScenarioAssertion,
    ScenarioLimits,
};

use super::failure::{
    Failure,
    parse,
};
use super::ids::CountIds;

/// Absent fields keep the kernel's conservative defaults.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Spec {
    max_scenarios: Option<u64>,
    max_steps_per_scenario: Option<u64>,
    max_nodes: Option<u64>,
    max_outputs: Option<u64>,
    max_trace_entries: Option<u64>,
}

/// A bound the kernel can receive; anything wider than `usize` exceeds every
/// ceiling, so the kernel rejects it as outside the supported range.
fn width(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// Checked `ScenarioLimits`, serialized with every resolved bound.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(into = "Resolved")]
pub struct Limits(pub(crate) ScenarioLimits);
#[derive(Serialize)]
struct Resolved {
    max_scenarios: usize,
    max_steps_per_scenario: usize,
    max_nodes: usize,
    max_outputs: usize,
    max_trace_entries: usize,
}
impl From<Limits> for Resolved {
    fn from(limits: Limits) -> Self {
        Self {
            max_scenarios: limits.0.max_scenarios(),
            max_steps_per_scenario: limits.0.max_steps_per_scenario(),
            max_nodes: limits.0.max_nodes(),
            max_outputs: limits.0.max_outputs(),
            max_trace_entries: limits.0.max_trace_entries(),
        }
    }
}
impl Limits {
    /// Build through `ScenarioLimits::builder`.
    ///
    /// # Errors
    /// Returns the kernel's `OutsideSupportedRange` for a zero or over-ceiling
    /// bound.
    pub fn parse(source: &str) -> Result<Self, Failure> {
        Self::build(parse(source)?)
    }
    pub(crate) fn build(spec: Spec) -> Result<Self, Failure> {
        let mut builder = ScenarioLimits::builder();
        if let Some(value) = spec.max_scenarios {
            builder = builder.max_scenarios(width(value));
        }
        if let Some(value) = spec.max_steps_per_scenario {
            builder = builder.max_steps_per_scenario(width(value));
        }
        if let Some(value) = spec.max_nodes {
            builder = builder.max_nodes(width(value));
        }
        if let Some(value) = spec.max_outputs {
            builder = builder.max_outputs(width(value));
        }
        if let Some(value) = spec.max_trace_entries {
            builder = builder.max_trace_entries(width(value));
        }
        builder.build().map(Self).map_err(Failure::limits)
    }
}

/// Ask the kernel whether an assertion with or without a predicate builds.
///
/// # Errors
/// Returns `MissingPredicate` when no predicate was supplied.
pub fn check_assertion(has_predicate: bool) -> Result<(), Failure> {
    let builder = ScenarioAssertion::<CountIds>::builder(());
    let builder = if has_predicate {
        builder.evaluate(|_| true)
    } else {
        builder
    };
    builder
        .build()
        .map(drop)
        .map_err(|error| Failure::assertion(None, error))
}
