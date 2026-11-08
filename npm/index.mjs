import api from './index.cjs';
export const { StatusMachinaError, DefinitionError, DispatchError, CallbackError, SnapshotError,
  InvariantError, PostconditionError, AmbiguousTransitionError, ActionError, StaleDecisionError,
  ForeignDecisionError, RevisionError, DisposedError, BusyError, LimitError, InspectionError,
  Definition, Machine, Decision, BehaviorRef, BehaviorRegistry, BehaviorRole } = api;
