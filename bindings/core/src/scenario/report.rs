//! Bounded replay reports as compact native metadata.
use serde::Serialize;
use status_machina::{
    BoundedScenarioReport,
    ScenarioFailure,
    ScenarioLimit,
    ScenarioResult,
};

use crate::callback::Observation;
use crate::ids::Ids;
use crate::value::Host;

/// The kernel's `ScenarioResult`. Scenarios are caller positions; every other
/// identity is an index of the definition's own role inventory.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Report {
    CompleteWithinScenarios {
        scenarios: usize,
        nodes: usize,
    },
    Counterexample {
        scenario: u32,
        assertion: u32,
        steps: Vec<Observation>,
    },
    LimitReached {
        scenario: u32,
        step: usize,
        limit: &'static str,
    },
    ExecutionFailed {
        scenario: u32,
        step: usize,
        failure: &'static str,
        steps: Vec<Observation>,
    },
}
fn steps<H: Host>(steps: &[status_machina::ScenarioObservation<Ids<H>>]) -> Vec<Observation> {
    // bounded: the kernel caps the trace at max_trace_entries.
    steps.iter().map(Observation::new).collect()
}
const fn limit(limit: ScenarioLimit) -> &'static str {
    match limit {
        ScenarioLimit::Outputs => "outputs",
        ScenarioLimit::TraceEntries => "trace_entries",
    }
}
const fn failure(failure: ScenarioFailure) -> &'static str {
    match failure {
        ScenarioFailure::EvaluationFailed => "evaluation_failed",
        ScenarioFailure::InvalidInitialData => "invalid_initial_data",
        ScenarioFailure::Ambiguous => "ambiguous",
        ScenarioFailure::InvariantFailed => "invariant_failed",
        ScenarioFailure::PostconditionFailed => "postcondition_failed",
        ScenarioFailure::ActionFailed => "action_failed",
        ScenarioFailure::RevisionExhausted => "revision_exhausted",
        ScenarioFailure::DefinitionMismatch => "definition_mismatch",
        ScenarioFailure::InstanceMismatch => "instance_mismatch",
        ScenarioFailure::StaleRevision => "stale_revision",
    }
}
impl<H: Host> From<&BoundedScenarioReport<Ids<H>>> for Report {
    fn from(report: &BoundedScenarioReport<Ids<H>>) -> Self {
        match report.result() {
            ScenarioResult::CompleteWithinScenarios { scenarios, nodes } => {
                Self::CompleteWithinScenarios {
                    scenarios: *scenarios,
                    nodes: *nodes,
                }
            }
            ScenarioResult::Counterexample(trace) => Self::Counterexample {
                scenario: trace.scenario().index,
                assertion: *trace.assertion(),
                steps: steps(trace.steps()),
            },
            ScenarioResult::LimitReached {
                scenario,
                step,
                limit: reached,
            } => Self::LimitReached {
                scenario: scenario.index,
                step: *step,
                limit: limit(*reached),
            },
            ScenarioResult::ExecutionFailed {
                scenario,
                step,
                failure: category,
                steps: trace,
            } => Self::ExecutionFailed {
                scenario: scenario.index,
                step: *step,
                failure: failure(*category),
                steps: steps(trace),
            },
        }
    }
}
