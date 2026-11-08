"""Checked definitions, owned instances and single-use decisions."""
from __future__ import annotations
import json
from typing import Callable, Generic, Hashable, Iterable, TypeVar
from . import _native
from ._compile import Compiler
from ._context import Context, DEFAULT_SNAPSHOT, translate
from .errors import DefinitionError, DisposedError, LimitError
from .model import (BehaviorRegistry, Disposition, EffectIntent, Event, Ignored, Limits,
    OutputIntent, Refused, State, Transition, Transitioned, TransitionRecord)

StateT = TypeVar("StateT", bound=Hashable)
KindT = TypeVar("KindT", bound=Hashable)
DataT = TypeVar("DataT")

def bounded(values, limit, label):
    result = []
    for value in values:
        if len(result) == limit:
            raise LimitError(f"{label} exceeds its declared limit", limit=limit)
        result.append(value)
    return tuple(result)

class Definition(Generic[StateT, KindT]):
    """Immutable checked graph. Identifiers remain caller-defined native values."""
    def __init__(self, *, id: Hashable, initial: StateT,
            states: Iterable[State[StateT]], transitions: Iterable[Transition[StateT, KindT]],
            dispositions: Iterable[Disposition] = (), behaviors: BehaviorRegistry = BehaviorRegistry(),
            snapshot: Callable[[object], object] = DEFAULT_SNAPSHOT, limits: Limits = Limits()):
        self._id = id
        self._initial = initial
        self._limits = limits
        self._states = bounded(states, limits.max_states, "state inventory")
        self._transitions = bounded(transitions, limits.max_transitions, "transition inventory")
        self._dispositions = bounded(dispositions, 128, "disposition inventory")
        self._behaviors = behaviors
        self._snapshot = snapshot
        compiler = Compiler(limits)
        compiler.ids.intern("definition", id)
        source = {"initial": compiler.ids.intern("state", initial),
            "max_states": limits.max_states, "max_transitions": limits.max_transitions,
            "max_bindings": behaviors.max_bindings,
            "states": [compiler.state(state) for state in self._states],
            "transitions": [compiler.transition(edge) for edge in self._transitions],
            "dispositions": [compiler.disposition(rule) for rule in self._dispositions],
            "behaviors": compiler.behaviors(behaviors)}
        self._compiler = compiler
        self._callbacks = tuple(compiler.callbacks)
        try:
            self._native = _native._Definition(json.dumps(source, separators=(",", ":")))
        except _native.NativeFailure as error:
            translate(error, compiler.ids)
        if not callable(snapshot):
            raise DefinitionError("snapshot must be callable")

    @property
    def id(self) -> Hashable:
        return self._id

    @property
    def initial(self) -> StateT:
        return self._initial

    @property
    def states(self) -> tuple[State[StateT], ...]:
        return self._states

    @property
    def transitions(self) -> tuple[Transition[StateT, KindT], ...]:
        return self._transitions

    def instantiate(self, *, id: Hashable, data: DataT) -> Machine[StateT, KindT, DataT]:
        hash((type(id), id))
        context = Context(self, id)
        try:
            context.native = self._native.instantiate(context.allocate(data), self._limits.max_values, context.invoke)
            if context.arena_id != context.native.arena_id():
                raise RuntimeError("Native arena identity changed during admission")
        except _native.NativeFailure as error:
            context.values.clear()
            translate(error, self._compiler.ids)
        context.sweep()
        return Machine(context)

class Machine(Generic[StateT, KindT, DataT]):
    """An owned instance. Callbacks are synchronous; external effects stay caller-owned."""
    def __init__(self, context):
        self._context = context

    @property
    def id(self) -> Hashable:
        if self._context.closed:
            raise DisposedError("The machine has been disposed")
        return self._context.identity

    @property
    def state(self) -> StateT:
        with self._context.operation() as context:
            return context.ids.resolve("state", context.native.state())

    @property
    def revision(self) -> int:
        with self._context.operation() as context:
            return context.native.revision()

    @property
    def data(self) -> DataT:
        with self._context.operation() as context:
            return context.resolve(json.loads(context.native.snapshot(context.invoke)))

    def _outcome(self, encoded, event=None):
        value = json.loads(encoded)
        ids = self._context.ids
        if value["kind"] == "refused":
            return Refused(ids.resolve("state", value["state"]), event.kind)
        if value["kind"] == "ignored":
            return Ignored(ids.resolve("disposition", value["disposition"]),
                ids.resolve("state", value["state"]), event.kind, value["reason"])
        if value["kind"] != "transitioned":
            raise RuntimeError("Unrecognized native dispatch outcome")
        record = value["record"]
        return Transitioned(TransitionRecord(ids.resolve("transition", record["transition"]),
            ids.resolve("state", record["from_state"]), ids.resolve("state", record["to_state"]),
            int(record["previous_revision"]), int(record["revision"])),
            tuple(EffectIntent(ids.resolve("effect", item["id"]), self._context.resolve(item["payload"])) for item in value["effects"]),
            tuple(OutputIntent(ids.resolve("event", item["id"]), self._context.resolve(item["payload"])) for item in value["outputs"]))

    def dispatch(self, event: Event[KindT, object], *, facts: object = None, expected_revision: int | None = None):
        with self._context.operation() as context:
            encoded = context.native.dispatch(context.ids.find("event", event.kind),
                context.allocate(event.payload), context.allocate(facts), context.invoke, expected_revision)
            return self._outcome(encoded, event)

    def evaluate(self, event: Event[KindT, object], *, facts: object = None, expected_revision: int | None = None):
        with self._context.operation() as context:
            decision = context.native.evaluate(context.ids.find("event", event.kind),
                context.allocate(event.payload), context.allocate(facts), context.invoke, expected_revision)
            return Decision(context, decision, event.kind)

    def accept(self, decision):
        if decision._context is None:
            raise DisposedError("The decision has already been consumed")
        with self._context.operation() as context:
            source = decision._context
            decision._context = None
            try:
                return self._outcome(context.native.accept(decision._native), Event(decision._event_kind, None))
            finally:
                if source is not context:
                    source.sweep()

    def dispose(self) -> None:
        with self._context.operation(allow_closed=True) as context:
            if not context.closed:
                context.native.dispose()
                context.detach()

    close = dispose

    def __enter__(self):
        if self._context.closed:
            raise DisposedError("The machine has been disposed")
        return self

    def __exit__(self, *_):
        self.dispose()

class Decision:
    """An unaccepted candidate; acceptance consumes it exactly once."""
    def __init__(self, context, native, event_kind):
        self._context = context
        self._native = native
        self._event_kind = event_kind

    @property
    def expected_revision(self) -> int:
        if self._context is None:
            raise DisposedError("The decision has already been consumed")
        try:
            return self._native.expected_revision()
        except _native.NativeFailure as error:
            translate(error, self._context.ids)

    def dispose(self) -> None:
        if self._context is None:
            return
        context = self._context
        with context.operation(allow_closed=True):
            self._native.dispose()
            self._context = None

    close = dispose
    def __enter__(self):
        _ = self.expected_revision
        return self
    def __exit__(self, *_):
        self.dispose()
