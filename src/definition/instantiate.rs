use std::sync::Arc;

use super::{
    DefinitionBuildError,
    DefinitionResult,
    MachineDefinition,
    StateDefinition,
};
use crate::{
    MachineInstance,
    MachineInstanceDefinition,
};

impl<D, E, F, X, O, I: crate::MachineIds> MachineDefinition<D, E, F, X, O, I>
where
    D: crate::FallibleCandidateData,
{
    /// Create an instance after validating its initial data.
    ///
    /// # Errors
    ///
    /// Returns an error if initial data violates the initial state's invariant.
    pub fn instantiate(
        &self,
        instance_id: I::Instance,
        data: D,
    ) -> DefinitionResult<MachineInstance<'_, D, E, F, X, O, I>, I> {
        let initial = self.initial_state()?;
        if !initial
            .accepts(&data)
            .map_err(|error| DefinitionBuildError::EvaluationFailed { error })?
        {
            return Err(DefinitionBuildError::InitialInvariantFailed {
                state: self.initial.clone(),
            });
        }
        Ok(MachineInstance::new(
            MachineInstanceDefinition::Borrowed(self),
            instance_id,
            self.initial.clone(),
            data,
        ))
    }

    /// Create an instance that keeps this shared definition alive after the
    /// caller drops its own `Arc` handle.
    ///
    /// This is useful when an owner stores a long-lived instance without a
    /// self-referential struct. Definitions remain shared and immutable.
    ///
    /// # Errors
    ///
    /// Returns an error if the initial data violates the initial-state
    /// invariant.
    pub fn instantiate_shared(
        self: Arc<Self>,
        instance_id: I::Instance,
        data: D,
    ) -> DefinitionResult<MachineInstance<'static, D, E, F, X, O, I>, I> {
        let initial = self.initial_state()?;
        if !initial
            .accepts(&data)
            .map_err(|error| DefinitionBuildError::EvaluationFailed { error })?
        {
            return Err(DefinitionBuildError::InitialInvariantFailed {
                state: self.initial.clone(),
            });
        }
        Ok(MachineInstance::new(
            MachineInstanceDefinition::Shared(self.clone()),
            instance_id,
            self.initial.clone(),
            data,
        ))
    }

    fn initial_state(&self) -> Result<&StateDefinition<D, E, F, X, I>, DefinitionBuildError<I>> {
        self.states
            .iter()
            .find(|state| state.id == self.initial)
            .ok_or(DefinitionBuildError::MissingInitialState)
    }
}
