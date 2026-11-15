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
class BindingError extends RecordError {}
class UnknownBehaviorError extends BindingError {}
class UnknownImplementationError extends BindingError {}
class RoleMismatchError extends BindingError {}
class IdentityMapError extends StatusMachinaError {}
class IdentityRoleError extends IdentityMapError {}
class IdentityMapLimitError extends IdentityMapError {}
class DuplicateIdentityError extends IdentityMapError {}
class DuplicateRepresentationError extends IdentityMapError {}
class UnknownIdentityError extends IdentityMapError {}
class UnknownRepresentationError extends IdentityMapError {}
const types = Object.freeze({ inspection: InspectionError, definition: DefinitionError, callback: CallbackError,
  invariant: InvariantError, postcondition: PostconditionError, ambiguous: AmbiguousTransitionError,
  action: ActionError, stale_revision: StaleDecisionError, foreign_definition: ForeignDecisionError,
  foreign_instance: ForeignDecisionError, revision_exhausted: RevisionError, disposed: DisposedError, busy: BusyError,
  record: RecordError, record_condition: ConditionRecordError, record_transition: TransitionRecordError,
  record_disposition: DispositionRecordError, unknown_behavior: UnknownBehaviorError,
  unknown_implementation: UnknownImplementationError, role_mismatch: RoleMismatchError });
const contextRoles = Object.freeze(['state', 'condition', 'transition', 'disposition', 'story', 'implementation']);
function translate(error, ids) {
  if (error.code !== 'GenericFailure' || typeof error.message !== 'string' || !error.message.startsWith('{')) throw error;
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
  for (const name of ['expected', 'actual']) {
    // BehaviorRole values are the kernel's wire names, so they map one to one.
    if (Object.hasOwn(context, `${name}_role`)) { context[name] = context[`${name}_role`]; delete context[`${name}_role`]; }
  }
  for (const name of ['expected_revision', 'current_revision']) {
    if (Object.hasOwn(context, name)) context[name === 'expected_revision' ? 'expectedRevision' : 'currentRevision'] = BigInt(context[name]);
    delete context[name];
  }
  throw new Type(info.message, context, { cause: error.cause });
}
module.exports = { StatusMachinaError, DefinitionError, DispatchError, CallbackError, SnapshotError,
  InvariantError, PostconditionError, AmbiguousTransitionError, ActionError, StaleDecisionError,
  ForeignDecisionError, RevisionError, DisposedError, BusyError, LimitError, InspectionError,
  RecordError, ConditionRecordError, TransitionRecordError, DispositionRecordError, BindingError,
  UnknownBehaviorError, UnknownImplementationError, RoleMismatchError, IdentityMapError, IdentityRoleError,
  IdentityMapLimitError, DuplicateIdentityError, DuplicateRepresentationError, UnknownIdentityError,
  UnknownRepresentationError, translate };
