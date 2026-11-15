"""Explicit, bounded composition of native machines through the Rust kernel.

The kernel's ``InstanceRegistry``, ``Composition``, ``CapabilitySnapshot`` and
``BoundComposition`` decide every registration, routing, binding and propagation
rule. This module only lowers native values to private indexes and resolves the
kernel's results back to the original native values.
"""
from __future__ import annotations
from contextlib import ExitStack
from itertools import islice
import json
import threading
from typing import Any, Hashable, Iterable, Mapping
from . import _native
from . import errors
from ._compile import UNKNOWN_EVENT, Identities
from ._context import Context, exception
from .machine import Machine, decode_outcome
from .model import CapabilityReport, CapabilityStatus, EffectIntent, Event, OutputIntent
from .routing import (
    HARD_MAX_INSTANCES, HARD_MAX_ROUTES, AddressedEffect, Deferred, Delivery, InstanceAddress,
    InstanceRegistry, PropagationLimits, PropagationReport, PropagationStatus, PropagationStep,
    Route, Routed, RoutedBatch, Unmatched, read, representable, same,
)

_HARD_MAX_BATCH = 256
_HARD_MAX_OBSERVATIONS = 1024
_SCOPE_SPACE = 2**32 - 1

def _encode(value: object) -> str:
    return json.dumps(value, separators=(",", ":"))

class _Table:
    """The composition's native addresses; index ``i`` is the kernel's address ``i``."""
    def __init__(self) -> None:
        self.scopes: dict[tuple[type, Hashable], int] = {}
        self.addresses: list[InstanceAddress] = []
        self.indexes: dict[InstanceAddress, int] = {}
        self.ids: list[Identities | None] = []
        self.wire: list[dict] = []

    def scope(self, value: Hashable) -> int:
        return self.scopes.setdefault((type(value), value), len(self.scopes))

    def add(self, address: InstanceAddress, machine: int | None = None, ids: Identities | None = None) -> int:
        if address in self.indexes:
            return self.indexes[address]
        index = len(self.addresses)
        self.addresses.append(address)
        self.indexes[address] = index
        self.ids.append(ids)
        self.wire.append({"scope": self.scope(address.scope), "machine": machine})
        return index

class _Call:
    """Call-local addresses outside the table, each with its own unknown scope."""
    def __init__(self, table: _Table) -> None:
        self.table = table
        self.extra: list[dict] = []
        self.addresses: list[InstanceAddress] = []
        self.indexes: dict[InstanceAddress, int] = {}

    def address(self, address: InstanceAddress, report: int | None = None) -> int:
        if not isinstance(address, InstanceAddress):
            raise TypeError("Composition addresses must be InstanceAddress values")
        if address in self.table.indexes:
            return self.table.indexes[address]
        if address not in self.indexes:
            self.indexes[address] = len(self.table.addresses) + len(self.extra)
            self.extra.append({"scope": _SCOPE_SPACE - len(self.extra), "report": report})
            self.addresses.append(address)
        return self.indexes[address]

    def resolve(self, index: int) -> InstanceAddress:
        table = self.table.addresses
        return table[index] if index < len(table) else self.addresses[index - len(table)]

    def ids(self, index: int) -> Identities | None:
        return self.table.ids[index] if index < len(self.table.ids) else None

def _find(ids: Identities | None, role: str, value: Hashable) -> int:
    return UNKNOWN_EVENT if ids is None else ids.find(role, value)

def _require(ids: Identities | None) -> Identities:
    if ids is None:
        raise RuntimeError("The kernel returned an unregistered address")
    return ids

_KINDS: dict[tuple[str, str], type[errors.CompositionError]] = {
    **{("registry", reason): errors.InstanceRegistryLimitError for reason in ("invalid_limit",
        "limit_below_current_size", "limit_exceeded", "instance_event_limit_exceeded",
        "aggregate_event_limit_exceeded")},
    ("registry", "address_mismatch"): errors.AddressMismatchError,
    ("registry", "duplicate_instance"): errors.DuplicateInstanceError,
    **{("definition", reason): errors.CompositionLimitError for reason in ("invalid_limits",
        "limit_below_current_size", "route_limit_exceeded")},
    ("definition", "duplicate_route"): errors.DuplicateRouteError,
    ("definition", "unregistered_source"): errors.UnregisteredRouteAddressError,
    ("definition", "unregistered_destination"): errors.UnregisteredRouteAddressError,
    ("definition", "undeclared_source_event"): errors.UndeclaredRouteEventError,
    ("definition", "undeclared_destination_event"): errors.UndeclaredRouteEventError,
    ("batch", "unregistered_source"): errors.UnregisteredSourceError,
    ("batch", "batch_limit_exceeded"): errors.BatchLimitError,
    ("binding", "duplicate_instance"): errors.DuplicateBindingError,
    ("binding", "unregistered_instance"): errors.UnregisteredBindingError,
    ("binding", "identity_or_inventory_mismatch"): errors.BindingMismatchError,
    ("binding", "signature_mismatch"): errors.BindingMismatchError,
    ("binding", "missing_instance"): errors.MissingBindingError,
    ("binding", "aliased_instance"): errors.AliasedBindingError,
    ("limits", "invalid_limits"): errors.PropagationLimitsError,
    **{("snapshot", reason): errors.CapabilitySnapshotLimitError for reason in ("invalid_limit",
        "limit_below_current_size", "limit_exceeded")},
    ("snapshot", "instance_mismatch"): errors.ReportInstanceMismatchError,
    ("snapshot", "duplicate_revision"): errors.DuplicateObservationError,
    ("snapshot", "duplicate_capability"): errors.DuplicateObservationError,
    ("snapshot", "foreign_snapshot"): errors.ForeignSnapshotError,
}
_FAMILIES: dict[str, type[errors.CompositionError]] = {
    "registry": errors.InstanceRegistryError, "definition": errors.CompositionDefinitionError,
    "route": errors.CompositionDefinitionError, "batch": errors.RouteBatchError,
    "binding": errors.CompositionBindingError, "limits": errors.PropagationLimitsError,
    "snapshot": errors.CapabilitySnapshotError,
}

def _failure(error: _native.NativeFailure, call: _Call, *, routes: tuple[Route, ...] = (),
        registries: Mapping[str, InstanceRegistry] | None = None, **extra: object) -> errors.StatusMachinaError:
    """The typed exception for a kernel composition rejection, with native context."""
    info = json.loads(str(error))
    if info["code"] != "composition":
        return exception(error)
    context: dict[str, object] = {"reason": info["reason"], **extra}
    for name in ("registry", "list", "entry", "limit"):
        if info[name] is not None:
            context[name] = info[name]
    if info["address"] is not None:
        context["address"] = call.resolve(info["address"])
    if info["route"] is not None and routes:
        context["route"] = routes[info["route"]]
        context["index"] = info["route"]
    if registries is not None and info["registry"] is not None and info["entry"] is not None:
        context["machine"] = registries[info["registry"]].entries[info["entry"]][1]
    kind = _KINDS.get((info["family"], info["reason"]), _FAMILIES[info["family"]])
    return kind(info["message"], **context)

class CapabilitySnapshot:
    """Caller-observed revisions and capability reports, lowered by one composition.

    Missing or stale observations make a capability ``UNKNOWN``; reports are advisory.
    """
    def __init__(self, composition: Composition, native: _native._CapabilitySnapshot,
            revisions: tuple[tuple[InstanceAddress, int], ...],
            reports: tuple[tuple[InstanceAddress, CapabilityReport], ...], max_observations: int) -> None:
        self._composition = composition
        self._native = native
        self.revisions = revisions
        self.reports = reports
        self.max_observations = max_observations

class Composition:
    """Immutable, bounded routing between registered machine instances.

    It never owns or dispatches a machine. ``route_batch`` routes outputs without
    dispatch; ``bind`` takes exclusive use of the actual machines for ``propagate``.
    """
    def __init__(self, *, sources: InstanceRegistry, destinations: InstanceRegistry,
            routes: Iterable[Route] = (), max_routes: int = 128, max_batch: int = 128) -> None:
        if not isinstance(sources, InstanceRegistry) or not isinstance(destinations, InstanceRegistry):
            raise TypeError("sources and destinations must be InstanceRegistry values")
        for name, value in (("max_routes", max_routes), ("max_batch", max_batch)):
            if type(value) is not int:
                raise TypeError(f"{name} must be an integer")
        accepted = read(routes, max_routes, HARD_MAX_ROUTES)
        if not all(isinstance(route, Route) for route in accepted):
            raise TypeError("routes must be Route values")
        table = _Table()
        machines: list[Machine[Any, Any, Any]] = []
        for registry in (sources, destinations):
            for address, machine in registry.entries:
                if not any(machine is known for known in machines):
                    machines.append(machine)
        for registry in (sources, destinations):
            for address, machine in registry.entries:
                index = next(i for i, known in enumerate(machines) if known is machine)
                if same(address.instance, machine.id):
                    table.add(address, index, machine._context.ids)
                else:
                    table.add(address)
        lowered = []
        for route in accepted:
            source, destination = table.add(route.source), table.add(route.destination)
            target = table.ids[destination]
            lowered.append({"source": source, "event": _find(table.ids[source], "event", route.event),
                "destination": destination, "input_event": _find(target, "event", route.input_event),
                "capability": None if route.capability is None else _find(target, "capability", route.capability)})
        registries = {"sources": sources, "destinations": destinations}
        spec = {"addresses": table.wire, "routes": lowered, "max_routes": representable(max_routes) or 0,
            "max_batch": representable(max_batch) or 0,
            **{name: {"max_instances": representable(value.max_instances) or 0,
                "entries": [{"address": table.indexes[address],
                    "machine": next(i for i, known in enumerate(machines) if known is machine)}
                    for address, machine in value.entries]} for name, value in registries.items()}}
        try:
            self._native = _native._Composition(_encode(spec), [machine._context_native() for machine in machines])
        except _native.NativeFailure as error:
            raise _failure(error, _Call(table), routes=accepted, registries=registries) from None
        for index, (route, wire) in enumerate(zip(accepted, lowered)):
            if wire["capability"] == UNKNOWN_EVENT:
                raise errors.UndeclaredCapabilityError(
                    "The route's capability is not declared by the destination definition",
                    reason="undeclared_capability", route=route, index=index, capability=route.capability)
        self._table = table
        self.routes: tuple[Route, ...] = accepted
        self.max_routes = max_routes
        self.max_batch = max_batch

    def capabilities(self, *, revisions: Iterable[tuple[InstanceAddress, int]] = (),
            reports: Iterable[tuple[InstanceAddress, CapabilityReport]] = (),
            max_observations: int = 256) -> CapabilitySnapshot:
        """Observe current revisions, then reports, for this composition's addresses.

        Each report must come from ``Machine.evaluate_capability``; reports are never
        rebuilt from their fields.
        """
        if type(max_observations) is not int:
            raise TypeError("max_observations must be an integer")
        observed = read(revisions, max_observations, _HARD_MAX_OBSERVATIONS)
        reported = read(reports, max_observations, _HARD_MAX_OBSERVATIONS)
        for address, revision in observed:
            if type(revision) is not int or not 0 <= revision < 2**64:
                raise TypeError("Revisions must be unsigned 64-bit integers")
        natives = []
        for address, report in reported:
            native = getattr(report, "_native", None) if isinstance(report, CapabilityReport) else None
            if native is None:
                raise TypeError("Capability reports must come from Machine.evaluate_capability")
            natives.append(native)
        call = _Call(self._table)
        report_wire = [{"address": call.address(address, index if same(address.instance, report.instance) else None),
            "report": index} for index, (address, report) in enumerate(reported)]
        spec = {"max_observations": representable(max_observations) or 0,
            "revisions": [{"address": call.address(address), "revision": str(revision)} for address, revision in observed],
            "reports": report_wire, "extra": call.extra}
        try:
            native_snapshot = self._native.snapshot(_encode(spec), natives)
        except _native.NativeFailure as error:
            raise _failure(error, call) from None
        return CapabilitySnapshot(self, native_snapshot, observed, reported, max_observations)

    def _snapshot(self, capabilities: CapabilitySnapshot | None) -> _native._CapabilitySnapshot | None:
        if capabilities is None:
            return None
        if not isinstance(capabilities, CapabilitySnapshot):
            raise TypeError("capabilities must be a CapabilitySnapshot")
        return capabilities._native

    def route_batch(self, source: InstanceAddress, outputs: Iterable[OutputIntent], *,
            capabilities: CapabilitySnapshot | None = None) -> RoutedBatch:
        """Route one source's outputs in order, without dispatching any destination."""
        call = _Call(self._table)
        index = call.address(source)
        batch = read(outputs, self.max_batch, _HARD_MAX_BATCH)
        if not all(isinstance(output, OutputIntent) for output in batch):
            raise TypeError("outputs must be OutputIntent values")
        ids = call.ids(index)
        spec = {"source": index, "outputs": [{"event": _find(ids, "event", output.kind), "payload": position}
            for position, output in enumerate(batch)], "extra": call.extra}
        arena = _native._Arena(max(1, len(batch)))
        try:
            raw = json.loads(self._native.route_batch(arena, _encode(spec), self._snapshot(capabilities)))
        except _native.NativeFailure as error:
            raise _failure(error, call, outputs=batch) from None
        return RoutedBatch(tuple(self._item(item, output, source) for item, output in zip(raw, batch)))

    def _item(self, item: dict, output: OutputIntent, source: InstanceAddress) -> Routed | Unmatched | Deferred:
        if item["kind"] == "unmatched":
            return Unmatched(source, output)
        destination = self._table.addresses[item["destination"]]
        ids = _require(self._table.ids[item["destination"]])
        if item["kind"] == "routed":
            return Routed(source, destination, Event(ids.resolve("event", item["event"]), output.payload))
        return Deferred(source, destination, ids.resolve("capability", item["capability"]),
            CapabilityStatus(item["status"]), output)

    def bind(self, instances: Mapping[InstanceAddress[Any, Any], Machine[Any, Any, Any]]
            | Iterable[tuple[InstanceAddress[Any, Any], Machine[Any, Any, Any]]]) -> BoundComposition:
        """Take exclusive use of every registered machine until the binding is disposed."""
        pairs = instances.items() if isinstance(instances, Mapping) else instances
        entries = tuple(islice(pairs, 2 * HARD_MAX_INSTANCES + 1))
        for entry in entries:
            if (not isinstance(entry, tuple) or len(entry) != 2 or not isinstance(entry[0], InstanceAddress)
                    or not isinstance(entry[1], Machine)):
                raise TypeError("Bindings must be (InstanceAddress, Machine) pairs")
        return BoundComposition(self, entries)

class BoundComposition:
    """Exclusive use of bound machines, like Rust's ``&mut`` borrows.

    While bound, each machine refuses direct operations with ``BusyError``.
    ``propagate`` runs synchronously in the caller's stack; dispose the binding
    before executing returned effects.
    """
    def __init__(self, composition: Composition, entries: tuple[tuple[InstanceAddress, Machine[Any, Any, Any]], ...]) -> None:
        self._composition = composition
        call = _Call(composition._table)
        self._addresses = [call.address(address) for address, _ in entries]
        self._extra = call.extra
        self._machines = [machine for _, machine in entries]
        self._contexts: list[Context] = []
        self._lock = threading.Lock()
        contexts = [machine._context for machine in self._machines]
        with ExitStack() as stack:
            for context in dict.fromkeys(contexts):
                if not context._lock.acquire(blocking=False):
                    raise errors.BusyError("An operation is already active on this instance")
                stack.callback(context._lock.release)
                if context.owner is not None:
                    raise errors.BusyError("The machine is already bound to a composition")
            try:
                composition._native.bind(_encode({"extra": self._extra, "addresses": self._addresses}),
                    [machine._context_native() for machine in self._machines])
            except _native.NativeFailure as error:
                raise _failure(error, call) from None
            for context in contexts:
                context.owner = self
        self._contexts = contexts

    def propagate(self, inputs: Iterable[Delivery[Any, Any]], *, limits: PropagationLimits, facts: object = None,
            capabilities: CapabilitySnapshot | None = None) -> PropagationReport:
        """Dispatch inputs and route accepted outputs FIFO until quiescence or a budget stop."""
        if not isinstance(limits, PropagationLimits):
            raise TypeError("limits must be PropagationLimits")
        if not self._lock.acquire(blocking=False):
            raise errors.BusyError("This composition is already propagating")
        try:
            if not self._contexts:
                raise errors.DisposedError("The composition binding has been disposed")
            return self._propagate(inputs, limits, facts, capabilities)
        finally:
            self._lock.release()

    def _propagate(self, inputs, limits, facts, capabilities) -> PropagationReport:
        composition = self._composition
        deliveries = tuple(islice(inputs, limits.max_intents + 1))
        if not all(isinstance(delivery, Delivery) for delivery in deliveries):
            raise TypeError("inputs must be Delivery values")
        # Every bound address is registered, so each call starts a fresh extra list.
        call = _Call(composition._table)
        lowered = []
        for position, delivery in enumerate(deliveries):
            index = call.address(delivery.destination)
            lowered.append({"destination": index, "payload": position + 1,
                "event": _find(call.ids(index), "event", delivery.event.kind)})
        scratch = _Scratch(len(deliveries) + 1, (facts, *(delivery.event.payload for delivery in deliveries)))
        spec = {"binding": {"extra": call.extra, "addresses": self._addresses}, "inputs": lowered, "facts": 0,
            "max_steps": limits.max_steps, "max_intents": limits.max_intents}
        sources: dict[str | None, Any] = {context.arena_id: context for context in self._contexts}
        sources[scratch.arena_id] = scratch
        snapshot = composition._snapshot(capabilities)
        with ExitStack() as stack:
            for context in dict.fromkeys(self._contexts):
                stack.enter_context(context.operation(sources=sources, owner=self))
            try:
                encoded, failure = composition._native.propagate(scratch.native, _encode(spec),
                    [machine._context_native() for machine in self._machines],
                    [context.invoke for context in self._contexts], snapshot)
            except _native.NativeFailure as error:
                raise _failure(error, call) from None
            return _Report(composition._table, call, sources, deliveries).build(json.loads(encoded), failure)

    def dispose(self) -> None:
        """Release exclusive use; idempotent."""
        with self._lock:
            for context in self._contexts:
                if context.owner is self:
                    context.owner = None
            self._contexts = []

    close = dispose

    def __enter__(self) -> BoundComposition:
        if not self._contexts:
            raise errors.DisposedError("The composition binding has been disposed")
        return self

    def __exit__(self, *_: object) -> None:
        self.dispose()

class _Scratch:
    """Call-local arena for caller inputs and facts; no callback runs in it."""
    def __init__(self, capacity: int, values: tuple) -> None:
        self.native = _native._Arena(capacity)
        self.arena_id = self.native.arena_id()
        self.values = dict(enumerate(values))

class _Report:
    def __init__(self, table: _Table, call: _Call, sources: dict[str | None, Any], deliveries: tuple[Delivery, ...]) -> None:
        self.table, self.call, self.sources, self.deliveries = table, call, sources, deliveries

    def value(self, key: dict) -> object:
        return self.sources[key["arena"]].values[key["key"]]

    def build(self, raw: dict, failure: _native.NativeFailure | None) -> PropagationReport:
        table, value = self.table, self.value
        steps = []
        for step in raw["steps"]:
            ids = _require(table.ids[step["destination"]])
            kind = None if step["input"] is None else self.deliveries[step["input"]].event.kind
            steps.append(PropagationStep(table.addresses[step["destination"]],
                decode_outcome(step["outcome"], ids, value, kind)))
        effects = tuple(AddressedEffect(table.addresses[effect["source"]], EffectIntent(
            _require(table.ids[effect["source"]]).resolve("effect", effect["id"]), value(effect["payload"])))
            for effect in raw["effects"])
        pending = tuple(self.deliveries[item["input"]] if item["input"] is not None else Delivery(
            table.addresses[item["destination"]], Event(_require(table.ids[item["destination"]]).resolve(
                "event", item["event"]), value(item["payload"]))) for item in raw["pending"])
        error = None
        if failure is not None:
            error = exception(failure, table.ids[self.table.indexes[pending[0].destination]])
        return PropagationReport(PropagationStatus(raw["status"]), tuple(steps), effects,
            tuple(self.unrouted(item) for item in raw["unrouted"]), pending, error)

    def unrouted(self, item: dict) -> Unmatched | Deferred:
        source = self.table.addresses[item["source"]]
        intent = OutputIntent(_require(self.table.ids[item["source"]]).resolve("event", item["event"]),
            self.value(item["payload"]))
        if item["kind"] == "unmatched":
            return Unmatched(source, intent)
        ids = _require(self.table.ids[item["destination"]])
        return Deferred(source, self.table.addresses[item["destination"]],
            ids.resolve("capability", item["capability"]), CapabilityStatus(item["status"]), intent)
