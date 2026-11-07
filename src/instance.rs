mod definition_owner;
mod evaluation;
mod event_inventory;
mod record;
pub(crate) use definition_owner::MachineInstanceDefinition;
pub use record::TransitionRecord;

use crate::{
    Event,
    MachineDefinition,
    OutputEventIntent,
};

/// Result of caller-driven event dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchOutcome<X = (), O = (), I: crate::MachineIds = crate::DefaultIds> {
    /// No edge from the current state matched this event and its guards.
    Refused {
        /// Current state at refusal.
        state: I::State,
        /// Event that had no enabled transition.
        event: I::Event,
    },
    /// An explicitly declared harmless event disposition was accepted.
    Ignored {
        /// Disposition that explains the ignored event.
        disposition: I::Disposition,
        /// Current state at disposition.
        state: I::State,
        /// Event that was intentionally ignored.
        event: I::Event,
        /// Human-readable disposition explanation.
        reason: String,
    },
    /// One edge was accepted and committed.
    Transitioned {
        /// Accepted transition and revision change.
        record: TransitionRecord<I>,
        /// Bounded external work descriptions for the caller to execute.
        effects: Vec<EffectIntent<X, I>>,
        /// Bounded typed events returned to the caller for explicit routing.
        outputs: Vec<OutputEventIntent<O, I>>,
    },
}

impl<X, O, I: crate::MachineIds> DispatchOutcome<X, O, I> {
    /// Whether dispatch explicitly refused this event without mutation.
    #[must_use]
    pub const fn is_refused(&self) -> bool {
        matches!(self, Self::Refused { .. })
    }

    /// Whether dispatch accepted one transition.
    #[must_use]
    pub const fn is_transitioned(&self) -> bool {
        matches!(self, Self::Transitioned { .. })
    }
}

/// A typed external-work description; execution remains caller-owned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectIntent<X, I: crate::MachineIds = crate::DefaultIds> {
    id: I::Effect,
    payload: X,
}

impl<X, I: crate::MachineIds> EffectIntent<X, I> {
    /// Return the named effect.
    #[must_use]
    pub const fn id(&self) -> &I::Effect {
        &self.id
    }

    /// Return the caller-owned request payload.
    #[must_use]
    pub const fn payload(&self) -> &X {
        &self.payload
    }

    /// Consume the intent and return its identifier and owned request payload.
    ///
    /// The caller can move the payload into external execution without cloning
    /// it. This method does not execute the effect.
    #[must_use]
    pub fn into_parts(self) -> (I::Effect, X) {
        (self.id, self.payload)
    }
}

/// Failure evaluating or accepting a transition. With the
/// [`CandidateData`](crate::CandidateData) and callback contracts satisfied,
/// the instance remains unchanged.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DispatchError<I: crate::MachineIds = crate::DefaultIds> {
    /// More than one guarded edge matched the supplied state and event.
    #[error("event matches multiple enabled transitions from state {state:?}")]
    Ambiguous {
        /// State from which the event matched multiple edges.
        state: I::State,
    },
    /// Candidate data failed a destination invariant.
    #[error("candidate data violates destination invariant for {state:?}")]
    InvariantFailed {
        /// Destination state whose invariant failed.
        state: I::State,
    },
    /// Candidate data failed a declared postcondition.
    #[error("transition postcondition {condition:?} rejected candidate data")]
    PostconditionFailed {
        /// Condition that rejected the candidate.
        condition: I::Condition,
    },
    /// A pure candidate action returned an error.
    #[error("transition action failed: {message}")]
    ActionFailed {
        /// Error returned by the candidate action.
        message: String,
    },
    /// A decision names a different immutable definition identity.
    #[error("decision definition {expected:?} does not match {current:?}")]
    DefinitionMismatch {
        /// Definition used during evaluation.
        expected: I::Definition,
        /// Definition held by the acceptor.
        current: I::Definition,
    },
    /// A decision belongs to another instance incarnation.
    #[error("decision instance {expected:?} does not match {current:?}")]
    InstanceMismatch {
        /// Instance used during evaluation.
        expected: I::Instance,
        /// Instance held by the acceptor.
        current: I::Instance,
    },
    /// The local monotonic revision cannot advance further.
    #[error("machine instance revision is exhausted")]
    RevisionExhausted,
    /// The caller evaluated against an obsolete instance revision.
    #[error("stale machine decision: expected revision {expected}, current revision {current}")]
    StaleRevision {
        /// Revision the caller expected to accept.
        expected: u64,
        /// Revision currently held by the instance.
        current: u64,
    },
}

/// One machine instance. Its owner serializes calls to
/// [`dispatch`](Self::dispatch).
///
/// Data must obey [`CandidateData`](crate::CandidateData). Guards, invariants
/// and intent constructors must not mutate accepted data or external state;
/// actions may mutate only the isolated candidate. Immutable sharing is valid,
/// but interior-mutable aliases and impure callbacks are caller violations.
pub struct MachineInstance<
    'definition,
    D,
    E,
    F,
    X = (),
    O = (),
    I: crate::MachineIds = crate::DefaultIds,
> {
    definition: MachineInstanceDefinition<'definition, D, E, F, X, O, I>,
    instance_id: I::Instance,
    state: I::State,
    data: D,
    revision: u64,
}

impl<'definition, D, E, F, X, O, I: crate::MachineIds>
    MachineInstance<'definition, D, E, F, X, O, I>
where
    D: crate::CandidateData,
{
    pub(crate) fn new(
        definition: MachineInstanceDefinition<'definition, D, E, F, X, O, I>,
        instance_id: I::Instance,
        state: I::State,
        data: D,
    ) -> Self {
        Self {
            definition,
            instance_id,
            state,
            data,
            revision: 0,
        }
    }

    /// Return this instance's caller-supplied identity.
    #[must_use]
    pub fn instance_id(&self) -> I::Instance {
        self.instance_id.clone()
    }

    /// Return the current state identifier.
    #[must_use]
    pub const fn state(&self) -> &I::State {
        &self.state
    }

    /// Return the current local data.
    #[must_use]
    pub const fn data(&self) -> &D {
        &self.data
    }

    /// Return the accepted transition revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn definition(&self) -> &MachineDefinition<D, E, F, X, O, I> {
        self.definition.definition()
    }

    /// Evaluate and atomically accept at most one matching transition.
    ///
    /// # Errors
    ///
    /// Returns an error for ambiguous enabled edges, failed actions or
    /// invariants, or exhausted revision space. With the
    /// [`CandidateData`](crate::CandidateData) and callback contracts
    /// satisfied, every error leaves accepted state, data and revision
    /// unchanged and releases no intents. Panics and external effects are not
    /// rolled back.
    pub fn dispatch(
        &mut self,
        event: Event<E, I>,
        facts: &F,
    ) -> Result<DispatchOutcome<X, O, I>, DispatchError<I>> {
        self.dispatch_at(self.revision, event, facts)
    }

    /// Dispatch only if the caller's observed revision is still current.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError::StaleRevision`] before evaluating the event if
    /// the caller's expected revision differs from this instance.
    pub fn dispatch_at(
        &mut self,
        expected_revision: u64,
        event: Event<E, I>,
        facts: &F,
    ) -> Result<DispatchOutcome<X, O, I>, DispatchError<I>> {
        let decision = self.evaluate_at(expected_revision, event, facts)?;
        self.accept(decision)
    }

    /// Consume a decision after checking definition, instance and revision.
    ///
    /// Acceptance runs no callbacks. Candidate state/data/revision are
    /// installed together before the caller receives intents. A failed check
    /// drops the decision and its owned payloads without changing this
    /// instance.
    ///
    /// # Errors
    /// Rejects a decision for a different definition, instance or revision.
    ///
    /// # Panics
    /// Panics if an internal transition decision lacks its validated candidate.
    /// Public construction cannot create such a decision.
    pub fn accept(
        &mut self,
        decision: crate::MachineDecision<D, X, O, I>,
    ) -> Result<DispatchOutcome<X, O, I>, DispatchError<I>> {
        let validated =
            decision.validate_for(self.definition().id(), &self.instance_id, self.revision)?;
        let (candidate, outcome) = validated.into_parts();
        if let DispatchOutcome::Transitioned { record, .. } = &outcome {
            self.data = candidate.expect("transition decision contains validated candidate data");
            self.state = record.to().clone();
            self.revision = record.revision();
        }
        Ok(outcome)
    }
}
