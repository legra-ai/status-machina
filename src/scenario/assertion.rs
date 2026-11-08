use std::sync::Arc;

use super::ScenarioObservation;

type AssertionCallback<I = crate::DefaultIds> =
    Arc<dyn Fn(&ScenarioObservation<I>) -> Result<bool, crate::EvaluationError> + Send + Sync>;

/// One independent, named predicate over compact step observations.
pub struct ScenarioAssertion<I: crate::MachineIds = crate::DefaultIds> {
    pub(in crate::scenario) id: I::Assertion,
    pub(in crate::scenario) evaluate: AssertionCallback<I>,
    binding: Option<crate::BehaviorRef<I>>,
}

impl<I: crate::MachineIds> ScenarioAssertion<I> {
    /// Return the registered Story/implementation reference, if bound by a
    /// registry.
    pub const fn binding(&self) -> Option<&crate::BehaviorRef<I>> {
        self.binding.as_ref()
    }

    pub(crate) fn bound(
        id: I::Assertion,
        binding: crate::BehaviorRef<I>,
        evaluate: AssertionCallback<I>,
    ) -> Self {
        Self {
            id,
            binding: Some(binding),
            evaluate,
        }
    }

    /// Start a named safety assertion.
    #[must_use]
    pub fn builder(id: I::Assertion) -> ScenarioAssertionBuilder<I> {
        ScenarioAssertionBuilder { id, evaluate: None }
    }
}

/// Builder for one pure, bounded safety assertion callback.
pub struct ScenarioAssertionBuilder<I: crate::MachineIds = crate::DefaultIds> {
    id: I::Assertion,
    evaluate: Option<AssertionCallback<I>>,
}

impl<I: crate::MachineIds> ScenarioAssertionBuilder<I> {
    /// Set a predicate that returns true while the safety assertion holds.
    ///
    /// The callback must be independently authored, pure, total and bounded.
    /// Rust cannot enforce these properties.
    #[must_use]
    pub fn evaluate(
        mut self,
        evaluate: impl Fn(&ScenarioObservation<I>) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.evaluate = Some(Arc::new(move |observation| Ok(evaluate(observation))));
        self
    }

    /// Bind a safety assertion that can raise an application exception.
    #[must_use]
    pub fn try_evaluate(
        mut self,
        evaluate: impl Fn(&ScenarioObservation<I>) -> Result<bool, crate::EvaluationError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.evaluate = Some(Arc::new(evaluate));
        self
    }

    /// Build the assertion.
    ///
    /// # Errors
    ///
    /// Returns an error if no predicate was supplied.
    pub fn build(self) -> Result<ScenarioAssertion<I>, ScenarioAssertionBuildError> {
        Ok(ScenarioAssertion {
            binding: None,
            id: self.id,
            evaluate: self
                .evaluate
                .ok_or(ScenarioAssertionBuildError::MissingPredicate)?,
        })
    }
}

/// A scenario assertion omitted its predicate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ScenarioAssertionBuildError {
    /// No predicate was supplied.
    #[error("scenario assertion predicate is missing")]
    MissingPredicate,
}

pub(in crate::scenario) const fn hard_max_assertions() -> usize {
    128
}
