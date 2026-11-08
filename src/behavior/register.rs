use std::sync::Arc;

use super::implementation::Implementation;
use super::{
    BehaviorRef,
    BehaviorRegistryBuilder,
    BindingError,
};

impl<D, E, F, X, O, I: crate::MachineIds> BehaviorRegistryBuilder<D, E, F, X, O, I> {
    /// Register a trusted action implementation under typed
    /// Story/implementation identities.
    ///
    /// # Errors
    /// Rejects duplicate declarations, inconsistent Story signatures or excess
    /// capacity.
    pub fn action(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&mut D, &E, &F) -> Result<(), String> + Send + Sync + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(
            reference,
            Implementation::Action(Arc::new(move |data, event, facts| {
                callback(data, event, facts).map_err(crate::action::ActionFailure::Rejected)
            })),
        )
    }

    /// Register a trusted condition implementation under typed
    /// Story/implementation identities.
    ///
    /// # Errors
    /// Rejects duplicate declarations, inconsistent Story signatures or excess
    /// capacity.
    pub fn condition(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&D, &E, &F) -> bool + Send + Sync + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(
            reference,
            Implementation::Condition(Arc::new(move |data, event, facts| {
                Ok(callback(data, event, facts))
            })),
        )
    }

    /// Register a trusted effect implementation under typed
    /// Story/implementation identities.
    ///
    /// # Errors
    /// Rejects duplicate declarations, inconsistent Story signatures or excess
    /// capacity.
    pub fn effect(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&D, &E, &F) -> X + Send + Sync + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(
            reference,
            Implementation::Effect(Arc::new(move |data, event, facts| {
                Ok(callback(data, event, facts))
            })),
        )
    }

    /// Register a trusted output event implementation under typed
    /// Story/implementation identities.
    ///
    /// # Errors
    /// Rejects duplicate declarations, inconsistent Story signatures or excess
    /// capacity.
    pub fn output_event(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&D, &E, &F) -> O + Send + Sync + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(
            reference,
            Implementation::OutputEvent(Arc::new(move |data, event, facts| {
                Ok(callback(data, event, facts))
            })),
        )
    }

    /// Register a trusted invariant implementation under typed
    /// Story/implementation identities.
    ///
    /// # Errors
    /// Rejects duplicate declarations, inconsistent Story signatures or excess
    /// capacity.
    pub fn invariant(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&D) -> bool + Send + Sync + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(
            reference,
            Implementation::Invariant(Arc::new(move |data| Ok(callback(data)))),
        )
    }

    /// Register a trusted capability implementation under typed
    /// Story/implementation identities.
    ///
    /// # Errors
    /// Rejects duplicate declarations, inconsistent Story signatures or excess
    /// capacity.
    pub fn capability(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&I::State, &D, &F) -> crate::CapabilityStatus + Send + Sync + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(
            reference,
            Implementation::Capability(Arc::new(move |state, data, facts| {
                Ok(callback(state, data, facts))
            })),
        )
    }

    /// Register a trusted assertion implementation under typed
    /// Story/implementation identities.
    ///
    /// # Errors
    /// Rejects duplicate declarations, inconsistent Story signatures or excess
    /// capacity.
    pub fn assertion(
        self,
        reference: BehaviorRef<I>,
        callback: impl Fn(&crate::ScenarioObservation<I>) -> bool + Send + Sync + 'static,
    ) -> Result<Self, BindingError<I>> {
        self.insert(
            reference,
            Implementation::Assertion(Arc::new(move |observation| Ok(callback(observation)))),
        )
    }
}
