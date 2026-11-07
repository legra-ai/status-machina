use std::collections::HashMap;

use super::{
    DEFAULT_MAX_CAPABILITY_OBSERVATIONS,
    HARD_MAX_CAPABILITY_OBSERVATIONS,
    InstanceAddress,
};
use crate::{
    CapabilityReport,
    CapabilityStatus,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct CapabilityKey<I: crate::MachineIds = crate::DefaultIds> {
    address: InstanceAddress<I>,
    capability: I::Capability,
}

/// Caller-supplied current revisions and named capability reports.
pub struct CapabilitySnapshot<I: crate::MachineIds = crate::DefaultIds> {
    current_revisions: HashMap<InstanceAddress<I>, u64>,
    reports: HashMap<CapabilityKey<I>, CapabilityReport<I>>,
}

impl<I: crate::MachineIds> CapabilitySnapshot<I> {
    /// Construct an empty snapshot; missing observations are Unknown.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            current_revisions: HashMap::new(),
            reports: HashMap::new(),
        }
    }

    /// Start building a bounded caller-owned snapshot.
    #[must_use]
    pub fn builder() -> CapabilitySnapshotBuilder<I> {
        CapabilitySnapshotBuilder {
            current_revisions: HashMap::new(),
            reports: HashMap::new(),
            max_observations: DEFAULT_MAX_CAPABILITY_OBSERVATIONS,
        }
    }

    pub(super) fn status(
        &self,
        address: &InstanceAddress<I>,
        capability: &I::Capability,
    ) -> CapabilityStatus {
        let key = CapabilityKey {
            address: address.clone(),
            capability: capability.clone(),
        };
        let Some(report) = self.reports.get(&key) else {
            return CapabilityStatus::Unknown;
        };
        let current_revision = self.current_revisions.get(address);
        if report.instance() != address.instance_id()
            || current_revision != Some(&report.revision())
        {
            return CapabilityStatus::Unknown;
        }
        report.status()
    }
}

/// Bounded builder for current-revision and capability observations.
pub struct CapabilitySnapshotBuilder<I: crate::MachineIds = crate::DefaultIds> {
    // bounded: the combined current-revision and report count is capped.
    current_revisions: HashMap<InstanceAddress<I>, u64>,
    reports: HashMap<CapabilityKey<I>, CapabilityReport<I>>,
    max_observations: usize,
}

impl<I: crate::MachineIds> CapabilitySnapshotBuilder<I> {
    /// Set the positive combined observation bound.
    ///
    /// # Errors
    ///
    /// Returns an error when the bound exceeds its hard ceiling or is below
    /// observations already supplied.
    pub fn max_observations(
        mut self,
        max_observations: usize,
    ) -> Result<Self, CapabilitySnapshotBuildError> {
        if max_observations == 0 || max_observations > HARD_MAX_CAPABILITY_OBSERVATIONS {
            return Err(CapabilitySnapshotBuildError::InvalidLimit);
        }
        if self.observation_count() > max_observations {
            return Err(CapabilitySnapshotBuildError::LimitBelowCurrentSize);
        }
        self.max_observations = max_observations;
        Ok(self)
    }

    /// Record the caller's current revision for one exact instance address.
    ///
    /// # Errors
    ///
    /// Returns an error for a duplicate address or a full snapshot.
    pub fn current_revision(
        mut self,
        address: InstanceAddress<I>,
        revision: u64,
    ) -> Result<Self, CapabilitySnapshotBuildError> {
        if self.current_revisions.contains_key(&address) {
            return Err(CapabilitySnapshotBuildError::DuplicateRevision);
        }
        self.ensure_capacity()?;
        self.current_revisions.insert(address, revision);
        Ok(self)
    }

    /// Add one named policy report for its exact instance address.
    ///
    /// # Errors
    ///
    /// Returns an error for an instance mismatch, duplicate report, or full
    /// snapshot.
    pub fn report(
        mut self,
        address: InstanceAddress<I>,
        report: CapabilityReport<I>,
    ) -> Result<Self, CapabilitySnapshotBuildError> {
        if report.instance() != address.instance_id() {
            return Err(CapabilitySnapshotBuildError::InstanceMismatch);
        }
        let key = CapabilityKey {
            address,
            capability: report.capability().clone(),
        };
        if self.reports.contains_key(&key) {
            return Err(CapabilitySnapshotBuildError::DuplicateCapability);
        }
        self.ensure_capacity()?;
        self.reports.insert(key, report);
        Ok(self)
    }

    /// Finish the snapshot.
    #[must_use]
    pub fn build(self) -> CapabilitySnapshot<I> {
        CapabilitySnapshot {
            current_revisions: self.current_revisions,
            reports: self.reports,
        }
    }

    fn ensure_capacity(&self) -> Result<(), CapabilitySnapshotBuildError> {
        if self.observation_count() == self.max_observations {
            return Err(CapabilitySnapshotBuildError::LimitExceeded {
                limit: self.max_observations,
            });
        }
        Ok(())
    }

    fn observation_count(&self) -> usize {
        self.current_revisions.len() + self.reports.len()
    }
}

/// An invalid or over-capacity capability snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CapabilitySnapshotBuildError {
    /// Observation limit is zero or above the hard implementation ceiling.
    #[error("capability snapshot limit is outside the supported range")]
    InvalidLimit,
    /// New observation limit is below existing observations.
    #[error("capability snapshot limit is below its current size")]
    LimitBelowCurrentSize,
    /// Report's instance identity differs from its address.
    #[error("capability report instance does not match its address")]
    InstanceMismatch,
    /// An address already has a current revision observation.
    #[error("current revision for instance address is already recorded")]
    DuplicateRevision,
    /// A capability report already exists for this address and policy.
    #[error("capability report for address and policy is already recorded")]
    DuplicateCapability,
    /// The configured observation count was reached.
    #[error("capability snapshot limit {limit} exceeded")]
    LimitExceeded {
        /// Configured maximum combined observation count.
        limit: usize,
    },
}
