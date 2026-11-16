"""Assert the exact contents and metadata of the native release artifacts.

    python3 scripts/package_contents.py wheel DIST.whl --tag cp310-abi3-win_amd64
    python3 scripts/package_contents.py sdist DIST.tar.gz
    python3 scripts/package_contents.py npm PACKAGE.tgz

Expected files are derived from the tracked sources of this checkout, so a new
module that is missing from an artifact, or a stray file that leaks into one,
fails the build instead of shipping. The version is the core crate's version.
"""

import argparse
from email.parser import HeaderParser
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
import tomllib
import zipfile

from sync_binding_versions import python_version

ROOT = Path(__file__).resolve().parent.parent
LICENSES = ('LICENSE-MIT', 'LICENSE-APACHE')
NATIVE_SUFFIXES = ('.so', '.pyd', '.dylib', '.dll', '.node', '.exe', '.a', '.lib', '.rlib')
# bounded: one entry per declared napi target.
NODE_SUFFIXES = {
    'x86_64-unknown-linux-gnu': 'linux-x64-gnu',
    'aarch64-unknown-linux-gnu': 'linux-arm64-gnu',
    'aarch64-apple-darwin': 'darwin-arm64',
    'x86_64-pc-windows-msvc': 'win32-x64-msvc',
    'aarch64-pc-windows-msvc': 'win32-arm64-msvc',
}


class ContentsError(ValueError):
    """An artifact differs from the contract for its kind."""


class Checkout:
    """Tracked sources that define what each artifact must contain."""

    def __init__(self, root):
        self.root = root

    def tracked(self, *paths):
        names = subprocess.check_output(['git', 'ls-files', '-z', '--', *paths], cwd=self.root, text=True)
        return [name for name in names.split('\0') if name]

    def crate_version(self):
        return tomllib.loads((self.root / 'Cargo.toml').read_text(encoding='utf-8'))['package']['version']

    def license(self, name):
        return (self.root / name).read_bytes()

    def npm_manifest(self):
        return json.loads((self.root / 'npm/package.json').read_text(encoding='utf-8'))


def compare(kind, actual, expected, optional=()):
    unexpected = sorted(name for name in actual - expected
                        if not any(re.fullmatch(pattern, name) for pattern in optional))
    missing = sorted(expected - actual)
    if unexpected or missing:
        raise ContentsError(f'{kind} contents differ: missing {missing}, unexpected {unexpected}')


def require_python_metadata(kind, text, version):
    metadata = HeaderParser().parsestr(text)
    checks = {
        'Name': ['status-machina'],
        'Version': [python_version(version)],
        'License-Expression': ['MIT OR Apache-2.0'],
        'Requires-Python': ['>=3.10'],
        'License-File': [f'licenses/{name}' for name in LICENSES],
    }
    for field, values in checks.items():
        found = metadata.get_all(field) or []
        if sorted(found) != sorted(values):
            raise ContentsError(f'{kind} {field} is {found}, expected {values}')
    if not any(value.startswith('typing-extensions') for value in metadata.get_all('Requires-Dist') or []):
        raise ContentsError(f'{kind} must require typing-extensions')


def python_sources(checkout):
    return {name.removeprefix('python/') for name in checkout.tracked('python/status_machina')}


class Wheel:
    """One abi3 wheel: the typed package, exactly one native module, licenses."""

    def __init__(self, checkout, path, tag):
        self.checkout = checkout
        self.path = path
        self.tag = tag

    def check(self, version):
        if not re.fullmatch(r'cp310-abi3-(?:manylinux_2_28_(?:x86_64|aarch64)|macosx_26_0_arm64|win_(?:amd64|arm64))',
                            self.tag):
            raise ContentsError(f'{self.tag} is not a declared abi3-py310 platform tag')
        dist = f'status_machina-{python_version(version)}'
        if self.path.name != f'{dist}-{self.tag}.whl':
            raise ContentsError(f'Wheel {self.path.name} is not {dist}-{self.tag}.whl')
        native = 'status_machina/_native.pyd' if '-win_' in self.tag else 'status_machina/_native.abi3.so'
        info = f'{dist}.dist-info'
        expected = python_sources(self.checkout) | {native, f'{info}/METADATA', f'{info}/WHEEL', f'{info}/RECORD'}
        expected |= {f'{info}/licenses/licenses/{name}' for name in LICENSES}
        with zipfile.ZipFile(self.path) as wheel:
            compare('Wheel', set(wheel.namelist()), expected,
                    optional=[re.escape(info) + r'/sboms/[A-Za-z0-9_.-]+\.cyclonedx\.json'])
            for name in LICENSES:
                if wheel.read(f'{info}/licenses/licenses/{name}') != self.checkout.license(name):
                    raise ContentsError(f'Wheel {name} differs from the repository license')
            require_python_metadata('Wheel', wheel.read(f'{info}/METADATA').decode(), version)
            header = HeaderParser().parsestr(wheel.read(f'{info}/WHEEL').decode())
        if header.get_all('Tag') != [self.tag] or header['Root-Is-Purelib'] != 'false':
            raise ContentsError(f'Wheel tags {header.get_all("Tag")} differ from {self.tag}')


class SourceDistribution:
    """The Python sdist: every source a build needs, no build products or VCS state."""

    FIXED = {'PKG-INFO', 'pyproject.toml', 'Cargo.toml', 'Cargo.lock', 'README.md', 'build.rs',
             *LICENSES, *(f'licenses/{name}' for name in LICENSES)}
    FORBIDDEN = re.compile(r'(?:^|/)(?:\.git|\.github|target|node_modules|__pycache__|\.venv|dist)(?:/|$)'
                           r'|(?:^|/)\.env|^npm/')

    def __init__(self, checkout, path):
        self.checkout = checkout
        self.path = path

    def check(self, version):
        top = f'status_machina-{python_version(version)}'
        if self.path.name != f'{top}.tar.gz':
            raise ContentsError(f'Source distribution {self.path.name} is not {top}.tar.gz')
        with tarfile.open(self.path) as archive:
            members = archive.getmembers()
            names = set()
            for member in members:
                path = PurePosixPath(member.name)
                if path.parts[0] != top or '..' in path.parts or member.issym() or member.islnk():
                    raise ContentsError(f'Unsafe source distribution member {member.name}')
                if member.isfile():
                    names.add(str(PurePosixPath(*path.parts[1:])))
            for name in sorted(names):
                if self.FORBIDDEN.search(name) or name.endswith(NATIVE_SUFFIXES):
                    raise ContentsError(f'Source distribution must not contain {name}')
            required = self.FIXED | python_sources(self.checkout)
            required |= set(self.checkout.tracked('src', 'tests', 'bindings/core', 'python/status-machina-pyo3'))
            missing = sorted(required - names)
            if missing:
                raise ContentsError(f'Source distribution omits {missing}')
            for name in (*LICENSES, *(f'licenses/{name}' for name in LICENSES)):
                if archive.extractfile(f'{top}/{name}').read() != self.checkout.license(PurePosixPath(name).name):
                    raise ContentsError(f'Source distribution {name} differs from the repository license')
            require_python_metadata('Source distribution', archive.extractfile(f'{top}/PKG-INFO').read().decode(),
                                    version)


class NpmTarball:
    """The npm package: entry points, private modules, licenses and every declared addon."""

    FIXED = {'package.json', 'README.md', 'index.mjs', 'index.cjs', 'index.d.ts', *LICENSES}

    def __init__(self, checkout, path):
        self.checkout = checkout
        self.path = path

    def addons(self):
        targets = self.checkout.npm_manifest()['napi']['targets']
        unknown = sorted(set(targets) - set(NODE_SUFFIXES))
        if unknown:
            raise ContentsError(f'No addon file name is declared for napi targets {unknown}')
        return {f'status_machina.{NODE_SUFFIXES[target]}.node' for target in targets}

    def check(self, version):
        if self.path.name != f'legra-ai-status-machina-{version}.tgz':
            raise ContentsError(f'npm tarball {self.path.name} is not legra-ai-status-machina-{version}.tgz')
        expected = self.FIXED | self.addons() | {name.removeprefix('npm/') for name in self.checkout.tracked('npm/lib')}
        with tarfile.open(self.path) as archive:
            names = set()
            for member in archive.getmembers():
                path = PurePosixPath(member.name)
                if path.parts[0] != 'package' or '..' in path.parts or not member.isfile():
                    raise ContentsError(f'Unsafe npm tarball member {member.name}')
                names.add(str(PurePosixPath(*path.parts[1:])))
            compare('npm tarball', names, expected)
            for name in LICENSES:
                if archive.extractfile(f'package/{name}').read() != self.checkout.license(name):
                    raise ContentsError(f'npm tarball {name} differs from the repository license')
            manifest = json.loads(archive.extractfile('package/package.json').read())
        for field, value in [('name', '@legra-ai/status-machina'), ('version', version),
                             ('license', 'MIT OR Apache-2.0')]:
            if manifest.get(field) != value:
                raise ContentsError(f'npm package {field} is {manifest.get(field)!r}, expected {value!r}')
        if manifest.get('engines', {}).get('node') != '>=22.10':
            raise ContentsError('npm package must declare engines.node >=22.10')
        if 'scripts' in manifest and any(hook in manifest['scripts'] for hook in ('preinstall', 'install', 'postinstall')):
            raise ContentsError('npm package must not run install scripts')


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__.split('\n', 1)[0])
    parser.add_argument('kind', choices=['wheel', 'sdist', 'npm'])
    parser.add_argument('artifact', type=Path)
    parser.add_argument('--tag', help='the expected wheel tag, e.g. cp310-abi3-win_amd64')
    parser.add_argument('--root', type=Path, default=ROOT)
    options = parser.parse_args(arguments)
    checkout = Checkout(options.root)
    if options.kind == 'wheel':
        if not options.tag:
            parser.error('wheel checks require --tag')
        artifact = Wheel(checkout, options.artifact, options.tag)
    elif options.kind == 'sdist':
        artifact = SourceDistribution(checkout, options.artifact)
    else:
        artifact = NpmTarball(checkout, options.artifact)
    version = checkout.crate_version()
    artifact.check(version)
    print(f'{options.artifact.name}: {options.kind} contents and metadata match {version}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
