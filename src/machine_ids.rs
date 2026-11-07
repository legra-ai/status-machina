use std::fmt::Debug;
use std::hash::Hash;

/// Value operations used for identity matching, bounded indexes and
/// diagnostics.
///
/// Identity never depends on display text or serialization. Ordering and label
/// formatting are additional requirements of inspection operations only.
/// Values must have stable equality/hashing; their storage, clone and
/// comparison costs must be bounded by the caller. Interior mutation must not
/// change identity.
pub trait Identity: Clone + Debug + Eq + Hash {}

impl<T: Clone + Debug + Eq + Hash> Identity for T {}

/// Caller-defined identity roles for one machine definition and its instances.
///
/// Use distinct enums or newtypes for distinct roles. The family itself is a
/// marker; its derives support the ordinary value traits of generic records.
/// No identifier needs to implement Display, Serde, or a UUID convention.
///
/// Enums need no display, ordering or serialization implementation:
///
/// ```rust
/// use status_machina::{
///     Event,
///     MachineDefinition,
///     MachineIds,
///     StateDefinition,
///     Transition,
/// };
/// #[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// struct OrderIds;
/// #[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// enum State {
///     Pending,
///     Complete,
/// }
/// #[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// enum Kind {
///     Finish,
/// }
/// #[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// struct Instance(u64);
/// impl MachineIds for OrderIds {
///     type Definition = u16;
///     type Instance = Instance;
///     type State = State;
///     type Event = Kind;
///     type Transition = u8;
///     type Action = u8;
///     type Condition = u8;
///     type Effect = u8;
///     type Disposition = u8;
///     type Capability = u8;
///     type Operation = u8;
///     type Scenario = u8;
///     type Assertion = u8;
///     type Scope = u8;
///     type Behavior = u16;
///     type Implementation = u16;
/// }
/// let transition = Transition::<(), (), (), (), (), OrderIds>::builder(1)
///     .from(State::Pending)
///     .on(Kind::Finish)
///     .to(State::Complete)
///     .build()
///     .unwrap();
/// let definition = MachineDefinition::<(), (), (), (), (), OrderIds>::builder(42)
///     .initial(State::Pending)
///     .state(StateDefinition::builder(State::Pending).build())
///     .unwrap()
///     .state(StateDefinition::builder(State::Complete).build())
///     .unwrap()
///     .transition(transition)
///     .unwrap()
///     .build()
///     .unwrap();
/// let mut instance = definition.instantiate(Instance(7), ()).unwrap();
/// assert!(
///     instance
///         .dispatch(Event::new(Kind::Finish, ()), &())
///         .unwrap()
///         .is_transitioned()
/// );
/// assert_eq!(instance.state(), &State::Complete);
/// ```
///
/// The compiler rejects using an event kind as a state:
///
/// ```compile_fail,E0308
/// # use status_machina::{Event, MachineDefinition, MachineIds, StateDefinition, Transition};
/// # #[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// # struct OrderIds;
/// # #[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// # enum State { Pending, Complete }
/// # #[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// # enum Kind { Finish }
/// # #[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// # struct Instance(u64);
/// # impl MachineIds for OrderIds {
/// #     type Definition = u16;
/// #     type Instance = Instance;
/// #     type State = State;
/// #     type Event = Kind;
/// #     type Transition = u8;
/// #     type Action = u8;
/// #     type Condition = u8;
/// #     type Effect = u8;
/// #     type Disposition = u8;
/// #     type Capability = u8;
/// #     type Operation = u8;
/// #     type Scenario = u8;
/// #     type Assertion = u8;
/// #     type Scope = u8;
/// #     type Behavior = u16;
/// #     type Implementation = u16;
/// # }
/// Transition::<(), (), (), (), (), OrderIds>::builder(1).from(Kind::Finish);
/// ```
///
/// An instance identity cannot stand in for an event kind:
///
/// ```compile_fail,E0308
/// # use status_machina::{Event, MachineDefinition, MachineIds, StateDefinition, Transition};
/// # #[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// # struct OrderIds;
/// # #[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// # enum State { Pending, Complete }
/// # #[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// # enum Kind { Finish }
/// # #[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// # struct Instance(u64);
/// # impl MachineIds for OrderIds {
/// #     type Definition = u16;
/// #     type Instance = Instance;
/// #     type State = State;
/// #     type Event = Kind;
/// #     type Transition = u8;
/// #     type Action = u8;
/// #     type Condition = u8;
/// #     type Effect = u8;
/// #     type Disposition = u8;
/// #     type Capability = u8;
/// #     type Operation = u8;
/// #     type Scenario = u8;
/// #     type Assertion = u8;
/// #     type Scope = u8;
/// #     type Behavior = u16;
/// #     type Implementation = u16;
/// # }
/// let event = Event::<(), OrderIds>::new(Instance(7), ());
/// ```
pub trait MachineIds: Clone + Debug + Eq + Hash {
    /// Reusable behavior identity (a Story in the EKGF model).
    type Behavior: Identity;
    /// Executable implementation identity, distinct from the behavior.
    type Implementation: Identity;
    /// Definition identity.
    type Definition: Identity;
    /// Instance identity, supplied by the caller.
    type Instance: Identity;
    /// Finite control-state identity.
    type State: Identity;
    /// Input/output event kind, separate from its payload.
    type Event: Identity;
    /// Transition identity.
    type Transition: Identity;
    /// Local action identity.
    type Action: Identity;
    /// Guard/postcondition identity.
    type Condition: Identity;
    /// External effect identity.
    type Effect: Identity;
    /// Explicit disposition identity.
    type Disposition: Identity;
    /// Advisory capability identity.
    type Capability: Identity;
    /// Retry operation identity.
    type Operation: Identity;
    /// Replay scenario identity.
    type Scenario: Identity;
    /// Safety assertion identity.
    type Assertion: Identity;
    /// Caller-owned routing namespace.
    type Scope: Identity;
}

/// Convenience family using validated text IDs and caller-supplied `UUIDv7`
/// instances.
///
/// These representation policies belong to this family, not the transition
/// kernel. Other families can use enums, integers or caller-owned newtypes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DefaultIds;

impl MachineIds for DefaultIds {
    type Definition = crate::MachineId;
    type Instance = crate::MachineInstanceId;
    type State = crate::StateId;
    type Event = crate::EventId;
    type Transition = crate::TransitionId;
    type Action = crate::ActionId;
    type Condition = crate::ConditionId;
    type Effect = crate::EffectId;
    type Disposition = crate::DispositionId;
    type Capability = crate::CapabilityId;
    type Operation = crate::OperationId;
    type Scenario = crate::ScenarioId;
    type Assertion = crate::AssertionId;
    type Scope = crate::RoutingScope;
    type Behavior = crate::BehaviorId;
    type Implementation = crate::ImplementationId;
}
