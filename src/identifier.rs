use std::fmt;

use thiserror::Error;

/// An invalid machine-graph identifier.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{kind} identifier must contain 1 to 128 non-control characters")]
pub struct IdentifierError {
    kind: &'static str,
}

macro_rules! identifier {
    ($name:ident, $kind:literal) => {
        #[doc = concat!("Validated identifier for ", $kind, ".")]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Validate and construct an identifier.
            ///
            /// # Errors
            ///
            /// Returns an error for empty values, values over 128 bytes, or control
            /// characters.
            pub fn try_new(value: impl Into<String>) -> Result<Self, IdentifierError> {
                let value = value.into();
                if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
                    return Err(IdentifierError { kind: $kind });
                }
                Ok(Self(value))
            }

            /// Return the identifier text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

identifier!(MachineId, "machine");
identifier!(StateId, "state");
identifier!(EventId, "event");
identifier!(TransitionId, "transition");
identifier!(EffectId, "effect");
identifier!(ActionId, "action");
identifier!(ConditionId, "condition");
identifier!(DispositionId, "disposition");
identifier!(CapabilityId, "capability");
identifier!(AssertionId, "safety assertion");
identifier!(ScenarioId, "scenario");
identifier!(OperationId, "retry operation");

/// Validated caller-owned namespace for routing machine instances.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RoutingScope(String);

impl RoutingScope {
    /// Validate and construct a routing scope.
    ///
    /// # Errors
    ///
    /// Returns an error for empty values, values over 128 bytes, or control
    /// characters.
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
            return Err(IdentifierError {
                kind: "routing scope",
            });
        }
        Ok(Self(value))
    }

    /// Return the scope text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RoutingScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Caller-supplied `UUIDv7` identity for one machine instance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MachineInstanceId(uuid::Uuid);

impl MachineInstanceId {
    /// Validate and wrap a non-nil `UUIDv7` value.
    ///
    /// # Errors
    ///
    /// Returns an error when the value is nil or is not UUID version 7.
    pub fn try_new(value: uuid::Uuid) -> Result<Self, IdentifierError> {
        if value.is_nil() || value.get_version_num() != 7 {
            return Err(IdentifierError {
                kind: "machine instance",
            });
        }
        Ok(Self(value))
    }

    /// Return the wrapped UUID.
    #[must_use]
    pub const fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
    }
}
