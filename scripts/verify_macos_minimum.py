"""Require macOS release artifacts to declare exactly the tested minimum macOS.

The native package matrix supports macOS 26 ARM64 only, because that is the
only macOS CI tests. A Mach-O binary that declares an older minimum would
advertise support nothing verified. Accepts native modules and wheels.
"""

import argparse
from collections.abc import Iterable, Iterator
from pathlib import Path
import re
import subprocess

from native_binaries import native_binaries


def minimum_version(lines: Iterable[str]) -> tuple[int, ...]:
    """Read the deployment target from `otool -l` load commands, streaming."""
    found = []
    command = None
    for line in lines:
        fields = line.split()
        if fields[:1] == ['cmd']:
            command = fields[1] if len(fields) > 1 else None
        elif command == 'LC_BUILD_VERSION' and fields[:1] == ['minos']:
            found.append(fields[1])
        elif command == 'LC_VERSION_MIN_MACOSX' and fields[:1] == ['version']:
            found.append(fields[1])
    if len(set(found)) != 1:
        raise ValueError(f'Expected one macOS deployment target, found {found or "none"}')
    if not re.fullmatch(r'\d+(?:\.\d+){0,2}', found[0]):
        raise ValueError(f'Unreadable macOS deployment target {found[0]!r}')
    return normalized(found[0])


def normalized(version: str) -> tuple[int, ...]:
    parts = tuple(map(int, version.split('.')))
    return parts + (0,) * (3 - len(parts))


def require_minimum(lines: Iterable[str], expected: str) -> tuple[int, ...]:
    declared = minimum_version(lines)
    if declared != normalized(expected):
        shown = '.'.join(map(str, declared))
        raise ValueError(f'Declared macOS minimum {shown} differs from the tested minimum {expected}')
    return declared


def load_commands(binary: Path) -> Iterator[str]:
    with subprocess.Popen(['otool', '-l', str(binary)], stdout=subprocess.PIPE, text=True) as process:
        yield from process.stdout
        result = process.wait()
        if result != 0:
            raise subprocess.CalledProcessError(result, process.args)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split('\n', 1)[0])
    parser.add_argument('--minimum', required=True)
    parser.add_argument('binaries', nargs='+', type=Path)
    arguments = parser.parse_args()
    if not re.fullmatch(r'\d+\.\d+', arguments.minimum):
        parser.error('--minimum must be a major.minor macOS version')
    with native_binaries(arguments.binaries) as binaries:
        for binary in binaries:
            require_minimum(load_commands(binary), arguments.minimum)
            print(f'{binary.name}: macOS minimum {arguments.minimum}')


if __name__ == '__main__':
    main()
