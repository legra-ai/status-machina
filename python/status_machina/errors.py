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

class RecordError(DefinitionError):
    """A portable definition record failed checked binding; ``context`` names native IDs."""
class ConditionRecordError(RecordError):
    """A condition record is structurally invalid; ``context['condition']`` is its ID."""
class TransitionRecordError(RecordError):
    """A transition record is structurally invalid; ``context['transition']`` is its ID."""
class DispositionRecordError(RecordError):
    """A disposition record is structurally invalid; ``context['disposition']`` is its ID."""
class BindingError(RecordError):
    """Exact Story resolution failed; no implementation was substituted or loaded."""
class UnknownBehaviorError(BindingError):
    """No implementation of ``context['story']`` is registered."""
class UnknownImplementationError(BindingError):
    """The Story is registered, but not ``context['implementation']``."""
class RoleMismatchError(BindingError):
    """The selected implementation has the ``context['actual']`` BehaviorRole, not ``expected``."""
class IdentityMapError(StatusMachinaError):
    """An identity codec is invalid or was asked for an unregistered value."""
class IdentityRoleError(IdentityMapError):
    """A codec was supplied for, or requested from, the wrong identity role."""
class IdentityMapLimitError(IdentityMapError, LimitError):
    """The codec bound is invalid (1..4096) or the entries exceed it."""
class DuplicateIdentityError(IdentityMapError):
    """One identity occurs twice; ``context['entry']`` is the second position."""
class DuplicateRepresentationError(IdentityMapError):
    """Two identities share one external representation."""
class UnknownIdentityError(IdentityMapError, LookupError):
    """The identity has no registered representation; none is derived from str/repr."""
class UnknownRepresentationError(IdentityMapError, LookupError):
    """The representation has no registered identity; no default is substituted."""
