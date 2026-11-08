"""Native exceptions with structured domain context and original callback causes."""
from types import MappingProxyType

class StatusMachinaError(Exception):
    """Base error; ``context`` retains native identifiers instead of wire labels."""
    def __init__(self, message: str, **context: object) -> None:
        super().__init__(message)
        self.context = MappingProxyType(context)
        self.phase = context.get("phase")

class DefinitionError(StatusMachinaError):
    """The proposed graph or binding is invalid."""
class DispatchError(StatusMachinaError):
    """Evaluation or checked acceptance failed without committing the decision."""
class CallbackError(DispatchError):
    """Application code raised an exception; ``__cause__`` is the original object."""
class SnapshotError(CallbackError):
    """Application data could not be isolated."""
class InvariantError(DispatchError):
    """Candidate data violates a declared state invariant."""
class PostconditionError(DispatchError):
    """Candidate data violates a transition postcondition."""
class AmbiguousTransitionError(DispatchError):
    """Multiple guarded transitions or dispositions matched."""
class ActionError(DispatchError):
    """A declared candidate action refused its operation."""
class StaleDecisionError(DispatchError):
    """The expected revision is no longer current."""
class ForeignDecisionError(DispatchError):
    """A decision belongs to another definition or instance incarnation."""
class RevisionError(DispatchError):
    """The monotonic revision cannot advance."""
class DisposedError(StatusMachinaError):
    """A disposed or consumed resource cannot be used."""
class BusyError(StatusMachinaError):
    """An operation is already active; recursive/concurrent mutation is refused."""
class LimitError(StatusMachinaError):
    """A declared resource bound was reached."""
class InspectionError(StatusMachinaError):
    """The requested inspection exceeds its bounds or has invalid labels."""
