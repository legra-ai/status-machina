//! Bounded inspection of an already validated graph; never evaluates behavior.

use super::MachineDefinition;

mod coverage;
mod diagram;
pub use coverage::{
    StateCoverage,
    StructuralCoverage,
};

/// Bounds applied before inspection allocation and during diagram rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectionLimits {
    max_entries: usize,
    max_output_bytes: usize,
}

impl InspectionLimits {
    /// Build explicit inspection limits.
    #[must_use]
    pub const fn builder() -> InspectionLimitsBuilder {
        InspectionLimitsBuilder {
            max_entries: 4096,
            max_output_bytes: 1_048_576,
        }
    }
}

/// Builder for inspection resource limits.
pub struct InspectionLimitsBuilder {
    max_entries: usize,
    max_output_bytes: usize,
}

impl InspectionLimitsBuilder {
    /// Limit the combined states, transitions and disposition records.
    #[must_use]
    pub const fn max_entries(mut self, limit: usize) -> Self {
        self.max_entries = limit;
        self
    }

    /// Limit the diagram's UTF-8 bytes, including escaped labels.
    #[must_use]
    pub const fn max_output_bytes(mut self, limit: usize) -> Self {
        self.max_output_bytes = limit;
        self
    }

    /// Validate and build the limits.
    ///
    /// # Errors
    /// Rejects zero entry or byte limits.
    pub const fn build(self) -> Result<InspectionLimits, InspectionError> {
        if self.max_entries == 0 || self.max_output_bytes == 0 {
            return Err(InspectionError::EmptyLimit);
        }
        Ok(InspectionLimits {
            max_entries: self.max_entries,
            max_output_bytes: self.max_output_bytes,
        })
    }
}

/// An inspection exceeded its caller-supplied resource budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum InspectionError {
    /// Limits must be positive.
    #[error("inspection limits must be positive")]
    EmptyLimit,
    /// The graph has more entries than inspection permits.
    #[error("inspection entry limit {limit} exceeded")]
    EntryLimit {
        /// Configured maximum total records.
        limit: usize,
    },
    /// The rendered diagram would exceed its byte limit.
    #[error("inspection output byte limit {limit} exceeded")]
    OutputLimit {
        /// Configured maximum rendered bytes.
        limit: usize,
    },
    /// A supposedly validated edge refers to a state absent from the graph.
    #[error("validated graph contains an unknown state endpoint")]
    MissingGraphEndpoint,
}

impl<D, E, F, X, O, I: crate::MachineIds> MachineDefinition<D, E, F, X, O, I> {
    fn check_inspection_limit(&self, limits: &InspectionLimits) -> Result<(), InspectionError> {
        let total = self
            .states
            .len()
            .checked_add(self.transitions.len())
            .and_then(|count| count.checked_add(self.dispositions.len()));
        if total.is_none_or(|count| count > limits.max_entries) {
            return Err(InspectionError::EntryLimit {
                limit: limits.max_entries,
            });
        }
        Ok(())
    }
}
