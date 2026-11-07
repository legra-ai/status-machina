mod error;
mod implementation;
mod invariant;
mod reference;
mod register;
mod registry;
mod resolve;

pub use error::BindingError;
pub use invariant::BoundInvariant;
pub use reference::{
    BehaviorId,
    BehaviorRef,
    BehaviorRole,
    ImplementationId,
};
pub use registry::{
    BehaviorRegistry,
    BehaviorRegistryBuilder,
};
