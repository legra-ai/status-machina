"""Keep every binding manifest at the published core crate's version.

The root `Cargo.toml` package version is the single source of truth; release
preparation (`cargo version-info bump`) rewrites it and then runs this script
as a `pre_bump_hook`, before it refreshes `Cargo.lock`. Every file this script
writes is therefore part of the version commit, and the Python wheel, source
distribution and npm tarball built from that commit carry the crate's version.

    python3 scripts/sync_binding_versions.py 0.1.14   # rewrite to this version
    python3 scripts/sync_binding_versions.py --check  # verify against the crate
"""

import argparse
import json
from pathlib import Path
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parent.parent
VERSION = re.compile(r'(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-(?:alpha|beta|rc)\.(?:0|[1-9]\d*))?')
PEP440_CHANNELS = {'alpha': 'a', 'beta': 'b', 'rc': 'rc'}
CARGO_MEMBERS = ('bindings/core/Cargo.toml', 'python/status-machina-pyo3/Cargo.toml',
                 'npm/status-machina-napi/Cargo.toml')


class VersionedFile:
    """One manifest whose single version field is located by a strict pattern."""

    def __init__(self, path, pattern):
        self.path = path
        self.pattern = re.compile(pattern, flags=re.MULTILINE)

    def _match(self, text):
        matches = list(self.pattern.finditer(text))
        if len(matches) != 1:
            raise ValueError(f'{self.path}: expected exactly one version field, found {len(matches)}')
        return matches[0]

    def read(self, root):
        return self._match((root / self.path).read_text(encoding='utf-8')).group('version')

    def write(self, root, version):
        target = root / self.path
        text = target.read_text(encoding='utf-8')
        match = self._match(text)
        start, end = match.span('version')
        target.write_text(text[:start] + version + text[end:], encoding='utf-8', newline='\n')


def python_version(version):
    """Spell a Cargo pre-release in PEP 440 form (1.2.3-rc.1 -> 1.2.3rc1)."""
    match = re.fullmatch(r'(\d+\.\d+\.\d+)(?:-(alpha|beta|rc)\.(\d+))?', version)
    if match is None:
        raise ValueError(f'Unsupported version: {version!r}')
    base, channel, number = match.groups()
    return base if channel is None else f'{base}{PEP440_CHANNELS[channel]}{number}'


FILES = {
    **{path: (VersionedFile(path, r'\A\[package\]\n(?:(?!\[).*\n)*?version = "(?P<version>[^"]+)"$'), str)
       for path in CARGO_MEMBERS},
    'python/pyproject.toml': (VersionedFile(
        'python/pyproject.toml', r'^\[project\]\n(?:(?!\[).*\n)*?version = "(?P<version>[^"]+)"$'), python_version),
    'python/uv.lock': (VersionedFile(
        'python/uv.lock', r'^name = "status-machina"\nversion = "(?P<version>[^"]+)"\nsource = \{ editable = "\." \}$'),
        python_version),
    'npm/package.json': (VersionedFile(
        'npm/package.json', r'\A\{\n  "name": "@legra-ai/status-machina",\n  "version": "(?P<version>[^"]+)",$'), str),
}


def crate_version(root):
    version = tomllib.loads((root / 'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
    if not isinstance(version, str) or not VERSION.fullmatch(version):
        raise ValueError(f'Unsupported core crate version: {version!r}')
    return version


def mismatches(root, version):
    return [(path, found, spell(version)) for path, (field, spell) in FILES.items()
            if (found := field.read(root)) != spell(version)]


def synchronize(root, version):
    if not VERSION.fullmatch(version):
        raise ValueError(f'Unsupported version: {version!r}')
    if version != crate_version(root):
        raise ValueError(f'Version {version} differs from the core crate version {crate_version(root)}')
    for field, spell in FILES.values():
        field.write(root, spell(version))
    remaining = mismatches(root, version)
    if remaining:
        raise RuntimeError(f'Versions remain unsynchronized: {remaining}')
    # npm/package.json must stay valid JSON after the textual rewrite.
    json.loads((root / 'npm/package.json').read_text(encoding='utf-8'))


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__.split('\n', 1)[0])
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('version', nargs='?')
    group.add_argument('--check', action='store_true')
    parser.add_argument('--root', type=Path, default=ROOT)
    options = parser.parse_args(arguments)
    if options.check:
        version = crate_version(options.root)
        remaining = mismatches(options.root, version)
        for path, found, expected in remaining:
            print(f'{path}: {found} != {expected}', file=sys.stderr)
        if remaining:
            return 1
        print(f'Every binding manifest is at {version}')
        return 0
    synchronize(options.root, options.version)
    print(f'Synchronized every binding manifest to {options.version}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
