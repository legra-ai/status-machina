//! Exclusive borrowed execution of already validated connections.
mod propagation;
mod report;
mod target;

use std::collections::HashMap;

pub use report::{
    AddressedEffect,
    Delivery,
    PropagationLimits,
    PropagationLimitsBuilder,
    PropagationLimitsError,
    PropagationReport,
    PropagationStatus,
    PropagationStep,
};
use target::Target;

use super::{
    Composition,
    InstanceAddress,
    InstanceRegistry,
};
use crate::{
    CandidateData,
    MachineIds,
    MachineInstance,
};

/// Exclusive mutable bindings for synchronous, bounded event propagation.
///
/// Instances may have different data types, but share event/effect payload and
/// fact types. No machine or background work is owned here. The borrowed
/// instances cannot be accessed independently until this binding is dropped.
/// External effects must run after releasing the binding and any caller lock.
///
/// Overlapping dispatch cannot compile:
/// ```compile_fail,E0499
/// use status_machina::{Composition, InstanceAddress, MachineInstance};
/// fn overlap(composition: &Composition<()>, address: InstanceAddress,
///     instance: &mut MachineInstance<'_, (), (), (), (), ()>) {
///     let builder = composition.bind().instance(address.clone(), instance).unwrap();
///     let _other = composition.bind().instance(address, instance).unwrap();
///     let _bound = builder.build().unwrap();
/// }
/// ```
pub struct BoundComposition<'a, E, F, X, I: MachineIds = crate::DefaultIds> {
    composition: &'a Composition<E, I>,
    // bounded: subset of the two registries, each capped at 512 entries.
    targets: HashMap<InstanceAddress<I>, &'a mut dyn Target<E, F, X, I>>,
}

impl<E, I: MachineIds> Composition<E, I> {
    /// Bind mutable instances to the existing declared connections.
    #[must_use]
    pub fn bind<F, X>(&self) -> BoundCompositionBuilder<'_, E, F, X, I> {
        BoundCompositionBuilder {
            composition: self,
            targets: HashMap::new(),
        }
    }
}

/// Builder checking actual exclusive instances against registered signatures.
pub struct BoundCompositionBuilder<'a, E, F, X, I: MachineIds = crate::DefaultIds> {
    composition: &'a Composition<E, I>,
    // bounded: every entry must belong to the bounded source/destination union.
    targets: HashMap<InstanceAddress<I>, &'a mut dyn Target<E, F, X, I>>,
}

impl<'a, E: 'static, F, X, I: MachineIds> BoundCompositionBuilder<'a, E, F, X, I> {
    /// Bind one actual instance. This borrows exclusively without taking locks.
    ///
    /// # Errors
    /// Refuses unknown/duplicate addresses, mismatched identities or
    /// signatures.
    pub fn instance<D: CandidateData>(
        mut self,
        address: InstanceAddress<I>,
        instance: &'a mut MachineInstance<'_, D, E, F, X, E, I>,
    ) -> Result<Self, BindingError> {
        if self.targets.contains_key(&address) {
            return Err(BindingError::DuplicateInstance);
        }
        let source = self.composition.sources.signature(&address);
        let destination = self.composition.destinations.signature(&address);
        if source.is_none() && destination.is_none() {
            return Err(BindingError::UnregisteredInstance);
        }
        let actual = InstanceRegistry::builder()
            .instance(address.clone(), instance)
            .map_err(|_| BindingError::IdentityOrInventoryMismatch)?
            .build();
        let signature = actual
            .signature(&address)
            .ok_or(BindingError::IdentityOrInventoryMismatch)?;
        if source.is_some_and(|expected| expected != signature)
            || destination.is_some_and(|expected| expected != signature)
        {
            return Err(BindingError::SignatureMismatch);
        }
        self.targets.insert(address, instance);
        Ok(self)
    }

    /// Finish only when every registered instance is bound.
    ///
    /// # Errors
    /// Refuses missing bindings before any machine can be dispatched.
    pub fn build(self) -> Result<BoundComposition<'a, E, F, X, I>, BindingError> {
        for address in self
            .composition
            .sources
            .entries
            .keys()
            .chain(self.composition.destinations.entries.keys())
        {
            if !self.targets.contains_key(address) {
                return Err(BindingError::MissingInstance);
            }
        }
        Ok(BoundComposition {
            composition: self.composition,
            targets: self.targets,
        })
    }
}

/// Invalid actual bindings for a declared composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BindingError {
    /// An address was bound twice.
    #[error("duplicate bound instance")]
    DuplicateInstance,
    /// The address is outside both registries.
    #[error("unregistered bound instance")]
    UnregisteredInstance,
    /// Address identity differs or the instance exceeds signature inventory
    /// bounds.
    #[error("bound instance identity or inventory mismatch")]
    IdentityOrInventoryMismatch,
    /// The registered and actual input/output signatures differ.
    #[error("bound instance signature mismatch")]
    SignatureMismatch,
    /// A registered instance has no exclusive binding.
    #[error("missing bound instance")]
    MissingInstance,
}
