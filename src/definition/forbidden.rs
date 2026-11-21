//! Explicit structural prohibitions: typed state pairs no transition may join.

use std::collections::HashSet;

use super::{
    DefinitionBuildError,
    DefinitionBuilder,
};

/// A typed source/destination pair that no declared transition may connect.
///
/// Dispatch already refuses every undeclared edge, so a prohibition is not
/// needed to refuse an edge that is absent. It protects a definition against a
/// later edit that would add the edge: construction rejects a definition that
/// declares both a transition and a matching prohibition, whatever the
/// declaration order, the transition's event, kind or guard. A same-state pair
/// forbids internal and external self-transitions alike.
///
/// A prohibition is data only. It has no identity besides its pair, never
/// executes, and is never rendered as a diagram edge.
///
/// ```rust
/// use status_machina::{
///     DefinitionBuildError,
///     EventId,
///     ForbiddenEdge,
///     MachineDefinition,
///     MachineId,
///     StateDefinition,
///     StateId,
///     Transition,
///     TransitionId,
/// };
/// fn state(name: &str) -> StateId {
///     StateId::try_new(name).unwrap()
/// }
/// fn definition(edges: &[(&str, &str, &str)]) -> Result<(), DefinitionBuildError> {
///     let mut builder =
///         MachineDefinition::<(), (), ()>::builder(MachineId::try_new("seal").unwrap())
///             .initial(state("open"))
///             .forbid(ForbiddenEdge::new(state("sealing"), state("cancelled")))?;
///     for name in ["open", "sealing", "done", "cancelled"] {
///         builder = builder.state(StateDefinition::builder(state(name)).build())?;
///     }
///     for (name, from, to) in edges {
///         let edge = Transition::builder(TransitionId::try_new(*name).unwrap())
///             .from(state(from))
///             .on(EventId::try_new(*name).unwrap())
///             .to(state(to))
///             .build()
///             .unwrap();
///         builder = builder.transition(edge)?;
///     }
///     builder.build().map(drop)
/// }
/// let legal = [("seal", "open", "sealing"), ("finish", "sealing", "done"), ("cancel", "open", "cancelled")];
/// assert!(definition(&legal).is_ok());
/// let edited = [legal[0], legal[1], legal[2], ("abandon", "sealing", "cancelled")];
/// assert!(matches!(
///     definition(&edited),
///     Err(DefinitionBuildError::ForbiddenTransition { transition, from, to })
///         if transition == TransitionId::try_new("abandon").unwrap()
///             && from == state("sealing")
///             && to == state("cancelled")
/// ));
/// ```
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ForbiddenEdge<I: crate::MachineIds = crate::DefaultIds> {
    from: I::State,
    to: I::State,
}

impl<I: crate::MachineIds> ForbiddenEdge<I> {
    /// Forbid every transition from `from` to `to`.
    #[must_use]
    pub const fn new(from: I::State, to: I::State) -> Self {
        Self { from, to }
    }

    /// Return the forbidden source state.
    #[must_use]
    pub const fn from(&self) -> &I::State {
        &self.from
    }

    /// Return the forbidden destination state.
    #[must_use]
    pub const fn to(&self) -> &I::State {
        &self.to
    }

    /// Whether this prohibition forbids an edge between these endpoints.
    pub(crate) fn forbids(&self, from: &I::State, to: &I::State) -> bool {
        self.from == *from && self.to == *to
    }
}

impl<D, E, F, X, O, I: crate::MachineIds> DefinitionBuilder<D, E, F, X, O, I> {
    /// Check prohibitions against declared states, each other and every
    /// declared transition.
    ///
    /// There is no precedence: any transition joining a forbidden pair is a
    /// contradiction, whatever its event, kind or guard. Nothing is evaluated.
    pub(super) fn validate_forbidden_edges(
        &self,
        states: &HashSet<I::State>,
    ) -> Result<(), DefinitionBuildError<I>> {
        // bounded: at most MAX_FORBIDDEN_EDGES prohibitions can be stored.
        let mut pairs = HashSet::with_capacity(self.forbidden_edges.len());
        for edge in &self.forbidden_edges {
            if let Some(state) = [&edge.from, &edge.to]
                .into_iter()
                .find(|state| !states.contains(*state))
            {
                return Err(DefinitionBuildError::UnknownForbiddenEdgeState {
                    from: edge.from.clone(),
                    to: edge.to.clone(),
                    state: state.clone(),
                });
            }
            if !pairs.insert((&edge.from, &edge.to)) {
                return Err(DefinitionBuildError::DuplicateForbiddenEdge {
                    from: edge.from.clone(),
                    to: edge.to.clone(),
                });
            }
        }
        for transition in &self.transitions {
            if self
                .forbidden_edges
                .iter()
                .any(|edge| edge.forbids(&transition.from, &transition.to))
            {
                return Err(DefinitionBuildError::ForbiddenTransition {
                    transition: transition.id.clone(),
                    from: transition.from.clone(),
                    to: transition.to.clone(),
                });
            }
        }
        Ok(())
    }
}
