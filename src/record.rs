//! Borrowed, data-only graph records for checked construction.
mod condition;
mod definition;
mod error;
mod state;
mod transition;
mod usage;
pub use condition::ConditionRecord;
pub use definition::DefinitionRecord;
pub use error::{
    RecordBuildError,
    RecordBuildResult,
};
pub use state::StateRecord;
pub use transition::{
    DispositionRecord,
    TransitionDefinitionRecord,
};
pub use usage::BehaviorUse;
