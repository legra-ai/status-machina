/** Native identifiers compare with JavaScript Map semantics; no string codec is required. */
export type Identity = string | number | bigint | boolean | symbol | object | null | undefined;
export interface Event<K extends Identity, P = unknown> { readonly kind: K; readonly payload: P; }
export interface Limits { maxStates?: number; maxTransitions?: number; maxValues?: number; maxCallbacks?: number; }
export class BehaviorRef<S extends Identity = Identity, I extends Identity = Identity> {
  constructor(story: S, implementation: I);
  readonly story: S;
  readonly implementation: I;
}
export const BehaviorRole: Readonly<{ Action: 'action'; Condition: 'condition'; Invariant: 'invariant';
  Effect: 'effect'; Output: 'output'; Capability: 'capability'; Assertion: 'assertion' }>;
export type BehaviorRoleName = (typeof BehaviorRole)[keyof typeof BehaviorRole];
export interface BehaviorImplementation {
  readonly reference: BehaviorRef;
  readonly role: BehaviorRoleName;
  readonly callback: (...args: never[]) => unknown;
  readonly update?: boolean;
}
export class BehaviorRegistry {
  constructor(options?: { implementations?: Iterable<BehaviorImplementation>; maxBindings?: number });
  readonly implementations: readonly BehaviorImplementation[];
  readonly maxBindings: number;
}
export type Action<D, P = unknown, F = undefined> = { readonly id: Identity } & (
  { readonly apply: ((data: D, payload: P, facts: F) => void) | BehaviorRef; readonly update?: never } |
  { readonly update: (data: D, payload: P, facts: F) => D; readonly apply?: never });
export type Condition<D, P = unknown, F = undefined> = { readonly id: Identity } & (
  { readonly predicate: ((data: D, payload: P, facts: F) => boolean) | BehaviorRef; readonly allOf?: never; readonly anyOf?: never; readonly not?: never } |
  { readonly allOf: Iterable<Condition<D, P, F>>; readonly predicate?: never; readonly anyOf?: never; readonly not?: never } |
  { readonly anyOf: Iterable<Condition<D, P, F>>; readonly predicate?: never; readonly allOf?: never; readonly not?: never } |
  { readonly not: Condition<D, P, F>; readonly predicate?: never; readonly allOf?: never; readonly anyOf?: never });
export interface Effect<D, P = unknown, F = undefined, X = unknown> {
  readonly id: Identity;
  readonly describe: ((data: D, payload: P, facts: F) => X) | BehaviorRef;
}
export interface OutputEvent<D, P = unknown, F = undefined, X = unknown> {
  readonly kind: Identity;
  readonly describe: ((data: D, payload: P, facts: F) => X) | BehaviorRef;
}
export interface State<S extends Identity, D, P = unknown, F = undefined> {
  readonly id: S;
  readonly terminal?: boolean;
  readonly invariant?: ((data: D) => boolean) | BehaviorRef;
  readonly entryActions?: Iterable<Action<D, P, F>>;
  readonly exitActions?: Iterable<Action<D, P, F>>;
  readonly entryEffects?: Iterable<Effect<D, P, F>>;
  readonly exitEffects?: Iterable<Effect<D, P, F>>;
}
export interface Transition<S extends Identity, K extends Identity, D, P = unknown, F = undefined> {
  readonly id: Identity;
  readonly source: S;
  readonly event: K;
  readonly target: S;
  readonly internal?: boolean;
  readonly guard?: Condition<D, P, F>;
  readonly postconditions?: Iterable<Condition<D, P, F>>;
  readonly actions?: Iterable<Action<D, P, F>>;
  readonly effects?: Iterable<Effect<D, P, F>>;
  readonly outputs?: Iterable<OutputEvent<D, P, F>>;
}
export interface Disposition<S extends Identity, K extends Identity, D, P = unknown, F = undefined> {
  readonly id: Identity; readonly source: S; readonly event: K;
  readonly condition: Condition<D, P, F>; readonly reason: string;
}
export interface DefinitionOptions<S extends Identity, K extends Identity, D, P = unknown, F = undefined> {
  readonly id: Identity;
  readonly initial: S;
  readonly states: Iterable<State<S, D, P, F>>;
  readonly transitions: Iterable<Transition<S, K, D, P, F>>;
  readonly dispositions?: Iterable<Disposition<S, K, D, P, F>>;
  readonly behaviors?: BehaviorRegistry;
  readonly snapshot?: (data: D) => D;
  readonly inputSnapshot?: <T>(value: T) => T;
  readonly limits?: Limits;
}
export interface TransitionRecord<S extends Identity> {
  readonly transition: Identity; readonly source: S; readonly target: S;
  readonly previousRevision: bigint; readonly revision: bigint;
}
export interface EffectIntent { readonly id: Identity; readonly payload: unknown; }
export interface OutputIntent { readonly kind: Identity; readonly payload: unknown; }
export type Outcome<S extends Identity, K extends Identity> =
  { readonly kind: 'refused'; readonly state: S; readonly event: K } |
  { readonly kind: 'ignored'; readonly disposition: Identity; readonly state: S; readonly event: K; readonly reason: string } |
  { readonly kind: 'transitioned'; readonly record: TransitionRecord<S>;
    readonly effects: readonly EffectIntent[]; readonly outputs: readonly OutputIntent[] };
export interface DispatchOptions { readonly expectedRevision?: bigint; }
type FactsArguments<F> = undefined extends F ? [facts?: F, options?: DispatchOptions] : [facts: F, options?: DispatchOptions];
export class Definition<S extends Identity, K extends Identity, D = unknown, P = unknown, F = undefined> {
  constructor(specification: DefinitionOptions<S, K, D, P, F>);
  readonly id: Identity;
  readonly initial: S;
  readonly states: readonly State<S, D, P, F>[];
  readonly transitions: readonly Transition<S, K, D, P, F>[];
  instantiate(options: { id: Identity; data: D }): Machine<S, K, D, P, F>;
}
export class Machine<S extends Identity, K extends Identity, D, P = unknown, F = undefined> implements Disposable {
  private constructor();
  private readonly _brand;
  readonly id: Identity;
  readonly state: S;
  readonly revision: bigint;
  /** A detached native snapshot, retaining the application's class. */
  readonly data: D;
  dispatch(event: Event<K, P>, ...facts: FactsArguments<F>): Outcome<S, K>;
  evaluate(event: Event<K, P>, ...facts: FactsArguments<F>): Decision;
  accept(decision: Decision): Outcome<S, K>;
  dispose(): void;
  [Symbol.dispose](): void;
}
export class Decision implements Disposable {
  private constructor();
  private readonly _brand;
  readonly expectedRevision: bigint;
  dispose(): void;
  [Symbol.dispose](): void;
}
export class StatusMachinaError extends Error {
  constructor(message: string, context?: Readonly<Record<string, unknown>>, options?: ErrorOptions);
  readonly context: Readonly<Record<string, unknown>>;
  readonly phase?: string;
}
export class DefinitionError extends StatusMachinaError {}
export class DispatchError extends StatusMachinaError {}
export class CallbackError extends DispatchError {}
export class SnapshotError extends CallbackError {}
export class InvariantError extends DispatchError {}
export class PostconditionError extends DispatchError {}
export class AmbiguousTransitionError extends DispatchError {}
export class ActionError extends DispatchError {}
export class StaleDecisionError extends DispatchError {}
export class ForeignDecisionError extends DispatchError {}
export class RevisionError extends DispatchError {}
export class DisposedError extends StatusMachinaError {}
export class BusyError extends StatusMachinaError {}
export class LimitError extends StatusMachinaError {}
export class InspectionError extends StatusMachinaError {}
