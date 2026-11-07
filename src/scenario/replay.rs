use super::types::assertion::hard_max_assertions;
use super::types::{
    BoundedScenarioReport,
    CounterexampleTrace,
    ScenarioAssertion,
    ScenarioFailure,
    ScenarioLimit,
    ScenarioObservation,
    ScenarioResult,
    ScenarioSet,
    ScenarioStepKind,
};
use crate::{
    DispatchOutcome,
    MachineDefinition,
};

/// Caller-driven replay of explicit finite event/fact scenarios.
pub struct ScenarioReplay;

impl ScenarioReplay {
    /// Execute the supplied ordered scenarios on fresh ephemeral instances.
    ///
    /// Reports retain only identifiers, state names and step indexes. They
    /// never store or clone event, fact, machine-data, effect or output
    /// payloads. Each step performs exactly one dispatch. Output intents
    /// are counted and discarded; routed events are never recursively
    /// dispatched.
    ///
    /// Transition conditions, actions, output constructors and assertion
    /// callbacks are arbitrary Rust callbacks. They must be pure, total and
    /// bounded in runtime; the type system cannot enforce this contract.
    /// Payload byte size is caller-controlled and cannot be bounded for generic
    /// types. Initial data is snapshotted once per scenario through
    /// [`CandidateData`](crate::CandidateData); its byte size and snapshot
    /// cost remain caller-owned. Scenario step, node, combined output/effect
    /// intent and retained trace entry counts are bounded by
    /// [`ScenarioLimits`](super::ScenarioLimits).
    ///
    /// # Errors
    ///
    /// Returns an error when more than 128 safety assertions are supplied.
    #[allow(clippy::needless_pass_by_value)]
    pub fn run<D, E, F, X, O, I: crate::MachineIds>(
        definition: &MachineDefinition<D, E, F, X, O, I>,
        instance_id: I::Instance,
        initial_data: D,
        scenarios: ScenarioSet<E, F, I>,
        assertions: impl IntoIterator<Item = ScenarioAssertion<I>>,
    ) -> Result<BoundedScenarioReport<I>, ScenarioReplayBuildError>
    where
        D: crate::FallibleCandidateData,
    {
        let assertions = Self::bounded_assertions(assertions)?;
        let limits = scenarios.limits;
        let scenario_count = scenarios.scenarios.len();
        let mut nodes = 0;
        let mut intent_count = 0;

        for scenario in scenarios.scenarios {
            let data = match initial_data.try_snapshot() {
                Ok(data) => data,
                Err(_) => {
                    return Ok(BoundedScenarioReport {
                        result: ScenarioResult::ExecutionFailed {
                            scenario: scenario.id,
                            step: 0,
                            failure: ScenarioFailure::EvaluationFailed,
                            steps: Vec::new(),
                        },
                    });
                }
            };
            let mut instance = match definition.instantiate(instance_id.clone(), data) {
                Ok(instance) => instance,
                Err(_) => {
                    return Ok(BoundedScenarioReport {
                        result: ScenarioResult::ExecutionFailed {
                            scenario: scenario.id,
                            step: 0,
                            failure: ScenarioFailure::InvalidInitialData,
                            steps: Vec::new(),
                        },
                    });
                }
            };
            // bounded: each scenario has at most max_steps_per_scenario steps,
            // and trace capacity is capped independently.
            let mut trace = Vec::with_capacity(scenario.steps.len().min(limits.max_trace_entries));

            for (step_index, (event, facts)) in scenario.steps.into_iter().enumerate() {
                if trace.len() == limits.max_trace_entries {
                    return Ok(BoundedScenarioReport {
                        result: ScenarioResult::LimitReached {
                            scenario: scenario.id,
                            step: step_index,
                            limit: ScenarioLimit::TraceEntries,
                        },
                    });
                }

                let before = instance.state().clone();
                let input_event = event.id().clone();
                nodes += 1;
                let outcome = match instance.dispatch(event, &facts) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        let observation = ScenarioObservation {
                            scenario: scenario.id,
                            step: step_index,
                            input_event,
                            before: before.clone(),
                            after: instance.state().clone(),
                            kind: ScenarioStepKind::DispatchFailed,
                            output_events: Vec::new(),
                            effects: Vec::new(),
                        };
                        return Ok(Self::dispatch_failure_report(
                            observation,
                            ScenarioFailure::from(&error),
                            trace,
                            &assertions,
                        ));
                    }
                };
                let emitted_count = match &outcome {
                    DispatchOutcome::Transitioned {
                        effects, outputs, ..
                    } => effects.len() + outputs.len(),
                    DispatchOutcome::Ignored { .. } | DispatchOutcome::Refused { .. } => 0,
                };
                if emitted_count > limits.max_outputs - intent_count {
                    return Ok(BoundedScenarioReport {
                        result: ScenarioResult::LimitReached {
                            scenario: scenario.id,
                            step: step_index,
                            limit: ScenarioLimit::Outputs,
                        },
                    });
                }
                intent_count += emitted_count;

                let (kind, output_events, effects) = match outcome {
                    DispatchOutcome::Transitioned {
                        record,
                        effects: emitted_effects,
                        outputs: emitted_outputs,
                        ..
                    } => {
                        // bounded: combined intent count was checked against
                        // the remaining output/effect intent budget.
                        let output_events = emitted_outputs
                            .iter()
                            .map(|intent| intent.id().clone())
                            .collect();
                        let effects = emitted_effects
                            .iter()
                            .map(|intent| intent.id().clone())
                            .collect();
                        (
                            ScenarioStepKind::Transitioned(record.transition().clone()),
                            output_events,
                            effects,
                        )
                    }
                    DispatchOutcome::Ignored { disposition, .. } => (
                        ScenarioStepKind::Ignored(disposition),
                        Vec::new(),
                        Vec::new(),
                    ),
                    DispatchOutcome::Refused { .. } => {
                        (ScenarioStepKind::Refused, Vec::new(), Vec::new())
                    }
                };
                let observation = ScenarioObservation {
                    scenario: scenario.id.clone(),
                    step: step_index,
                    input_event,
                    before,
                    after: instance.state().clone(),
                    kind,
                    output_events,
                    effects,
                };
                let failed_assertion = assertions
                    .iter()
                    .find(|assertion| !(assertion.evaluate)(&observation))
                    .map(|assertion| assertion.id.clone());
                trace.push(observation);
                if let Some(assertion) = failed_assertion {
                    return Ok(BoundedScenarioReport {
                        result: ScenarioResult::Counterexample(CounterexampleTrace {
                            scenario: scenario.id,
                            assertion,
                            steps: trace,
                        }),
                    });
                }
            }
        }

        Ok(BoundedScenarioReport {
            result: ScenarioResult::CompleteWithinScenarios {
                scenarios: scenario_count,
                nodes,
            },
        })
    }

    fn bounded_assertions<I: crate::MachineIds>(
        assertions: impl IntoIterator<Item = ScenarioAssertion<I>>,
    ) -> Result<Vec<ScenarioAssertion<I>>, ScenarioReplayBuildError> {
        let mut bounded = Vec::new();
        for assertion in assertions {
            if bounded.len() == hard_max_assertions() {
                return Err(ScenarioReplayBuildError::AssertionLimitExceeded {
                    limit: hard_max_assertions(),
                });
            }
            bounded.push(assertion);
        }
        Ok(bounded)
    }

    fn dispatch_failure_report<I: crate::MachineIds>(
        observation: ScenarioObservation<I>,
        failure: ScenarioFailure,
        mut trace: Vec<ScenarioObservation<I>>,
        assertions: &[ScenarioAssertion<I>],
    ) -> BoundedScenarioReport<I> {
        let scenario = observation.scenario.clone();
        let step = observation.step;
        let failed_assertion = assertions
            .iter()
            .find(|assertion| !(assertion.evaluate)(&observation))
            .map(|assertion| assertion.id.clone());
        trace.push(observation);
        let result = if let Some(assertion) = failed_assertion {
            ScenarioResult::Counterexample(CounterexampleTrace {
                scenario,
                assertion,
                steps: trace,
            })
        } else {
            ScenarioResult::ExecutionFailed {
                scenario,
                step,
                failure,
                steps: trace,
            }
        };
        BoundedScenarioReport { result }
    }
}

/// Invalid scenario replay configuration supplied to the runner.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ScenarioReplayBuildError {
    /// Caller supplied more named assertions than the bounded ceiling.
    #[error("scenario assertion limit {limit} exceeded")]
    AssertionLimitExceeded {
        /// Hard maximum number of assertions.
        limit: usize,
    },
}
