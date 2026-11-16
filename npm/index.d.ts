/** Native identifiers compare with JavaScript Map semantics; no string codec is required. */
export type Identity = string | number | bigint | boolean | symbol | object | null | undefined;
export interface Event<K extends Identity, P = unknown> { readonly kind: K; readonly payload: P; }
/** Advisory policy observations, never authorization grants. */
export const CapabilityStatus: Readonly<{ Available: 'available'; Unavailable: 'unavailable'; Unknown: 'unknown' }>;
export type CapabilityStatusValue = (typeof CapabilityStatus)[keyof typeof CapabilityStatus];
export interface Capability<S extends Identity, D, F = undefined> {
  readonly id: Identity;
  readonly evaluate: ((state: S, data: D, facts: F) => CapabilityStatusValue) | BehaviorRef;
}
export interface CapabilityReport {
  readonly capability: Identity; readonly instance: Identity;
  readonly revision: bigint; readonly status: CapabilityStatusValue;
}
/** Bounded graph entries (1..1000000) and Mermaid output bytes (1..1048576). */
export interface InspectionLimits { readonly maxEntries?: number; readonly maxOutputBytes?: number; }
export interface DiagramLabels<S extends Identity, K extends Identity> {
  readonly states?: ReadonlyMap<S, string>;
  readonly transitions?: ReadonlyMap<Identity, string>;
  readonly events?: ReadonlyMap<K, string>;
  readonly conditions?: ReadonlyMap<Identity, string>;
  readonly dispositions?: ReadonlyMap<Identity, string>;
}
export interface StateCoverage<S extends Identity> {
  readonly id: S; readonly initial: boolean; readonly terminal: boolean;
  readonly incoming: number; readonly outgoing: number; readonly dispositions: number;
}
/** Declared structure in insertion order; not proof of execution coverage. */
export interface StructuralCoverage<S extends Identity> {
  readonly states: readonly StateCoverage<S>[];
  readonly transitions: readonly Identity[]; readonly dispositions: readonly Identity[];
}
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
  readonly capabilities?: Iterable<Capability<S, D, F>>;
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
  /** The portable record this definition was bound from, if any. */
  readonly record: DefinitionRecord<S, K> | undefined;
  readonly id: Identity;
  readonly initial: S;
  readonly states: readonly State<S, D, P, F>[];
  readonly transitions: readonly Transition<S, K, D, P, F>[];
  readonly capabilities: readonly Capability<S, D, F>[];
  structuralCoverage(limits?: InspectionLimits): StructuralCoverage<S>;
  diagram(labels: DiagramLabels<S, K>, limits?: InspectionLimits): string;
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
  evaluateCapability(capability: Identity, ...facts: undefined extends F ? [facts?: F] : [facts: F]): CapabilityReport;
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

/** A portable record failed checked binding; `context` names native identities. */
export class RecordError extends DefinitionError {}
/** `context.condition` is the invalid condition record's identity. */
export class ConditionRecordError extends RecordError {}
/** `context.transition` is the invalid transition record's identity. */
export class TransitionRecordError extends RecordError {}
/** `context.disposition` is the invalid disposition record's identity. */
export class DispositionRecordError extends RecordError {}
/** Exact Story resolution failed; no implementation was substituted or loaded. */
export class BindingError extends RecordError {}
/** No implementation of `context.story` is registered. */
export class UnknownBehaviorError extends BindingError {}
/** The Story is registered, but not `context.implementation`. */
export class UnknownImplementationError extends BindingError {}
/** The selected implementation has the `context.actual` role, not `context.expected`. */
export class RoleMismatchError extends BindingError {}
/** An identity codec is invalid or was asked for an unregistered value. */
export class IdentityMapError extends StatusMachinaError {}
/** A codec was supplied for, or requested from, the wrong identity role. */
export class IdentityRoleError extends IdentityMapError {}
/** The codec bound is invalid (1..4096) or the entries exceed it. */
export class IdentityMapLimitError extends IdentityMapError {}
/** One identity occurs twice; `context.entry` is the second position. */
export class DuplicateIdentityError extends IdentityMapError {}
/** Two identities share one external representation. */
export class DuplicateRepresentationError extends IdentityMapError {}
/** The identity has no registered representation; none is derived from `String()`. */
export class UnknownIdentityError extends IdentityMapError {}
/** The representation has no registered identity; no default is substituted. */
export class UnknownRepresentationError extends IdentityMapError {}

/** One graph use site (`id`) selecting an exact Story implementation. */
export class BehaviorUse<T extends Identity = Identity> {
  constructor(id: T, reference: BehaviorRef);
  readonly id: T;
  readonly reference: BehaviorRef;
}
/** A registered condition implementation; `use.id` is the condition identity. */
export class PredicateRecord {
  constructor(use: BehaviorUse);
  readonly kind: 'predicate';
  readonly use: BehaviorUse;
  readonly id: Identity;
}
/** Every operand must hold, in declared order (at most 16). */
export class AllRecord {
  constructor(id: Identity, operands: Iterable<ConditionRecord>);
  readonly kind: 'all';
  readonly id: Identity;
  readonly operands: readonly ConditionRecord[];
}
/** At least one operand must hold, in declared order (at most 16). */
export class AnyRecord {
  constructor(id: Identity, operands: Iterable<ConditionRecord>);
  readonly kind: 'any';
  readonly id: Identity;
  readonly operands: readonly ConditionRecord[];
}
/** Negate one expression. */
export class NotRecord {
  constructor(id: Identity, operand: ConditionRecord);
  readonly kind: 'not';
  readonly id: Identity;
  readonly operand: ConditionRecord;
}
/** A data-only condition expression of references, discriminated by `kind`. */
export type ConditionRecord = PredicateRecord | AllRecord | AnyRecord | NotRecord;
/** A state declaration; each behavior phase holds at most 16 uses. */
export class StateRecord<S extends Identity = Identity> {
  constructor(options: {
    readonly id: S; readonly terminal?: boolean; readonly invariant?: BehaviorRef;
    readonly entryActions?: Iterable<BehaviorUse>; readonly exitActions?: Iterable<BehaviorUse>;
    readonly entryEffects?: Iterable<BehaviorUse>; readonly exitEffects?: Iterable<BehaviorUse>;
  });
  readonly id: S;
  readonly terminal: boolean;
  readonly invariant: BehaviorRef | undefined;
  readonly entryActions: readonly BehaviorUse[];
  readonly exitActions: readonly BehaviorUse[];
  readonly entryEffects: readonly BehaviorUse[];
  readonly exitEffects: readonly BehaviorUse[];
}
/** A permitted edge; `outputs` use the emitted event kind as their identity. */
export class TransitionDefinitionRecord<S extends Identity = Identity, K extends Identity = Identity> {
  constructor(options: {
    readonly id: Identity; readonly source: S; readonly event: K; readonly target: S;
    readonly internal?: boolean; readonly guard?: ConditionRecord;
    readonly postconditions?: Iterable<ConditionRecord>; readonly actions?: Iterable<BehaviorUse>;
    readonly effects?: Iterable<BehaviorUse>; readonly outputs?: Iterable<BehaviorUse<K>>;
  });
  readonly id: Identity;
  readonly source: S;
  readonly event: K;
  readonly target: S;
  readonly internal: boolean;
  readonly guard: ConditionRecord | undefined;
  readonly postconditions: readonly ConditionRecord[];
  readonly actions: readonly BehaviorUse[];
  readonly effects: readonly BehaviorUse[];
  readonly outputs: readonly BehaviorUse<K>[];
}
/** An explicitly harmless event; `reason` is 1 to 256 UTF-8 bytes. */
export class DispositionRecord<S extends Identity = Identity, K extends Identity = Identity> {
  constructor(options: { readonly id: Identity; readonly source: S; readonly event: K;
    readonly condition: ConditionRecord; readonly reason: string });
  readonly id: Identity;
  readonly source: S;
  readonly event: K;
  readonly condition: ConditionRecord;
  readonly reason: string;
}
export interface RecordBindOptions<D> {
  readonly snapshot?: (data: D) => D;
  readonly inputSnapshot?: <T>(value: T) => T;
  /** Native value arena capacity per instance (1..65536, default 8192). */
  readonly maxValues?: number;
}
/** A data-only machine graph in declaration order, independent of any format. */
export class DefinitionRecord<S extends Identity = Identity, K extends Identity = Identity> {
  constructor(options: {
    readonly id: Identity; readonly initial: S;
    readonly states: Iterable<StateRecord<S>>;
    readonly transitions: Iterable<TransitionDefinitionRecord<S, K>>;
    readonly dispositions?: Iterable<DispositionRecord<S, K>>;
    readonly maxStates?: number; readonly maxTransitions?: number;
  });
  readonly id: Identity;
  readonly initial: S;
  readonly states: readonly StateRecord<S>[];
  readonly transitions: readonly TransitionDefinitionRecord<S, K>[];
  readonly dispositions: readonly DispositionRecord<S, K>[];
  readonly maxStates: number;
  readonly maxTransitions: number;
  /**
   * Resolve every use site in `behaviors` with the kernel's `DefinitionRecord::bind`.
   * No callback runs during binding, and nothing is loaded implicitly.
   */
  bind<D = unknown, P = unknown, F = undefined>(behaviors: BehaviorRegistry, options?: RecordBindOptions<D>): Definition<S, K, D, P, F>;
}

/** The identity roles of Rust `MachineIds`; `story` is `MachineIds::Behavior`. */
export const IdentityRole: Readonly<{
  Definition: 'definition'; Instance: 'instance'; State: 'state'; Event: 'event'; Transition: 'transition';
  Action: 'action'; Condition: 'condition'; Effect: 'effect'; Disposition: 'disposition'; Capability: 'capability';
  Operation: 'operation'; Scenario: 'scenario'; Assertion: 'assertion'; Scope: 'scope'; Story: 'story';
  Implementation: 'implementation' }>;
export type IdentityRoleName = (typeof IdentityRole)[keyof typeof IdentityRole];
/** Bounded one-to-one codec for one identity role, validated by the kernel `IdentityMap`. */
export class IdentityMap<R extends IdentityRoleName, T extends Identity, X extends Identity> implements Iterable<readonly [T, X]> {
  constructor(options: { readonly role: R; readonly entries: Iterable<readonly [T, X]>; readonly maxEntries?: number });
  private readonly _brand;
  readonly role: R;
  readonly maxEntries: number;
  readonly size: number;
  /** Registered pairs in declaration order. */
  [Symbol.iterator](): IterableIterator<readonly [T, X]>;
  /** The registered representation; never derived from `String()` or JSON. */
  encode(identity: T): X;
  /** The registered identity; unknown representations are errors. */
  decode(representation: X): T;
}
export type IdentityMapsByRole = { readonly [R in IdentityRoleName]?: IdentityMap<R, Identity, Identity> };
export type ConfiguredRole<M extends IdentityMapsByRole> = { [R in keyof M]-?: undefined extends M[R] ? never : R }[keyof M] & IdentityRoleName;
export type IdentityOf<C> = C extends IdentityMap<IdentityRoleName, infer T, Identity> ? T : never;
export type RepresentationOf<C> = C extends IdentityMap<IdentityRoleName, Identity, infer X> ? X : never;
/** At most one codec per identity role; each key accepts only its own role. */
export class MachineIdentityMaps<M extends IdentityMapsByRole> {
  constructor(maps: M);
  private readonly _brand;
  map<R extends ConfiguredRole<M>>(role: R): M[R];
  encode<R extends ConfiguredRole<M>>(role: R, identity: IdentityOf<M[R]>): RepresentationOf<M[R]>;
  decode<R extends ConfiguredRole<M>>(role: R, representation: RepresentationOf<M[R]>): IdentityOf<M[R]>;
}

/** Any owned machine, whatever its state, event, data, payload and facts types. */
export type AnyMachine = Machine<any, any, any, any, any>;
/** A caller-owned address: one instance identity within one routing scope. */
export class InstanceAddress<S extends Identity = Identity, I extends Identity = Identity> {
  constructor(scope: S, instance: I);
  readonly scope: S;
  readonly instance: I;
  /** Component-wise Map semantics; object tokens compare by identity. */
  equals(other: unknown): boolean;
}
/** Deliver `event` emitted at `source` to `destination` as `inputEvent`, optionally gated by a capability. */
export class Route<K extends Identity = Identity> {
  constructor(options: { readonly source: InstanceAddress; readonly event: Identity; readonly destination: InstanceAddress;
    readonly inputEvent: K; readonly capability?: Identity });
  readonly source: InstanceAddress;
  readonly event: Identity;
  readonly destination: InstanceAddress;
  readonly inputEvent: K;
  readonly capability: Identity | undefined;
}
/** Bounded `[address, machine]` registrations (1..512, default 128); the composition retains no machine. */
export class InstanceRegistry {
  constructor(options?: { readonly entries?: Iterable<readonly [InstanceAddress, AnyMachine]>; readonly maxInstances?: number });
  readonly entries: readonly (readonly [InstanceAddress, AnyMachine])[];
  readonly maxInstances: number;
}
/** An input addressed to exactly one registered instance. */
export class Delivery<K extends Identity = Identity, P = unknown> {
  constructor(destination: InstanceAddress, event: Event<K, P>);
  readonly destination: InstanceAddress;
  readonly event: Event<K, P>;
}
/** Explicit work budgets for one propagate call, each within 1..4096. */
export class PropagationLimits {
  constructor(options: { readonly maxSteps: number; readonly maxIntents: number });
  readonly maxSteps: number;
  readonly maxIntents: number;
}
export const PropagationStatus: Readonly<{ Quiescent: 'quiescent'; StepLimit: 'step_limit'; IntentLimit: 'intent_limit';
  UnboundDestination: 'unbound_destination'; DispatchFailed: 'dispatch_failed' }>;
export type PropagationStatusValue = (typeof PropagationStatus)[keyof typeof PropagationStatus];
/** One routing result, discriminated by `kind`; nothing has been dispatched. */
export type RoutedItem =
  { readonly kind: 'routed'; readonly source: InstanceAddress; readonly destination: InstanceAddress; readonly event: Event<Identity> } |
  { readonly kind: 'unmatched'; readonly source: InstanceAddress; readonly intent: OutputIntent } |
  { readonly kind: 'deferred'; readonly source: InstanceAddress; readonly destination: InstanceAddress;
    readonly capability: Identity; readonly status: CapabilityStatusValue; readonly intent: OutputIntent };
export type UnroutedItem = Exclude<RoutedItem, { readonly kind: 'routed' }>;
/** One completed dispatch; its effects and outputs are reported separately. */
export interface PropagationStep { readonly destination: InstanceAddress; readonly outcome: Outcome<Identity, Identity>; }
/** Accepted external work with the instance that requested it; nothing ran. */
export interface AddressedEffect { readonly source: InstanceAddress; readonly intent: EffectIntent; }
interface PropagationProgress {
  readonly steps: readonly PropagationStep[];
  readonly effects: readonly AddressedEffect[];
  readonly unrouted: readonly UnroutedItem[];
  /** The unaccepted input first, then remaining work; caller inputs are the original Delivery objects. */
  readonly pending: readonly Delivery[];
}
/** Ordered partial progress: accepted steps are never rolled back by a later failure or budget stop. */
export type PropagationReport = PropagationProgress & (
  { readonly status: Exclude<PropagationStatusValue, 'dispatch_failed'>; readonly error?: undefined } |
  { readonly status: 'dispatch_failed'; readonly error: DispatchError });
/** Caller-observed revisions and capability reports, lowered by one composition. */
export class CapabilitySnapshot {
  private constructor();
  private readonly _brand;
  readonly revisions: readonly (readonly [InstanceAddress, bigint])[];
  readonly reports: readonly (readonly [InstanceAddress, CapabilityReport])[];
  readonly maxObservations: number;
}
export interface CompositionOptions {
  readonly sources: InstanceRegistry;
  readonly destinations: InstanceRegistry;
  readonly routes?: Iterable<Route>;
  /** 1..512, default 128. */
  readonly maxRoutes?: number;
  /** 1..256, default 128. */
  readonly maxBatch?: number;
}
export interface CapabilitySnapshotOptions {
  readonly revisions?: Iterable<readonly [InstanceAddress, bigint]>;
  /** Reports from `Machine.evaluateCapability`; never rebuilt from their fields. */
  readonly reports?: Iterable<readonly [InstanceAddress, CapabilityReport]>;
  /** 1..1024, default 256. */
  readonly maxObservations?: number;
}
/** Immutable, bounded routing between registered machine instances; it never dispatches by itself. */
export class Composition {
  constructor(options: CompositionOptions);
  private readonly _brand;
  readonly routes: readonly Route[];
  readonly maxRoutes: number;
  readonly maxBatch: number;
  capabilities(options?: CapabilitySnapshotOptions): CapabilitySnapshot;
  routeBatch(source: InstanceAddress, outputs: Iterable<OutputIntent>, options?: { readonly capabilities?: CapabilitySnapshot }): readonly RoutedItem[];
  /** Take exclusive use of every registered machine until the binding is disposed. */
  bind(instances: ReadonlyMap<InstanceAddress, AnyMachine> | Iterable<readonly [InstanceAddress, AnyMachine]>): BoundComposition;
}
export interface PropagateOptions {
  readonly limits: PropagationLimits;
  readonly facts?: unknown;
  readonly capabilities?: CapabilitySnapshot;
}
/** Exclusive use of bound machines, like Rust `&mut` borrows; bound machines throw BusyError. */
export class BoundComposition implements Disposable {
  private constructor();
  private readonly _brand;
  propagate(inputs: Iterable<Delivery>, options: PropagateOptions): PropagationReport;
  dispose(): void;
  [Symbol.dispose](): void;
}
/** A composition was rejected by the kernel; `context.reason` is the kernel variant. */
export class CompositionError extends StatusMachinaError {}
/** `context` names the `registry`, `entry`, `address` and `machine`. */
export class InstanceRegistryError extends CompositionError {}
export class InstanceRegistryLimitError extends InstanceRegistryError {}
export class AddressMismatchError extends InstanceRegistryError {}
export class DuplicateInstanceError extends InstanceRegistryError {}
/** `context.route` is the native Route and `context.index` its position. */
export class CompositionDefinitionError extends CompositionError {}
export class CompositionLimitError extends CompositionDefinitionError {}
export class DuplicateRouteError extends CompositionDefinitionError {}
export class UnregisteredRouteAddressError extends CompositionDefinitionError {}
export class UndeclaredRouteEventError extends CompositionDefinitionError {}
export class UndeclaredCapabilityError extends CompositionDefinitionError {}
/** The batch was refused before routing; `outputs` holds every intent. */
export class RouteBatchError extends CompositionError {
  readonly outputs: readonly OutputIntent[];
}
export class UnregisteredSourceError extends RouteBatchError {}
export class BatchLimitError extends RouteBatchError {}
/** Exclusive binding was refused before any machine was dispatched. */
export class CompositionBindingError extends CompositionError {}
export class DuplicateBindingError extends CompositionBindingError {}
export class UnregisteredBindingError extends CompositionBindingError {}
export class BindingMismatchError extends CompositionBindingError {}
/** `context.address` is the first registered address left unbound. */
export class MissingBindingError extends CompositionBindingError {}
export class AliasedBindingError extends CompositionBindingError {}
export class PropagationLimitsError extends CompositionError {}
/** `context` names the `list`, `entry` and `address`. */
export class CapabilitySnapshotError extends CompositionError {}
export class CapabilitySnapshotLimitError extends CapabilitySnapshotError {}
export class ReportInstanceMismatchError extends CapabilitySnapshotError {}
export class DuplicateObservationError extends CapabilitySnapshotError {}
export class ForeignSnapshotError extends CapabilitySnapshotError {}

/** A non-negative span of time with exact BigInt nanoseconds, over the range of Rust's `Duration`. */
export class Duration {
  constructor(nanoseconds: bigint);
  static readonly ZERO: Duration;
  static readonly MAX: Duration;
  static from(parts: { readonly days?: number | bigint; readonly hours?: number | bigint; readonly minutes?: number | bigint;
    readonly seconds?: number | bigint; readonly milliseconds?: number | bigint; readonly microseconds?: number | bigint;
    readonly nanoseconds?: number | bigint }): Duration;
  static ofDays(value: number | bigint): Duration;
  static ofHours(value: number | bigint): Duration;
  static ofMinutes(value: number | bigint): Duration;
  static ofSeconds(value: number | bigint): Duration;
  static ofMilliseconds(value: number | bigint): Duration;
  static ofNanoseconds(value: number | bigint): Duration;
  readonly nanoseconds: bigint;
  /** Whole seconds. */
  readonly seconds: bigint;
  readonly subsecondNanoseconds: number;
  equals(other: unknown): boolean;
  compare(other: Duration): -1 | 0 | 1;
  plus(other: Duration): Duration;
  /** `instant + this`, rounded up to Date's millisecond so a deadline is never early. */
  after(instant: Date): Date;
  /** ISO 8601, e.g. `PT21600S`. */
  toString(): string;
  toJSON(): string;
  private readonly brand: 'Duration';
}
export class FixedDelay {
  constructor(delay: Duration);
  readonly kind: 'fixed';
  readonly delay: Duration;
}
export class ExponentialBackoff {
  constructor(options: { readonly initial: Duration; readonly maximum: Duration });
  readonly kind: 'exponential';
  readonly initial: Duration;
  readonly maximum: Duration;
}
export class RetryAt {
  private constructor();
  readonly kind: 'at';
  readonly delay: Duration;
}
export class RetryExhausted {
  private constructor();
  readonly kind: 'exhausted';
}
export type RetrySchedule = RetryAt | RetryExhausted;
export interface RetryPolicyOptions {
  readonly delay: FixedDelay | ExponentialBackoff;
  /** Every attempt, the first included (2..2**32-1). */
  readonly maxAttempts: number;
  /** A bound on time since the first attempt, or `null` for an explicitly indefinite window. */
  readonly maxElapsed: Duration | null;
}
/** A bounded fixed or capped exponential policy, validated by the kernel. Reads no clock. */
export class RetryPolicy {
  constructor(options: RetryPolicyOptions);
  readonly delay: FixedDelay | ExponentialBackoff;
  readonly maxAttempts: number;
  readonly maxElapsed: Duration | null;
  schedule(failedAttempts: number, elapsed: Duration): RetrySchedule;
}
/** The identity a reminder carries back; matching is the kernel's exact instance, operation and generation. */
export class WaitCorrelation<I extends Identity = Identity, O extends Identity = Identity> {
  constructor(value: { readonly instance: I; readonly operation: O; readonly generation: bigint });
  readonly instance: I;
  readonly operation: O;
  readonly generation: bigint;
  equals(other: unknown): boolean;
}
export interface WakeIntent<I extends Identity = Identity, O extends Identity = Identity, P = unknown> {
  readonly correlation: WaitCorrelation<I, O>;
  readonly deadline: Date;
  readonly payload: P;
}
/** Bounded active waits of one instance incarnation, held in its machine data; schedules nothing. */
export class RetryWaits<I extends Identity = Identity, O extends Identity = Identity, P = unknown> {
  constructor(options: { readonly instance: I; readonly capacity: number });
  readonly instance: I;
  readonly capacity: number;
  schedule(operation: O, deadline: Date, payload: P): WaitCorrelation<I, O>;
  pending(): readonly WakeIntent<I, O, P>[];
  get(operation: O): WakeIntent<I, O, P> | undefined;
  markRegistered(correlation: WaitCorrelation<Identity, Identity>): boolean;
  isDue(correlation: WaitCorrelation<Identity, Identity>, now: Date): boolean;
  consume(correlation: WaitCorrelation<Identity, Identity>, now: Date): WakeIntent<I, O, P> | undefined;
  cancel(operation: O): WakeIntent<I, O, P> | undefined;
  /** An independent copy for a custom snapshot function. */
  clone(copyPayload: (payload: P) => P): RetryWaits<I, O, P>;
}

export class RetryError extends StatusMachinaError {}
export class RetryPolicyError extends RetryError {}
export class ZeroDelayError extends RetryPolicyError {}
export class TooFewAttemptsError extends RetryPolicyError {}
export class CapBelowInitialError extends RetryPolicyError {}
export class RetryWaitError extends RetryError {}
export class InvalidWaitCapacityError extends RetryWaitError {}
export class WaitCapacityReachedError extends RetryWaitError {}
export class WaitGenerationExhaustedError extends RetryWaitError {}
/** A value lies outside its kernel type; `context.field` names it and `context.value` holds it. */
export class RetryRangeError extends RetryError {}

/** Positive replay budgets validated by the kernel; omitted fields take the kernel defaults (64, 256, 4096, 4096, 4096). */
export class ScenarioLimits {
  constructor(options?: { readonly maxScenarios?: number; readonly maxStepsPerScenario?: number; readonly maxNodes?: number;
    readonly maxOutputs?: number; readonly maxTraceEntries?: number });
  readonly maxScenarios: number;
  readonly maxStepsPerScenario: number;
  readonly maxNodes: number;
  readonly maxOutputs: number;
  readonly maxTraceEntries: number;
}
/** One dispatch: the event and the facts it is evaluated against. */
export interface ScenarioStep<K extends Identity, P = unknown, F = unknown> {
  readonly event: Event<K, P>;
  readonly facts?: F;
}
/** An identified, ordered, finite list of steps; read lazily, at most 4097 steps. */
export interface Scenario<C extends Identity, K extends Identity, P = unknown, F = unknown> {
  readonly id: C;
  readonly steps: Iterable<ScenarioStep<K, P, F>>;
}
/** A non-empty bounded set of scenarios, checked by the kernel's ScenarioSet builder. */
export class ScenarioSet<C extends Identity, K extends Identity> {
  constructor(options: { readonly scenarios: Iterable<Scenario<C, K>>; readonly limits?: ScenarioLimits });
  private readonly _brand;
  readonly scenarios: readonly { readonly id: C; readonly steps: readonly ScenarioStep<K>[] }[];
  readonly limits: ScenarioLimits;
}
/** The compact outcome of one replayed step, discriminated by `kind`. */
export type ScenarioStepKind =
  { readonly kind: 'transitioned'; readonly transition: Identity } |
  { readonly kind: 'ignored'; readonly disposition: Identity } |
  { readonly kind: 'refused' } |
  { readonly kind: 'dispatch_failed' };
/** Metadata of one replayed step; Rust `kind()` is `outcome`. Never payloads or machine data. */
export interface ScenarioObservation<C extends Identity, S extends Identity, K extends Identity> {
  readonly scenario: C;
  readonly step: number;
  readonly inputEvent: K;
  readonly before: S;
  readonly after: S;
  readonly outcome: ScenarioStepKind;
  readonly outputEvents: readonly Identity[];
  readonly effects: readonly Identity[];
}
/** One independently named safety predicate that returns true while the property holds. */
export class ScenarioAssertion<A extends Identity = Identity, C extends Identity = Identity, S extends Identity = Identity, K extends Identity = Identity> {
  constructor(options: { readonly id: A; readonly evaluate: ((observation: ScenarioObservation<C, S, K>) => boolean) | BehaviorRef });
  readonly id: A;
  readonly evaluate: ((observation: ScenarioObservation<C, S, K>) => boolean) | BehaviorRef;
}
export const ScenarioFailure: Readonly<{ EvaluationFailed: 'evaluation_failed'; InvalidInitialData: 'invalid_initial_data';
  Ambiguous: 'ambiguous'; InvariantFailed: 'invariant_failed'; PostconditionFailed: 'postcondition_failed';
  ActionFailed: 'action_failed'; RevisionExhausted: 'revision_exhausted'; DefinitionMismatch: 'definition_mismatch';
  InstanceMismatch: 'instance_mismatch'; StaleRevision: 'stale_revision' }>;
export type ScenarioFailureValue = (typeof ScenarioFailure)[keyof typeof ScenarioFailure];
export const ScenarioLimit: Readonly<{ Outputs: 'outputs'; TraceEntries: 'trace_entries' }>;
export type ScenarioLimitValue = (typeof ScenarioLimit)[keyof typeof ScenarioLimit];
/** The step prefix, through the failing step, that falsified `assertion`. */
export interface CounterexampleTrace<C extends Identity, S extends Identity, K extends Identity> {
  readonly scenario: C;
  readonly assertion: Identity;
  readonly steps: readonly ScenarioObservation<C, S, K>[];
}
/** The bounded replay outcome, discriminated by `kind`. */
export type ScenarioResult<C extends Identity, S extends Identity, K extends Identity> =
  /** Every supplied step was checked and passed: coverage of exactly these scenarios and nodes, not a proof. */
  { readonly kind: 'complete_within_scenarios'; readonly scenarios: number; readonly nodes: number } |
  { readonly kind: 'counterexample'; readonly trace: CounterexampleTrace<C, S, K> } |
  { readonly kind: 'limit_reached'; readonly scenario: C; readonly step: number; readonly limit: ScenarioLimitValue } |
  /** Step 0 with no steps: the initial data failed, or raised in, the initial state's invariant. */
  { readonly kind: 'execution_failed'; readonly scenario: C; readonly step: number; readonly failure: ScenarioFailureValue;
    readonly steps: readonly ScenarioObservation<C, S, K>[] };
/** A result whose name states its bound: only the supplied scenarios were checked. */
export interface BoundedScenarioReport<C extends Identity, S extends Identity, K extends Identity> {
  readonly result: ScenarioResult<C, S, K>;
}
export interface ScenarioReplayOptions<C extends Identity, S extends Identity, K extends Identity, D> {
  readonly id: Identity;
  readonly data: D;
  readonly scenarios: ScenarioSet<C, K>;
  readonly assertions?: Iterable<ScenarioAssertion<Identity, C, S, K>>;
}
/** Caller-driven replay of explicit finite scenarios on fresh ephemeral instances. */
export const ScenarioReplay: Readonly<{
  run<S extends Identity, K extends Identity, D, P, F, C extends Identity>(definition: Definition<S, K, D, P, F>,
    options: ScenarioReplayOptions<C, S, K, D>): BoundedScenarioReport<C, S, K>;
}>;
/** Scenario replay was rejected by the kernel; `context.reason` is the kernel variant. */
export class ScenarioError extends StatusMachinaError {}
export class ScenarioLimitsError extends ScenarioError {}
/** `context.entry` is the refused scenario's position and `context.scenario` its identity. */
export class ScenarioSetError extends ScenarioError {}
export class EmptyScenarioSetError extends ScenarioSetError {}
export class ScenarioCountLimitError extends ScenarioSetError {}
export class ScenarioStepLimitError extends ScenarioSetError {}
export class ScenarioNodeLimitError extends ScenarioSetError {}
export class ScenarioAssertionError extends ScenarioError {}
export class MissingAssertionPredicateError extends ScenarioAssertionError {}
export class ScenarioReplayError extends ScenarioError {}
/** More safety assertions than the kernel's ceiling of 128. */
export class AssertionLimitError extends ScenarioReplayError {}
