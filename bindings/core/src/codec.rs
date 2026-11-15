//! Role-aware identity codecs validated by the kernel's own `IdentityMap`.
//!
//! Each language interns native identities and representations into private
//! per-codec indexes, using its own equality (Python type-qualified hashing,
//! JavaScript `Map` semantics). The kernel map owns the one-to-one and bound
//! rules, so both languages share one construction and lookup contract.
use serde::Serialize;
use status_machina::{
    IdentityMap,
    IdentityMapError,
};

/// One bounded one-to-one mapping between private identity and representation
/// indexes for a single identity role.
pub struct Codec {
    map: IdentityMap<u32, u32>,
}
impl Codec {
    /// Validate `(identity, representation)` index pairs in caller order.
    ///
    /// # Errors
    /// Returns the kernel's rejection and, for entry errors, the zero-based
    /// position of the first offending pair.
    pub fn try_new(pairs: &[(u32, u32)], max_entries: usize) -> Result<Self, CodecFailure> {
        let mut consumed = 0_usize;
        let entries = pairs.iter().inspect(|_| consumed += 1).copied();
        IdentityMap::try_new(entries, max_entries)
            .map(|map| Self { map })
            .map_err(|error| CodecFailure::new(error, consumed.checked_sub(1)))
    }
    /// Return the representation index registered for an identity index.
    ///
    /// # Errors
    /// Rejects identities absent from this codec.
    pub fn encode(&self, identity: u32) -> Result<u32, CodecFailure> {
        self.map
            .encode(&identity)
            .copied()
            .map_err(|error| CodecFailure::new(error, None))
    }
    /// Return the identity index registered for a representation index.
    ///
    /// # Errors
    /// Rejects representations absent from this codec.
    pub fn decode(&self, representation: u32) -> Result<u32, CodecFailure> {
        self.map
            .decode(&representation)
            .copied()
            .map_err(|error| CodecFailure::new(error, None))
    }
}

/// Typed codec rejection crossing to the native exception translators.
#[derive(Debug, Serialize)]
pub struct CodecFailure {
    pub code: &'static str,
    pub reason: &'static str,
    pub message: String,
    pub limit: Option<usize>,
    /// Zero-based position of the offending entry, for construction errors.
    pub entry: Option<usize>,
}
impl CodecFailure {
    fn new(error: IdentityMapError, position: Option<usize>) -> Self {
        let (reason, limit, entry) = match error {
            IdentityMapError::InvalidLimit => ("invalid_limit", None, None),
            IdentityMapError::LimitExceeded { limit } => ("limit_exceeded", Some(limit), position),
            IdentityMapError::DuplicateIdentity => ("duplicate_identity", None, position),
            IdentityMapError::DuplicateRepresentation => {
                ("duplicate_representation", None, position)
            }
            IdentityMapError::UnknownIdentity => ("unknown_identity", None, None),
            IdentityMapError::UnknownRepresentation => ("unknown_representation", None, None),
        };
        Self {
            code: "identity_map",
            reason,
            message: error.to_string(),
            limit,
            entry,
        }
    }
    /// Serialize for a native exception translator.
    #[must_use]
    pub fn encode(&self) -> String {
        serde_json::to_string(self).expect("typed codec failure metadata is serializable")
    }
}
