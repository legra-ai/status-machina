use std::sync::Arc;

use thiserror::Error;

use super::{
    MachineDefinition,
    StateDefinition,
};
use crate::{
    MachineInstance,
    MachineInstanceDefinition,
};

/// Checked restoration result retaining the caller's identity family.
pub type RestoreResult<T, I = crate::DefaultIds> = Result<T, RestoreError<I>>;

/// A refused restoration of an instance at a caller-named state.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RestoreError<I: crate::MachineIds = crate::DefaultIds> {
    /// The definition declares no such state.
    #[error("cannot restore at undeclared state {state:?}")]
    UnknownState {
        /// The state the caller named.
        state: I::State,
    },
    /// The restored data violates the named state's invariant.
    #[error("restored data violates state invariant for {state:?}")]
    InvariantFailed {
        /// State whose invariant rejected the data.
        state: I::State,
    },
    /// Invariant evaluation raised an application error.
    #[error("restored invariant evaluation failed: {error}")]
    EvaluationFailed {
        /// Original application failure.
        #[source]
        error: crate::EvaluationError,
    },
}

impl<D, E, F, X, O, I: crate::MachineIds> MachineDefinition<D, E, F, X, O, I>
where
    D: crate::FallibleCandidateData,
{
    /// Create an instance at a declared state, for example from a persisted
    /// snapshot, after validating its data against that state's invariant.
    ///
    /// The instance starts at revision zero: revisions order this
    /// incarnation's accepted transitions, not the history before the
    /// snapshot. No entry action or effect runs.
    ///
    /// # Errors
    ///
    /// Refuses a state the definition does not declare, data violating the
    /// state's invariant, and a failing invariant evaluation.
    pub fn restore(
        &self,
        instance_id: I::Instance,
        state: I::State,
        data: D,
    ) -> RestoreResult<MachineInstance<'_, D, E, F, X, O, I>, I> {
        self.check_restored(&state, &data)?;
        Ok(MachineInstance::new(
            MachineInstanceDefinition::Borrowed(self),
            instance_id,
            state,
            data,
        ))
    }

    /// Like [`restore`](Self::restore), for an instance that keeps this
    /// shared definition alive after the caller drops its own handle.
    ///
    /// # Errors
    ///
    /// Refuses exactly what [`restore`](Self::restore) refuses.
    pub fn restore_shared(
        self: Arc<Self>,
        instance_id: I::Instance,
        state: I::State,
        data: D,
    ) -> RestoreResult<MachineInstance<'static, D, E, F, X, O, I>, I> {
        self.check_restored(&state, &data)?;
        Ok(MachineInstance::new(
            MachineInstanceDefinition::Shared(self),
            instance_id,
            state,
            data,
        ))
    }

    fn check_restored(&self, state: &I::State, data: &D) -> RestoreResult<(), I> {
        let declared: &StateDefinition<D, E, F, X, I> =
            self.state(state)
                .ok_or_else(|| RestoreError::UnknownState {
                    state: state.clone(),
                })?;
        if declared
            .accepts(data)
            .map_err(|error| RestoreError::EvaluationFailed { error })?
        {
            Ok(())
        } else {
            Err(RestoreError::InvariantFailed {
                state: state.clone(),
            })
        }
    }
}
