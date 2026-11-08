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
const types = Object.freeze({ definition: DefinitionError, callback: CallbackError,
  invariant: InvariantError, postcondition: PostconditionError, ambiguous: AmbiguousTransitionError,
  action: ActionError, stale_revision: StaleDecisionError, foreign_definition: ForeignDecisionError,
  foreign_instance: ForeignDecisionError, revision_exhausted: RevisionError, disposed: DisposedError, busy: BusyError });
function translate(error, ids) {
  if (error.code !== 'GenericFailure' || typeof error.message !== 'string' || !error.message.startsWith('{')) throw error;
  const info = JSON.parse(error.message);
  const Type = info.code === 'callback' && info.phase === 'snapshot' ? SnapshotError : types[info.code];
  if (Type === undefined) throw new Error('Unrecognized native failure category', { cause: error });
  const context = {};
  for (const [name, value] of Object.entries(info)) {
    if (name !== 'code' && name !== 'message' && value !== null) context[name] = value;
  }
  for (const role of ['state', 'condition']) {
    if (Object.hasOwn(context, role) && ids !== undefined) context[role] = ids.resolve(role, context[role]);
  }
  for (const name of ['expected_revision', 'current_revision']) {
    if (Object.hasOwn(context, name)) context[name] = BigInt(context[name]);
  }
  throw new Type(info.message, context, { cause: error.cause });
}
module.exports = { StatusMachinaError, DefinitionError, DispatchError, CallbackError, SnapshotError,
  InvariantError, PostconditionError, AmbiguousTransitionError, ActionError, StaleDecisionError,
  ForeignDecisionError, RevisionError, DisposedError, BusyError, LimitError, InspectionError, translate };
