use crate::{
    DispatchError,
    DispatchOutcome,
    MachineIds,
};

/// Owned, unaccepted evaluation result for one definition/instance/revision.
///
/// Candidate data and intent payloads remain sealed until identity and revision
/// checks succeed. This value is neither cloneable nor serializable; dropping
/// it discards the proposal without modifying its source instance. Evaluation
/// and acceptance use the same caller-owned identity types as dispatch.
///
/// Callers must assign unique instance-incarnation identities and keep each
/// definition identity bound to one immutable executable definition. Identity
/// equality does not prove arbitrary callbacks equivalent.
///
/// A decision cannot be accepted twice, even when its payloads are cloneable:
///
/// ```compile_fail,E0382
/// use status_machina::{MachineDecision, MachineInstance};
/// fn accept_twice(instance: &mut MachineInstance<'_, (), (), ()>, decision: MachineDecision<()>) {
///     instance.accept(decision).unwrap();
///     instance.accept(decision).unwrap();
/// }
/// ```
pub struct MachineDecision<D, X = (), O = (), I: MachineIds = crate::DefaultIds> {
    definition: I::Definition,
    instance: I::Instance,
    revision: u64,
    candidate: Option<D>,
    outcome: DispatchOutcome<X, O, I>,
}

impl<D, X, O, I: MachineIds> MachineDecision<D, X, O, I> {
    pub(crate) fn new(
        definition: I::Definition,
        instance: I::Instance,
        revision: u64,
        candidate: Option<D>,
        outcome: DispatchOutcome<X, O, I>,
    ) -> Self {
        Self {
            definition,
            instance,
            revision,
            candidate,
            outcome,
        }
    }

    /// Definition identity captured during evaluation.
    pub const fn definition_id(&self) -> &I::Definition {
        &self.definition
    }

    /// Instance-incarnation identity captured during evaluation.
    pub const fn instance_id(&self) -> &I::Instance {
        &self.instance
    }

    /// Revision the acceptor must still hold.
    pub const fn expected_revision(&self) -> u64 {
        self.revision
    }

    /// Validate against an acceptor's current identity and revision.
    ///
    /// An external durable adapter must read these values and persist the
    /// validated state and intents in the **same atomic transaction**. This
    /// method performs no I/O, commit, execution or durable acknowledgement;
    /// validation alone is not durable acceptance. Supplied external facts are
    /// not refreshed; their continued admissibility is the caller's policy.
    /// Execute intents only after
    /// that transaction commits. Refused/ignored decisions have no candidate.
    ///
    /// # Errors
    /// Rejects different definition/instance identities and stale revisions.
    /// Failure consumes and drops the proposal without exposing its payloads.
    pub fn validate_for(
        self,
        definition: &I::Definition,
        instance: &I::Instance,
        revision: u64,
    ) -> Result<ValidatedDecision<D, X, O, I>, DispatchError<I>> {
        if &self.definition != definition {
            return Err(DispatchError::DefinitionMismatch {
                expected: self.definition,
                current: definition.clone(),
            });
        }
        if &self.instance != instance {
            return Err(DispatchError::InstanceMismatch {
                expected: self.instance,
                current: instance.clone(),
            });
        }
        if self.revision != revision {
            return Err(DispatchError::StaleRevision {
                expected: self.revision,
                current: revision,
            });
        }
        Ok(ValidatedDecision {
            candidate: self.candidate,
            outcome: self.outcome,
        })
    }
}

/// A proposal checked against an acceptor's supplied identity and revision.
///
/// This is not evidence of a storage commit. An external adapter owns atomic
/// persistence and may only execute intents after successful durable
/// acceptance.
pub struct ValidatedDecision<D, X = (), O = (), I: MachineIds = crate::DefaultIds> {
    candidate: Option<D>,
    outcome: DispatchOutcome<X, O, I>,
}

impl<D, X, O, I: MachineIds> ValidatedDecision<D, X, O, I> {
    /// Consume the checked proposal without cloning candidate data or payloads.
    ///
    /// Transitioned outcomes carry `Some(candidate)`; refusals and ignored
    /// events carry `None` and never advance the accepted revision.
    pub fn into_parts(self) -> (Option<D>, DispatchOutcome<X, O, I>) {
        (self.candidate, self.outcome)
    }
}
