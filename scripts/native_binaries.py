"""Yield the native binaries of release artifacts: plain files or those inside wheels."""

from collections.abc import Iterable, Iterator
from contextlib import contextmanager
from pathlib import Path
import tempfile
import zipfile

NATIVE_SUFFIXES = ('.so', '.pyd', '.dylib', '.node')


@contextmanager
def native_binaries(artifacts: Iterable[Path]) -> Iterator[list[Path]]:
    """Expand each wheel into its native members; fail if any artifact holds none."""
    with tempfile.TemporaryDirectory(prefix='status-machina-native-') as directory:
        expanded = []
        for index, artifact in enumerate(artifacts):
            if artifact.suffix != '.whl':
                expanded.append(artifact)
                continue
            with zipfile.ZipFile(artifact) as wheel:
                members = [name for name in wheel.namelist() if name.endswith(NATIVE_SUFFIXES)]
                if not members:
                    raise ValueError(f'{artifact}: wheel contains no native module')
                for member in members:
                    expanded.append(Path(wheel.extract(member, Path(directory) / str(index))))
        if not expanded:
            raise ValueError('No release artifacts to inspect')
        yield expanded
