use std::collections::HashMap;

use crate::Identity;

const MAX_MAPPING_ENTRIES: usize = 4096;

/// Bounded, one-to-one external representation mapping for a single identity
/// role.
///
/// Runtime matching never consults this map. Adapters may use it for names,
/// IRIs or other caller-owned values and then use the ordinary checked
/// builders. It does not serialize executable definitions or resolve behavior
/// bindings.
pub struct IdentityMap<T, R> {
    // bounded: construction checks max_entries, capped at MAX_MAPPING_ENTRIES.
    representations: HashMap<T, R>,
    // bounded: one inverse entry per validated representation.
    identities: HashMap<R, T>,
}

impl<T: Identity, R: Identity> IdentityMap<T, R> {
    /// Validate an explicitly bounded, unambiguous mapping.
    ///
    /// Each identity and representation must occur exactly once. Values and
    /// their clones must themselves have caller-bounded size and cost.
    ///
    /// # Errors
    /// Rejects zero or more than 4096 entries as the configured bound,
    /// duplicate identities/representations, and input exceeding the configured
    /// bound.
    pub fn try_new(
        entries: impl IntoIterator<Item = (T, R)>,
        max_entries: usize,
    ) -> Result<Self, IdentityMapError> {
        if max_entries == 0 || max_entries > MAX_MAPPING_ENTRIES {
            return Err(IdentityMapError::InvalidLimit);
        }
        let mut mapping = Self {
            representations: HashMap::new(),
            identities: HashMap::new(),
        };
        for (identity, representation) in entries {
            if mapping.representations.len() == max_entries {
                return Err(IdentityMapError::LimitExceeded { limit: max_entries });
            }
            if mapping.representations.contains_key(&identity) {
                return Err(IdentityMapError::DuplicateIdentity);
            }
            if mapping.identities.contains_key(&representation) {
                return Err(IdentityMapError::DuplicateRepresentation);
            }
            mapping
                .identities
                .insert(representation.clone(), identity.clone());
            mapping.representations.insert(identity, representation);
        }
        Ok(mapping)
    }

    /// Look up an explicitly registered external representation.
    ///
    /// # Errors
    /// Rejects unknown identities rather than deriving a name from
    /// debug/display.
    pub fn encode(&self, identity: &T) -> Result<&R, IdentityMapError> {
        self.representations
            .get(identity)
            .ok_or(IdentityMapError::UnknownIdentity)
    }

    /// Resolve an external representation to its typed identity.
    ///
    /// # Errors
    /// Rejects unknown external values without substituting a default.
    pub fn decode(&self, representation: &R) -> Result<&T, IdentityMapError> {
        self.identities
            .get(representation)
            .ok_or(IdentityMapError::UnknownRepresentation)
    }
}

/// Invalid or unknown entries in an explicit identity representation mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum IdentityMapError {
    /// The requested bound is zero or exceeds the implementation ceiling.
    #[error("identity mapping limit must be between 1 and 4096")]
    InvalidLimit,
    /// The iterator supplied more entries than the configured bound.
    #[error("identity mapping limit {limit} exceeded")]
    LimitExceeded {
        /// Configured entry maximum.
        limit: usize,
    },
    /// One identity was supplied more than once.
    #[error("duplicate mapped identity")]
    DuplicateIdentity,
    /// Distinct entries used the same external representation.
    #[error("duplicate identity representation")]
    DuplicateRepresentation,
    /// An identity has no registered external representation.
    #[error("unknown mapped identity")]
    UnknownIdentity,
    /// An external representation has no registered identity.
    #[error("unknown identity representation")]
    UnknownRepresentation,
}
