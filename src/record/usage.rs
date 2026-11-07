use crate::{
    BehaviorRef,
    DefaultIds,
    MachineIds,
};

/// One graph use site selecting an exact Story implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BehaviorUse<T, I: MachineIds = DefaultIds> {
    /// Graph-local use-site identity, independent of reusable Story identity.
    pub id: T,
    /// Selected Story and implementation identities.
    pub reference: BehaviorRef<I>,
}
impl<T, I: MachineIds> BehaviorUse<T, I> {
    /// Construct a typed reference without resolving or executing it.
    pub const fn new(id: T, reference: BehaviorRef<I>) -> Self {
        Self { id, reference }
    }
}
