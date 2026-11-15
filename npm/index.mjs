import api from './index.cjs';
export const { StatusMachinaError, DefinitionError, DispatchError, CallbackError, SnapshotError,
  InvariantError, PostconditionError, AmbiguousTransitionError, ActionError, StaleDecisionError,
  ForeignDecisionError, RevisionError, DisposedError, BusyError, LimitError, InspectionError,
  RecordError, ConditionRecordError, TransitionRecordError, DispositionRecordError, BindingError,
  UnknownBehaviorError, UnknownImplementationError, RoleMismatchError, IdentityMapError, IdentityRoleError,
  IdentityMapLimitError, DuplicateIdentityError, DuplicateRepresentationError, UnknownIdentityError,
  UnknownRepresentationError,
  CapabilityStatus, Definition, Machine, Decision, BehaviorRef, BehaviorRegistry, BehaviorRole,
  BehaviorUse, PredicateRecord, AllRecord, AnyRecord, NotRecord, StateRecord, TransitionDefinitionRecord,
  DispositionRecord, DefinitionRecord, IdentityRole, IdentityMap, MachineIdentityMaps } = api;
