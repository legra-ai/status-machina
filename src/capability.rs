use std::sync::Arc;

use crate::MachineInstance;

type CapabilityEvaluator<D, F, I = crate::DefaultIds> =
    Arc<dyn Fn(&<I as crate::MachineIds>::State, &D, &F) -> CapabilityStatus + Send + Sync>;

/// Advisory result of a named capability policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityStatus {
    /// The supplied state and facts satisfy this capability policy.
    Available,
    /// The supplied state and facts do not satisfy this capability policy.
    Unavailable,
    /// The supplied facts are insufficient or stale for a determination.
    Unknown,
}

/// Named, pure policy evaluated against one machine's current state and data.
pub struct Capability<D, F, I: crate::MachineIds = crate::DefaultIds> {
    id: I::Capability,
    evaluate: CapabilityEvaluator<D, F, I>,
    binding: Option<crate::BehaviorRef<I>>,
}

impl<D, F, I: crate::MachineIds> Capability<D, F, I> {
    /// Return the registered Story/implementation reference, if bound by a
    /// registry.
    pub const fn binding(&self) -> Option<&crate::BehaviorRef<I>> {
        self.binding.as_ref()
    }

    pub(crate) fn bound(
        id: I::Capability,
        binding: crate::BehaviorRef<I>,
        evaluate: CapabilityEvaluator<D, F, I>,
    ) -> Self {
        Self {
            id,
            binding: Some(binding),
            evaluate,
        }
    }

    /// Start building a named capability policy.
    #[must_use]
    pub fn builder(id: I::Capability) -> CapabilityBuilder<D, F, I> {
        CapabilityBuilder { id, evaluate: None }
    }
}

/// Builder for a pure named capability policy.
pub struct CapabilityBuilder<D, F, I: crate::MachineIds = crate::DefaultIds> {
    id: I::Capability,
    evaluate: Option<CapabilityEvaluator<D, F, I>>,
}

impl<D, F, I: crate::MachineIds> CapabilityBuilder<D, F, I> {
    /// Bind a pure policy over the observed state, data and caller facts.
    #[must_use]
    pub fn evaluate(
        mut self,
        evaluate: impl Fn(&I::State, &D, &F) -> CapabilityStatus + Send + Sync + 'static,
    ) -> Self {
        self.evaluate = Some(Arc::new(evaluate));
        self
    }

    /// Build the policy.
    ///
    /// # Errors
    ///
    /// Returns an error when no policy implementation was supplied.
    pub fn build(self) -> Result<Capability<D, F, I>, CapabilityBuildError> {
        Ok(Capability {
            binding: None,
            id: self.id,
            evaluate: self
                .evaluate
                .ok_or(CapabilityBuildError::MissingEvaluator)?,
        })
    }
}

/// A capability builder omitted its evaluator.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CapabilityBuildError {
    /// No policy implementation was supplied.
    #[error("capability policy evaluator is missing")]
    MissingEvaluator,
}

/// Capability observation tied to one exact instance identity and revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityReport<I: crate::MachineIds = crate::DefaultIds> {
    capability: I::Capability,
    instance: I::Instance,
    revision: u64,
    status: CapabilityStatus,
}

impl<I: crate::MachineIds> CapabilityReport<I> {
    /// Return the named capability.
    #[must_use]
    pub const fn capability(&self) -> &I::Capability {
        &self.capability
    }

    /// Return the instance identity observed by the policy.
    #[must_use]
    pub fn instance(&self) -> I::Instance {
        self.instance.clone()
    }

    /// Return the machine revision observed by the policy.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Return the advisory result; this is never an authorization grant.
    #[must_use]
    pub const fn status(&self) -> CapabilityStatus {
        self.status
    }
}

impl<'definition, D, E, F, X, O, I: crate::MachineIds>
    MachineInstance<'definition, D, E, F, X, O, I>
where
    D: crate::CandidateData,
{
    /// Evaluate one named capability against this instance's state and
    /// revision.
    #[must_use]
    pub fn evaluate_capability(
        &self,
        capability: &Capability<D, F, I>,
        facts: &F,
    ) -> CapabilityReport<I> {
        CapabilityReport {
            capability: capability.id.clone(),
            instance: self.instance_id(),
            revision: self.revision(),
            status: (capability.evaluate)(self.state(), self.data(), facts),
        }
    }
}
