use std::sync::Arc;

use super::implementation::Implementation;
use super::{
    BehaviorRef,
    BehaviorRegistry,
    BehaviorRole,
    BindingError,
};

impl<D, E, F, X, O, I: crate::MachineIds> BehaviorRegistry<D, E, F, X, O, I> {
    /// Resolve an action without executing it, retaining its typed reference.
    ///
    /// # Errors
    /// Rejects unknown Stories/implementations and incompatible signature
    /// classes.
    ///
    /// # Panics
    /// Panics if the internal checked signature and implementation variant
    /// disagree.
    pub fn resolve_action(
        &self,
        id: I::Action,
        reference: &BehaviorRef<I>,
    ) -> Result<crate::LocalAction<D, E, F, I>, BindingError<I>> {
        let Implementation::Action(callback) = self.lookup(reference, BehaviorRole::Action)? else {
            unreachable!("checked binding signature matches its implementation");
        };
        Ok(crate::LocalAction::bound(
            id,
            reference.clone(),
            Arc::clone(callback),
        ))
    }

    /// Resolve a condition without executing it, retaining its typed reference.
    ///
    /// # Errors
    /// Rejects unknown Stories/implementations and incompatible signature
    /// classes.
    ///
    /// # Panics
    /// Panics if the internal checked signature and implementation variant
    /// disagree.
    pub fn resolve_condition(
        &self,
        id: I::Condition,
        reference: &BehaviorRef<I>,
    ) -> Result<crate::Condition<D, E, F, I>, BindingError<I>> {
        let Implementation::Condition(callback) =
            self.lookup(reference, BehaviorRole::Condition)?
        else {
            unreachable!("checked binding signature matches its implementation");
        };
        Ok(crate::Condition::bound(
            id,
            reference.clone(),
            Arc::clone(callback),
        ))
    }

    /// Resolve an effect without executing it, retaining its typed reference.
    ///
    /// # Errors
    /// Rejects unknown Stories/implementations and incompatible signature
    /// classes.
    ///
    /// # Panics
    /// Panics if the internal checked signature and implementation variant
    /// disagree.
    pub fn resolve_effect(
        &self,
        id: I::Effect,
        reference: &BehaviorRef<I>,
    ) -> Result<crate::Effect<D, E, F, X, I>, BindingError<I>> {
        let Implementation::Effect(callback) = self.lookup(reference, BehaviorRole::Effect)? else {
            unreachable!("checked binding signature matches its implementation");
        };
        Ok(crate::Effect::bound(
            id,
            reference.clone(),
            Arc::clone(callback),
        ))
    }

    /// Resolve an output event without executing it, retaining its typed
    /// reference.
    ///
    /// # Errors
    /// Rejects unknown Stories/implementations and incompatible signature
    /// classes.
    ///
    /// # Panics
    /// Panics if the internal checked signature and implementation variant
    /// disagree.
    pub fn resolve_output_event(
        &self,
        id: I::Event,
        reference: &BehaviorRef<I>,
    ) -> Result<crate::OutputEvent<D, E, F, O, I>, BindingError<I>> {
        let Implementation::OutputEvent(callback) =
            self.lookup(reference, BehaviorRole::OutputEvent)?
        else {
            unreachable!("checked binding signature matches its implementation");
        };
        Ok(crate::OutputEvent::bound(
            id,
            reference.clone(),
            Arc::clone(callback),
        ))
    }

    /// Resolve an invariant without executing it, retaining its typed
    /// reference.
    ///
    /// # Errors
    /// Rejects unknown Stories/implementations and incompatible signature
    /// classes.
    ///
    /// # Panics
    /// Panics if the internal checked signature and implementation variant
    /// disagree.
    pub fn resolve_invariant(
        &self,
        reference: &BehaviorRef<I>,
    ) -> Result<super::BoundInvariant<D, I>, BindingError<I>> {
        let Implementation::Invariant(callback) =
            self.lookup(reference, BehaviorRole::Invariant)?
        else {
            unreachable!("checked binding signature matches its implementation");
        };
        Ok(super::BoundInvariant::new(
            reference.clone(),
            Arc::clone(callback),
        ))
    }

    /// Resolve a capability without executing it, retaining its typed
    /// reference.
    ///
    /// # Errors
    /// Rejects unknown Stories/implementations and incompatible signature
    /// classes.
    ///
    /// # Panics
    /// Panics if the internal checked signature and implementation variant
    /// disagree.
    pub fn resolve_capability(
        &self,
        id: I::Capability,
        reference: &BehaviorRef<I>,
    ) -> Result<crate::Capability<D, F, I>, BindingError<I>> {
        let Implementation::Capability(callback) =
            self.lookup(reference, BehaviorRole::Capability)?
        else {
            unreachable!("checked binding signature matches its implementation");
        };
        Ok(crate::Capability::bound(
            id,
            reference.clone(),
            Arc::clone(callback),
        ))
    }

    /// Resolve an assertion without executing it, retaining its typed
    /// reference.
    ///
    /// # Errors
    /// Rejects unknown Stories/implementations and incompatible signature
    /// classes.
    ///
    /// # Panics
    /// Panics if the internal checked signature and implementation variant
    /// disagree.
    pub fn resolve_assertion(
        &self,
        id: I::Assertion,
        reference: &BehaviorRef<I>,
    ) -> Result<crate::ScenarioAssertion<I>, BindingError<I>> {
        let Implementation::Assertion(callback) =
            self.lookup(reference, BehaviorRole::Assertion)?
        else {
            unreachable!("checked binding signature matches its implementation");
        };
        Ok(crate::ScenarioAssertion::bound(
            id,
            reference.clone(),
            Arc::clone(callback),
        ))
    }
}
