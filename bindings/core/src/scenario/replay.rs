//! Fresh-instance replay through the kernel's `ScenarioReplay::run`.
use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;
use status_machina::{
    Event,
    ScenarioAssertion,
    ScenarioReplay,
    ScenarioReplayBuildError,
    ScenarioSet,
};

use super::failure::{
    Failure,
    Fault,
    parse,
};
use super::limits::{
    Limits,
    Spec as LimitsSpec,
};
use super::report::Report;
use crate::build::{
    Selection,
    Template,
};
use crate::callback;
use crate::error::Error;
use crate::ids::{
    Ids,
    ScenarioKey,
};
use crate::spec::Binding;
use crate::value::{
    Data,
    Host,
    Value,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    event: u32,
    payload: u32,
    facts: u32,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    steps: Vec<Step>,
}
/// An absent implementation reaches the kernel builder without a predicate.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Assertion {
    id: u32,
    implementation: Option<Binding>,
}
/// A scenario's identity is its position: the language resolves it, so equal
/// native identities in two scenarios stay two traces.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    limits: LimitsSpec,
    initial: u32,
    scenarios: Vec<Scenario>,
    assertions: Vec<Assertion>,
}

/// Claims each distinct arena key once; repeated inputs share one lease.
struct Leases<H: Host> {
    host: Arc<H>,
    // bounded: at most one lease per arena slot, and the arena is capped.
    values: HashMap<u32, Value<H>>,
}
impl<H: Host> Leases<H> {
    fn get(&mut self, key: u32) -> Result<Value<H>, Error<H>> {
        if let Some(value) = self.values.get(&key) {
            return Ok(value.clone());
        }
        let value = Value::claim(Arc::clone(&self.host), key).map_err(Error::evaluation)?;
        self.values.insert(key, value.clone());
        Ok(value)
    }
}

type NativeAssertion<H> = ScenarioAssertion<Ids<H>>;

impl<H: Host> Template<H> {
    /// Replay the supplied scenarios on fresh instances of this definition.
    ///
    /// Every input value is leased before the first callback runs. The
    /// report holds identities and positions only; every lease is released
    /// when replay returns.
    ///
    /// # Errors
    /// Returns kernel limit, set, assertion and replay rejections, and the
    /// original cause of a failed callback or snapshot.
    pub fn replay(&self, host: &Arc<H>, source: &str) -> Result<Report, Fault<H>> {
        let spec: Spec = parse(source)?;
        let limits = Limits::build(spec.limits)?;
        let mut leases = Leases {
            host: Arc::clone(host),
            values: HashMap::new(),
        };
        let initial = Data {
            value: leases.get(spec.initial)?,
        };
        let mut builder = ScenarioSet::builder(limits.0);
        for (position, scenario) in spec.scenarios.into_iter().enumerate() {
            // bounded: the native reader stops one step past the first bound.
            let mut steps = Vec::with_capacity(scenario.steps.len());
            for step in scenario.steps {
                let payload = leases.get(step.payload)?;
                steps.push((Event::new(step.event, payload), leases.get(step.facts)?));
            }
            let key = ScenarioKey {
                index: u32::try_from(position)
                    .map_err(|_| Failure::transport("scenario position exceeds u32"))?,
                host: Arc::clone(host),
            };
            builder = builder
                .scenario(key, steps)
                .map_err(|error| Failure::set(Some(position), error))?;
        }
        let scenarios = builder.build().map_err(|error| Failure::set(None, error))?;
        let assertions = self.assertions(spec.assertions)?;
        drop(leases);
        let report = ScenarioReplay::run(
            &self.definition,
            uuid::Uuid::now_v7(),
            initial,
            scenarios,
            assertions,
        )
        .map_err(|error| match error {
            ScenarioReplayBuildError::EvaluationFailed { phase, error } => {
                Fault::Core(Error::Evaluation { phase, error })
            }
            ScenarioReplayBuildError::AssertionLimitExceeded { limit } => {
                Fault::from(Failure::assertion_limit(limit))
            }
        })?;
        Ok(Report::from(&report))
    }

    /// Build inline assertions and resolve registered ones in caller order;
    /// the kernel bounds how many run.
    fn assertions(&self, specs: Vec<Assertion>) -> Result<Vec<NativeAssertion<H>>, Fault<H>> {
        // bounded: the native reader stops one entry past the kernel's 128.
        let mut assertions = Vec::with_capacity(specs.len());
        for (entry, spec) in specs.into_iter().enumerate() {
            let builder = ScenarioAssertion::builder(spec.id);
            let assertion = match spec.implementation {
                None => builder
                    .build()
                    .map_err(|error| Failure::assertion(Some(entry), error))?,
                Some(binding) => match binding.select()? {
                    Selection::Inline(cb) => builder
                        .try_evaluate(move |observation| callback::assertion(cb, observation))
                        .build()
                        .map_err(|error| Failure::assertion(Some(entry), error))?,
                    Selection::Registered(reference) => self
                        .registry
                        .resolve_assertion(spec.id, &reference)
                        .map_err(|error| Failure::binding(entry, &error))?,
                },
            };
            assertions.push(assertion);
        }
        Ok(assertions)
    }
}
