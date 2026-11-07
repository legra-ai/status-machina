use std::any::TypeId;
use std::collections::{
    HashMap,
    HashSet,
};

use super::{
    DEFAULT_MAX_INSTANCES,
    HARD_MAX_INSTANCE_EVENT_IDS,
    HARD_MAX_INSTANCES,
    HARD_MAX_REGISTRY_EVENT_ENTRIES,
};
use crate::MachineInstance;

/// Caller-owned address for one machine instance in one routing scope.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct InstanceAddress<I: crate::MachineIds = crate::DefaultIds> {
    scope: I::Scope,
    instance: I::Instance,
}

impl<I: crate::MachineIds> InstanceAddress<I> {
    /// Construct an address from an explicit scope and instance identity.
    #[must_use]
    pub const fn new(scope: I::Scope, instance: I::Instance) -> Self {
        Self { scope, instance }
    }

    /// Return the caller-owned routing scope.
    #[must_use]
    pub const fn scope(&self) -> &I::Scope {
        &self.scope
    }

    /// Return the machine instance identity.
    #[must_use]
    pub fn instance_id(&self) -> I::Instance {
        self.instance.clone()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct InstanceSignature<I: crate::MachineIds = crate::DefaultIds> {
    pub(super) input_payload: TypeId,
    pub(super) output_payload: TypeId,
    pub(super) input_events: HashSet<I::Event>,
    pub(super) output_events: HashSet<I::Event>,
}

/// Explicit, bounded registry of caller-owned machine-instance signatures.
#[derive(Clone)]
pub struct InstanceRegistry<I: crate::MachineIds = crate::DefaultIds> {
    // bounded: entries are capped by HARD_MAX_INSTANCES.
    pub(super) entries: HashMap<InstanceAddress<I>, InstanceSignature<I>>,
}

impl<I: crate::MachineIds> InstanceRegistry<I> {
    /// Start building a bounded registry.
    #[must_use]
    pub fn builder() -> InstanceRegistryBuilder<I> {
        InstanceRegistryBuilder {
            entries: HashMap::with_capacity(DEFAULT_MAX_INSTANCES),
            max_instances: DEFAULT_MAX_INSTANCES,
            event_entries: 0,
        }
    }

    pub(super) fn contains(&self, address: &InstanceAddress<I>) -> bool {
        self.entries.contains_key(address)
    }

    pub(super) fn signature(&self, address: &InstanceAddress<I>) -> Option<&InstanceSignature<I>> {
        self.entries.get(address)
    }
}

/// Builder for a bounded instance-signature registry.
pub struct InstanceRegistryBuilder<I: crate::MachineIds = crate::DefaultIds> {
    // bounded: max_instances is capped at HARD_MAX_INSTANCES.
    entries: HashMap<InstanceAddress<I>, InstanceSignature<I>>,
    max_instances: usize,
    event_entries: usize,
}

impl<I: crate::MachineIds> InstanceRegistryBuilder<I> {
    /// Set the positive registry size bound.
    ///
    /// # Errors
    ///
    /// Returns an error for zero, a value above the implementation ceiling, or
    /// a value below the number of registered instances.
    pub fn max_instances(
        mut self,
        max_instances: usize,
    ) -> Result<Self, InstanceRegistryBuildError> {
        if max_instances == 0 || max_instances > HARD_MAX_INSTANCES {
            return Err(InstanceRegistryBuildError::InvalidLimit);
        }
        if self.entries.len() > max_instances {
            return Err(InstanceRegistryBuildError::LimitBelowCurrentSize);
        }
        self.max_instances = max_instances;
        Ok(self)
    }

    /// Register one source or destination against the actual machine instance.
    ///
    /// The registry records only identity, event payload type identities and
    /// bounded event identifiers. It retains no reference to the machine.
    ///
    /// # Errors
    ///
    /// Returns an error when the address does not name this instance, the
    /// address is duplicated, or the instance/aggregate event inventory bound
    /// would be exceeded.
    pub fn instance<D, E, F, X, O>(
        mut self,
        address: InstanceAddress<I>,
        instance: &MachineInstance<'_, D, E, F, X, O, I>,
    ) -> Result<Self, InstanceRegistryBuildError>
    where
        D: crate::FallibleCandidateData,
        E: 'static,
        O: 'static,
    {
        if address.instance_id() != instance.instance_id() {
            return Err(InstanceRegistryBuildError::AddressMismatch);
        }
        if self.entries.contains_key(&address) {
            return Err(InstanceRegistryBuildError::DuplicateInstance);
        }
        if self.entries.len() == self.max_instances {
            return Err(InstanceRegistryBuildError::LimitExceeded {
                limit: self.max_instances,
            });
        }

        let mut input_events = HashSet::new();
        for event_id in instance.input_event_ids() {
            Self::insert_event(&mut input_events, event_id, &mut self.event_entries)?;
        }
        let mut output_events = HashSet::new();
        for event_id in instance.output_event_ids() {
            Self::insert_event(&mut output_events, event_id, &mut self.event_entries)?;
        }

        self.entries.insert(
            address,
            InstanceSignature {
                input_payload: TypeId::of::<E>(),
                output_payload: TypeId::of::<O>(),
                input_events,
                output_events,
            },
        );
        Ok(self)
    }

    /// Finish the registry.
    #[must_use]
    pub fn build(self) -> InstanceRegistry<I> {
        InstanceRegistry {
            entries: self.entries,
        }
    }

    fn insert_event(
        target: &mut HashSet<I::Event>,
        event_id: &I::Event,
        aggregate_count: &mut usize,
    ) -> Result<(), InstanceRegistryBuildError> {
        if target.contains(event_id) {
            return Ok(());
        }
        if target.len() == HARD_MAX_INSTANCE_EVENT_IDS {
            return Err(InstanceRegistryBuildError::InstanceEventLimitExceeded {
                limit: HARD_MAX_INSTANCE_EVENT_IDS,
            });
        }
        if *aggregate_count == HARD_MAX_REGISTRY_EVENT_ENTRIES {
            return Err(InstanceRegistryBuildError::AggregateEventLimitExceeded {
                limit: HARD_MAX_REGISTRY_EVENT_ENTRIES,
            });
        }
        target.insert(event_id.clone());
        *aggregate_count += 1;
        Ok(())
    }
}

/// An invalid or over-capacity instance registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum InstanceRegistryBuildError {
    /// A size limit was zero or above the hard implementation ceiling.
    #[error("instance registry limit is outside the supported range")]
    InvalidLimit,
    /// The new size limit is below the already registered count.
    #[error("instance registry limit is below its current size")]
    LimitBelowCurrentSize,
    /// The address instance identity differs from the supplied machine
    /// instance.
    #[error("instance address identity does not match the machine instance")]
    AddressMismatch,
    /// The address was already registered.
    #[error("machine instance address is already registered")]
    DuplicateInstance,
    /// The configured registry count was reached.
    #[error("instance registry limit {limit} exceeded")]
    LimitExceeded {
        /// Configured maximum registry size.
        limit: usize,
    },
    /// The instance's unique input or output event inventory reached its bound.
    #[error("instance event inventory limit {limit} exceeded")]
    InstanceEventLimitExceeded {
        /// Per-instance event ID maximum.
        limit: usize,
    },
    /// The aggregate copied event-ID inventory reached its registry-wide bound.
    #[error("registry aggregate event inventory limit {limit} exceeded")]
    AggregateEventLimitExceeded {
        /// Registry-wide input and output event ID maximum.
        limit: usize,
    },
}
