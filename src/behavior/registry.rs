use std::collections::HashMap;

use super::implementation::Implementation;
use super::{
    BehaviorRef,
    BehaviorRole,
    BindingError,
};

const MAX_BINDINGS: usize = 4096;

type Entries<D, E, F, X, O, I> = HashMap<BehaviorRef<I>, Implementation<D, E, F, X, O, I>>;
type ResolvedImplementation<'a, D, E, F, X, O, I> =
    Result<&'a Implementation<D, E, F, X, O, I>, BindingError<I>>;

/// Immutable, caller-owned registry of typed Story implementations.
///
/// Rust type parameters fix data/event/facts/effect/output signatures. Roles
/// distinguish callback shapes. Registration and resolution execute no
/// callbacks. Multiple implementations of a Story must have the same signature.
/// Repeated references reuse a declaration; duplicate declarations are errors.
/// Captures and identities must have caller-bounded size/cost and obey callback
/// purity contracts. This registry performs no discovery, code loading or I/O.
///
/// Callback payloads must match the registry's Rust signature:
///
/// ```compile_fail
/// use status_machina::{BehaviorId, BehaviorRef, BehaviorRegistry, ImplementationId};
/// let reference = BehaviorRef::new(BehaviorId::new(1), ImplementationId::new(1));
/// let registry = BehaviorRegistry::<u64, (), (), u64>::builder()
///     .effect(reference, |_, _, _| String::from("wrong payload"));
/// ```
pub struct BehaviorRegistry<D, E, F, X = (), O = (), I: crate::MachineIds = crate::DefaultIds> {
    // bounded: builder caps total declarations at MAX_BINDINGS.
    entries: Entries<D, E, F, X, O, I>,
}
impl<D, E, F, X, O, I: crate::MachineIds> BehaviorRegistry<D, E, F, X, O, I> {
    /// Configure a registry with a default maximum of 128 declarations.
    pub fn builder() -> BehaviorRegistryBuilder<D, E, F, X, O, I> {
        BehaviorRegistryBuilder {
            entries: HashMap::new(),
            max_bindings: 128,
        }
    }
    pub(super) fn lookup(
        &self,
        reference: &BehaviorRef<I>,
        role: BehaviorRole,
    ) -> ResolvedImplementation<'_, D, E, F, X, O, I> {
        let Some(implementation) = self.entries.get(reference) else {
            if self
                .entries
                .keys()
                .any(|entry| entry.behavior() == reference.behavior())
            {
                return Err(BindingError::UnknownImplementation {
                    reference: reference.clone(),
                });
            }
            return Err(BindingError::UnknownBehavior {
                behavior: reference.behavior().clone(),
            });
        };
        if implementation.role() != role {
            return Err(BindingError::SignatureMismatch {
                reference: reference.clone(),
                expected: role,
                actual: implementation.role(),
            });
        }
        Ok(implementation)
    }
}

/// Bounded registry builder for trusted caller-supplied implementations.
pub struct BehaviorRegistryBuilder<
    D,
    E,
    F,
    X = (),
    O = (),
    I: crate::MachineIds = crate::DefaultIds,
> {
    // bounded: max_bindings is positive and capped at MAX_BINDINGS.
    entries: Entries<D, E, F, X, O, I>,
    max_bindings: usize,
}
impl<D, E, F, X, O, I: crate::MachineIds> BehaviorRegistryBuilder<D, E, F, X, O, I> {
    /// Set the total declaration limit across roles and implementation
    /// versions.
    ///
    /// # Errors
    /// Rejects zero, values above 4096 and values below the existing count.
    pub fn max_bindings(mut self, limit: usize) -> Result<Self, BindingError<I>> {
        if limit == 0 || limit > MAX_BINDINGS {
            return Err(BindingError::InvalidLimit);
        }
        if limit < self.entries.len() {
            return Err(BindingError::LimitBelowCurrentSize);
        }
        self.max_bindings = limit;
        Ok(self)
    }
    pub(super) fn insert(
        mut self,
        reference: BehaviorRef<I>,
        implementation: Implementation<D, E, F, X, O, I>,
    ) -> Result<Self, BindingError<I>> {
        if self.entries.contains_key(&reference) {
            return Err(BindingError::DuplicateImplementation { reference });
        }
        if let Some(existing) = self
            .entries
            .iter()
            .find_map(|(entry, value)| (entry.behavior() == reference.behavior()).then_some(value))
            && existing.role() != implementation.role()
        {
            return Err(BindingError::SignatureMismatch {
                reference,
                expected: existing.role(),
                actual: implementation.role(),
            });
        }
        if self.entries.len() == self.max_bindings {
            return Err(BindingError::LimitExceeded {
                limit: self.max_bindings,
            });
        }
        self.entries.insert(reference, implementation);
        Ok(self)
    }
    /// Finish registration without invoking any implementation.
    pub fn build(self) -> BehaviorRegistry<D, E, F, X, O, I> {
        BehaviorRegistry {
            entries: self.entries,
        }
    }
}
