//! Scenario-set bounds checked by the kernel builder at native construction.
use serde::Deserialize;
use status_machina::{
    Event,
    ScenarioSet,
};

use super::failure::{
    Failure,
    parse,
};
use super::ids::CountIds;
use super::limits::{
    Limits,
    Spec as LimitsSpec,
};

/// The step count of each supplied scenario, in caller order. A native reader
/// stops one entry past the first exceeded bound, so the counts stay bounded.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    limits: LimitsSpec,
    steps: Vec<usize>,
}

/// Build the counted set through `ScenarioSet::builder` and report the
/// kernel's first rejection, in the kernel's own precedence.
///
/// # Errors
/// Returns invalid limits or the kernel's `ScenarioSetBuildError`, with the
/// position of the scenario it refused.
pub fn check(source: &str) -> Result<(), Failure> {
    let spec: Spec = parse(source)?;
    let limits = Limits::build(spec.limits)?;
    let mut builder = ScenarioSet::<(), (), CountIds>::builder(limits.0);
    for (entry, count) in spec.steps.into_iter().enumerate() {
        builder = builder
            .scenario((), std::iter::repeat_n((Event::new((), ()), ()), count))
            .map_err(|error| Failure::set(Some(entry), error))?;
    }
    builder
        .build()
        .map(drop)
        .map_err(|error| Failure::set(None, error))
}
