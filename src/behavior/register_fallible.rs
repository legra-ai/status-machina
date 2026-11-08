use std::sync::Arc;

use super::implementation::Implementation;
use super::{
    BehaviorRef,
    BehaviorRegistryBuilder,
    BindingError,
};

impl<D, E, F, X, O, I: crate::MachineIds> BehaviorRegistryBuilder<D, E, F, X, O, I> {
    /// Register a fallible action Story implementation.
    ///
    /// # Errors
    /// Rejects duplicate declarations, incompatible roles and excess capacity.
    pub fn try_action(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&mut D, &E, &F) -> Result<(), crate::EvaluationError> + Send + Sync + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(
            reference,
            Implementation::Action(Arc::new(move |data, event, facts| {
                callback(data, event, facts).map_err(crate::action::ActionFailure::Raised)
            })),
        )
    }
    /// Register a fallible condition Story implementation.
    ///
    /// # Errors
    /// Rejects duplicate declarations, incompatible roles and excess capacity.
    pub fn try_condition(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&D, &E, &F) -> Result<bool, crate::EvaluationError> + Send + Sync + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(reference, Implementation::Condition(Arc::new(callback)))
    }
    /// Register a fallible effect Story implementation.
    ///
    /// # Errors
    /// Rejects duplicate declarations, incompatible roles and excess capacity.
    pub fn try_effect(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&D, &E, &F) -> Result<X, crate::EvaluationError> + Send + Sync + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(reference, Implementation::Effect(Arc::new(callback)))
    }
    /// Register a fallible output event Story implementation.
    ///
    /// # Errors
    /// Rejects duplicate declarations, incompatible roles and excess capacity.
    pub fn try_output_event(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&D, &E, &F) -> Result<O, crate::EvaluationError> + Send + Sync + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(reference, Implementation::OutputEvent(Arc::new(callback)))
    }
    /// Register a fallible invariant Story implementation.
    ///
    /// # Errors
    /// Rejects duplicate declarations, incompatible roles and excess capacity.
    pub fn try_invariant(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&D) -> Result<bool, crate::EvaluationError> + Send + Sync + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(reference, Implementation::Invariant(Arc::new(callback)))
    }
    /// Register a fallible capability Story implementation.
    ///
    /// # Errors
    /// Rejects duplicate declarations, incompatible roles and excess capacity.
    pub fn try_capability(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&I::State, &D, &F) -> Result<crate::CapabilityStatus, crate::EvaluationError>
        + Send
        + Sync
        + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(reference, Implementation::Capability(Arc::new(callback)))
    }
    /// Register a fallible assertion Story implementation.
    ///
    /// # Errors
    /// Rejects duplicate declarations, incompatible roles and excess capacity.
    pub fn try_assertion(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&crate::ScenarioObservation<I>) -> Result<bool, crate::EvaluationError>
        + Send
        + Sync
        + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(reference, Implementation::Assertion(Arc::new(callback)))
    }
}
