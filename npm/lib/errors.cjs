'use strict';
class StatusMachinaError extends Error {
  constructor(message, context = {}, options) {
    super(message, options);
    this.name = new.target.name;
    this.context = Object.freeze({ ...context });
    this.phase = context.phase;
  }
}
class DefinitionError extends StatusMachinaError {}
class DispatchError extends StatusMachinaError {}
class CallbackError extends DispatchError {}
class SnapshotError extends CallbackError {}
class InvariantError extends DispatchError {}
class PostconditionError extends DispatchError {}
class AmbiguousTransitionError extends DispatchError {}
class ActionError extends DispatchError {}
class StaleDecisionError extends DispatchError {}
class ForeignDecisionError extends DispatchError {}
class RevisionError extends DispatchError {}
class DisposedError extends StatusMachinaError {}
class BusyError extends StatusMachinaError {}
class LimitError extends StatusMachinaError {}
class InspectionError extends StatusMachinaError {}
class RecordError extends DefinitionError {}
class ConditionRecordError extends RecordError {}
class TransitionRecordError extends RecordError {}
class DispositionRecordError extends RecordError {}
/** Exact Story resolution failed, for a specification, a record or an assertion. */
class BindingError extends DefinitionError {}
class UnknownBehaviorError extends BindingError {}
class UnknownImplementationError extends BindingError {}
class RoleMismatchError extends BindingError {}
class DuplicateImplementationError extends BindingError {}
// Checked construction: `context.reason` is the kernel variant and the context
// names the rejected element natively.
class UnknownStateError extends DefinitionError {}
class DuplicateIdentifierError extends DefinitionError {}
class UnreachableStateError extends DefinitionError {}
class TerminalTransitionError extends DefinitionError {}
/** `context.transition` joins the forbidden `context.source` and `context.target`. */
class ForbiddenTransitionError extends DefinitionError {}
class InternalTransitionError extends DefinitionError {}
class InvalidConditionError extends DefinitionError {}
class InvalidDispositionError extends DefinitionError {}
class DefinitionLimitError extends DefinitionError {}
class InitialInvariantError extends DefinitionError {}
class UnknownCapabilityError extends DefinitionError {}
class IdentityMapError extends StatusMachinaError {}
class IdentityRoleError extends IdentityMapError {}
class IdentityMapLimitError extends IdentityMapError {}
class DuplicateIdentityError extends IdentityMapError {}
class DuplicateRepresentationError extends IdentityMapError {}
class UnknownIdentityError extends IdentityMapError {}
class UnknownRepresentationError extends IdentityMapError {}
class CompositionError extends StatusMachinaError {}
class InstanceRegistryError extends CompositionError {}
class InstanceRegistryLimitError extends InstanceRegistryError {}
class AddressMismatchError extends InstanceRegistryError {}
class DuplicateInstanceError extends InstanceRegistryError {}
class CompositionDefinitionError extends CompositionError {}
class CompositionLimitError extends CompositionDefinitionError {}
class DuplicateRouteError extends CompositionDefinitionError {}
class UnregisteredRouteAddressError extends CompositionDefinitionError {}
class UndeclaredRouteEventError extends CompositionDefinitionError {}
class UndeclaredCapabilityError extends CompositionDefinitionError {}
/** `outputs` holds every intent of the refused batch, unconsumed. */
class RouteBatchError extends CompositionError {
  constructor(message, context = {}, options) { super(message, context, options); this.outputs = context.outputs ?? Object.freeze([]); }
}
class UnregisteredSourceError extends RouteBatchError {}
class BatchLimitError extends RouteBatchError {}
class CompositionBindingError extends CompositionError {}
class DuplicateBindingError extends CompositionBindingError {}
class UnregisteredBindingError extends CompositionBindingError {}
class BindingMismatchError extends CompositionBindingError {}
class MissingBindingError extends CompositionBindingError {}
class AliasedBindingError extends CompositionBindingError {}
class PropagationLimitsError extends CompositionError {}
class CapabilitySnapshotError extends CompositionError {}
class CapabilitySnapshotLimitError extends CapabilitySnapshotError {}
class ReportInstanceMismatchError extends CapabilitySnapshotError {}
class DuplicateObservationError extends CapabilitySnapshotError {}
class ForeignSnapshotError extends CapabilitySnapshotError {}
/** A retry policy or wait set was rejected; `context.reason` is the kernel variant. */
class RetryError extends StatusMachinaError {}
class RetryPolicyError extends RetryError {}
class ZeroDelayError extends RetryPolicyError {}
class TooFewAttemptsError extends RetryPolicyError {}
class CapBelowInitialError extends RetryPolicyError {}
/** The kernel refused a wait; the wait set is unchanged. */
class RetryWaitError extends RetryError {}
class InvalidWaitCapacityError extends RetryWaitError {}
class WaitCapacityReachedError extends RetryWaitError {}
class WaitGenerationExhaustedError extends RetryWaitError {}
/** A value lies outside its kernel type; `context.field` names it and `context.value` holds it. */
class RetryRangeError extends RetryError {}
/** Scenario replay was rejected by the kernel; `context.reason` is the kernel variant. */
class ScenarioError extends StatusMachinaError {}
class ScenarioLimitsError extends ScenarioError {}
/** `context.entry` is the refused scenario's position and `context.scenario` its identity. */
class ScenarioSetError extends ScenarioError {}
class EmptyScenarioSetError extends ScenarioSetError {}
class ScenarioCountLimitError extends ScenarioSetError {}
class ScenarioStepLimitError extends ScenarioSetError {}
class ScenarioNodeLimitError extends ScenarioSetError {}
class ScenarioAssertionError extends ScenarioError {}
class MissingAssertionPredicateError extends ScenarioAssertionError {}
class ScenarioReplayError extends ScenarioError {}
class AssertionLimitError extends ScenarioReplayError {}
const types = Object.freeze({ inspection: InspectionError, definition: DefinitionError, callback: CallbackError,
  invariant: InvariantError, postcondition: PostconditionError, ambiguous: AmbiguousTransitionError,
  action: ActionError, stale_revision: StaleDecisionError, foreign_definition: ForeignDecisionError,
  foreign_instance: ForeignDecisionError, revision_exhausted: RevisionError, disposed: DisposedError, busy: BusyError,
  record: RecordError, record_condition: ConditionRecordError, record_transition: TransitionRecordError,
  record_disposition: DispositionRecordError, unknown_behavior: UnknownBehaviorError,
  unknown_implementation: UnknownImplementationError, role_mismatch: RoleMismatchError,
  duplicate_implementation: DuplicateImplementationError, unknown_state: UnknownStateError,
  duplicate_identifier: DuplicateIdentifierError, unreachable_state: UnreachableStateError,
  terminal_transition: TerminalTransitionError, internal_transition: InternalTransitionError,
  invalid_condition: InvalidConditionError, invalid_disposition: InvalidDispositionError,
  definition_limit: DefinitionLimitError, initial_invariant: InitialInvariantError,
  unknown_capability: UnknownCapabilityError, forbidden_transition: ForbiddenTransitionError });
const contextRoles = Object.freeze(['state', 'condition', 'transition', 'disposition', 'action', 'effect', 'event', 'capability',
  'story', 'implementation']);
/** True for a typed failure thrown by the native addon. */
function isNative(error) {
  return error !== null && typeof error === 'object' && error.code === 'GenericFailure'
    && typeof error.message === 'string' && error.message.startsWith('{');
}
function translate(error, ids) {
  if (!isNative(error)) throw error;
  throw exception(error, ids);
}
/** The typed exception for a native failure; its `cause` is the original. */
function exception(error, ids) {
  const info = JSON.parse(error.message);
  const Type = info.code === 'callback' && info.phase === 'snapshot' ? SnapshotError : types[info.code];
  if (Type === undefined) throw new Error('Unrecognized native failure category', { cause: error });
  const context = {};
  for (const [name, value] of Object.entries(info)) {
    if (name !== 'code' && name !== 'message' && value !== null) context[name] = value;
  }
  for (const role of contextRoles) {
    if (Object.hasOwn(context, role) && ids !== undefined) context[role] = ids.resolve(role, context[role]);
  }
  // Forbidden-edge endpoints are states.
  for (const name of ['source', 'target']) {
    if (Object.hasOwn(context, name) && ids !== undefined) context[name] = ids.resolve('state', context[name]);
  }
  for (const name of ['expected', 'actual']) {
    // BehaviorRole values are the kernel's wire names, so they map one to one.
    if (Object.hasOwn(context, `${name}_role`)) { context[name] = context[`${name}_role`]; delete context[`${name}_role`]; }
  }
  for (const name of ['expected_revision', 'current_revision']) {
    if (Object.hasOwn(context, name)) context[name === 'expected_revision' ? 'expectedRevision' : 'currentRevision'] = BigInt(context[name]);
    delete context[name];
  }
  return new Type(info.message, context, { cause: error.cause });
}
module.exports = { StatusMachinaError, DefinitionError, DispatchError, CallbackError, SnapshotError,
  InvariantError, PostconditionError, AmbiguousTransitionError, ActionError, StaleDecisionError,
  ForeignDecisionError, RevisionError, DisposedError, BusyError, LimitError, InspectionError,
  RecordError, ConditionRecordError, TransitionRecordError, DispositionRecordError, BindingError,
  UnknownBehaviorError, UnknownImplementationError, RoleMismatchError, IdentityMapError, IdentityRoleError,
  IdentityMapLimitError, DuplicateIdentityError, DuplicateRepresentationError, UnknownIdentityError,
  UnknownRepresentationError, CompositionError, InstanceRegistryError, InstanceRegistryLimitError, AddressMismatchError, DuplicateInstanceError, CompositionDefinitionError, CompositionLimitError, DuplicateRouteError, UnregisteredRouteAddressError, UndeclaredRouteEventError, UndeclaredCapabilityError, RouteBatchError, UnregisteredSourceError, BatchLimitError, CompositionBindingError, DuplicateBindingError, UnregisteredBindingError, BindingMismatchError, MissingBindingError, AliasedBindingError, PropagationLimitsError, CapabilitySnapshotError, CapabilitySnapshotLimitError, ReportInstanceMismatchError, DuplicateObservationError, ForeignSnapshotError, RetryError, RetryPolicyError, ZeroDelayError, TooFewAttemptsError, CapBelowInitialError,
  RetryWaitError, InvalidWaitCapacityError, WaitCapacityReachedError, WaitGenerationExhaustedError, RetryRangeError,
  ScenarioError, ScenarioLimitsError, ScenarioSetError, EmptyScenarioSetError, ScenarioCountLimitError,
  ScenarioStepLimitError, ScenarioNodeLimitError, ScenarioAssertionError, MissingAssertionPredicateError,
  ScenarioReplayError, AssertionLimitError, DuplicateImplementationError, UnknownStateError, DuplicateIdentifierError,
  UnreachableStateError, TerminalTransitionError, ForbiddenTransitionError, InternalTransitionError, InvalidConditionError,
  InvalidDispositionError, DefinitionLimitError, InitialInvariantError, UnknownCapabilityError,
  translate, exception, isNative };
