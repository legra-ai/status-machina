use std::sync::Arc;

pub(crate) type ActionOperation<D, E, F> =
    Arc<dyn Fn(&mut D, &E, &F) -> Result<(), String> + Send + Sync>;

/// One named, caller-bound pure action on candidate machine data.
///
/// An action with a different input payload type cannot be attached to an edge:
///
/// ```compile_fail
/// use status_machina::{ActionId, LocalAction, Transition, TransitionId};
/// let action = LocalAction::<u64, u32, ()>::builder(ActionId::try_new("add").unwrap())
///     .apply(|data, event, _| { *data += u64::from(*event); Ok(()) }).build().unwrap();
/// Transition::<u64, (), ()>::builder(TransitionId::try_new("tick").unwrap()).action(action);
/// ```
pub struct LocalAction<D, E, F, I: crate::MachineIds = crate::DefaultIds> {
    pub(crate) id: I::Action,
    pub(crate) apply: ActionOperation<D, E, F>,
    binding: Option<crate::BehaviorRef<I>>,
}

impl<D, E, F, I: crate::MachineIds> LocalAction<D, E, F, I> {
    /// Return the registered Story/implementation reference, if bound by a
    /// registry.
    pub const fn binding(&self) -> Option<&crate::BehaviorRef<I>> {
        self.binding.as_ref()
    }

    pub(crate) fn bound(
        id: I::Action,
        binding: crate::BehaviorRef<I>,
        apply: ActionOperation<D, E, F>,
    ) -> Self {
        Self {
            id,
            binding: Some(binding),
            apply,
        }
    }

    /// Start building an action with a stable identity.
    #[must_use]
    pub fn builder(id: I::Action) -> LocalActionBuilder<D, E, F, I> {
        LocalActionBuilder { id, apply: None }
    }

    /// Return the action identity.
    #[must_use]
    pub const fn id(&self) -> &I::Action {
        &self.id
    }
}

/// Builder for a local candidate-data action.
pub struct LocalActionBuilder<D, E, F, I: crate::MachineIds = crate::DefaultIds> {
    id: I::Action,
    apply: Option<ActionOperation<D, E, F>>,
}

impl<D, E, F, I: crate::MachineIds> LocalActionBuilder<D, E, F, I> {
    /// Bind a deterministic, bounded operation over isolated candidate data.
    ///
    /// Mutate only the candidate; captured values, event payloads and facts
    /// must not provide a path to mutate accepted data or external state.
    /// See [`CandidateData`](crate::CandidateData) for the isolation contract.
    #[must_use]
    pub fn apply(
        mut self,
        apply: impl Fn(&mut D, &E, &F) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        self.apply = Some(Arc::new(apply));
        self
    }

    /// Finish constructing the action.
    ///
    /// # Errors
    ///
    /// Returns an error when no action implementation was supplied.
    pub fn build(self) -> Result<LocalAction<D, E, F, I>, ActionBuildError> {
        Ok(LocalAction {
            binding: None,
            id: self.id,
            apply: self.apply.ok_or(ActionBuildError::MissingImplementation)?,
        })
    }
}

/// An action builder lacks its implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ActionBuildError {
    /// No implementation was supplied for the action identity.
    #[error("local action implementation is missing")]
    MissingImplementation,
}
