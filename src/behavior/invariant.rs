use std::sync::Arc;

use super::BehaviorRef;

pub(super) type InvariantPredicate<D> =
    Arc<dyn Fn(&D) -> Result<bool, crate::EvaluationError> + Send + Sync>;

/// State invariant resolved from a typed Story and implementation reference.
pub struct BoundInvariant<D, I: crate::MachineIds = crate::DefaultIds> {
    reference: BehaviorRef<I>,
    predicate: InvariantPredicate<D>,
}
impl<D, I: crate::MachineIds> BoundInvariant<D, I> {
    pub(super) fn new(reference: BehaviorRef<I>, predicate: InvariantPredicate<D>) -> Self {
        Self {
            reference,
            predicate,
        }
    }
    /// Return the checked Story/implementation selection.
    pub const fn binding(&self) -> &BehaviorRef<I> {
        &self.reference
    }
    pub(crate) fn into_parts(self) -> (BehaviorRef<I>, InvariantPredicate<D>) {
        (self.reference, self.predicate)
    }
}
