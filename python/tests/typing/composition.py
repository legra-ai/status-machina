"""Static contracts for addresses, routes, registries, batches and propagation reports."""
from enum import Enum, auto
from typing_extensions import assert_type
from status_machina import (
    CapabilityStatus, Composition, Delivery, Event, InstanceAddress, InstanceRegistry, Machine,
    OutputIntent, PropagationLimits, PropagationReport, PropagationStatus, Route, RoutedBatch,
    StatusMachinaError,
)

class Phase(Enum):
    OPEN = auto()

class Input(Enum):
    SHIP = auto()

class Output(Enum):
    PAID = auto()

address = InstanceAddress("tenant-a", "order-1")
assert_type(address, InstanceAddress[str, str])
assert_type(address.scope, str)
route = Route(address, Output.PAID, address, Input.SHIP)
assert_type(route.input_event, Input)
limits = PropagationLimits(max_steps=8, max_intents=8)
delivery = Delivery(address, Event(Input.SHIP, 3))
assert_type(delivery.event.payload, int)
Route("tenant-a", Output.PAID, address, Input.SHIP)  # type: ignore[arg-type]
Delivery(address, (Input.SHIP, None))  # type: ignore[arg-type]
PropagationLimits(max_steps="8", max_intents=8)  # type: ignore[arg-type]

def use(composition: Composition, machine: Machine[Phase, Input, int]) -> None:
    InstanceRegistry(((address, machine),), max_instances="1")  # type: ignore[arg-type]
    with composition.bind({address: machine}) as bound:
        report = bound.propagate([delivery], limits=limits, facts=None)
        assert_type(report, PropagationReport)
        assert_type(report.status, PropagationStatus)
        assert_type(report.error, StatusMachinaError | None)
        bound.propagate([delivery])  # type: ignore[call-arg]
        bound.propagate([Event(Input.SHIP, 3)], limits=limits)  # type: ignore[list-item]
    batch = composition.route_batch(address, [OutputIntent(Output.PAID, 1)])
    assert_type(batch, RoutedBatch)
    for item in batch:
        if item.kind == "deferred":
            assert_type(item.status, CapabilityStatus)
        elif item.kind == "routed":
            assert_type(item.destination, InstanceAddress)
    composition.route_batch("tenant-a", [])  # type: ignore[arg-type]
