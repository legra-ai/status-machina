"""Bundle, recover and publish the tested Python and npm packages of one release.

    python3 scripts/release_packages.py bundle DIST_DIR OUTPUT.zip
    python3 scripts/release_packages.py extract BUNDLE.zip DIRECTORY
    python3 scripts/release_packages.py plan DIRECTORY PENDING_PYTHON_DIR
    python3 scripts/release_packages.py preflight
    python3 scripts/release_packages.py verify-pypi DIRECTORY

`bundle` runs in every CI run after every artifact was built and its installed
tests passed; it records the run's own revision. The shared release pipeline
attaches the bundle to the draft release exactly once, from the release
revision's own run, and a resumed release keeps that original asset. The
publishing job therefore always works from the preserved asset (`extract`),
never from a later run's rebuilt packages. `plan` compares it with what the
registries already hold: identical files are skipped, missing ones published,
and any other file under the release version fails. `preflight` proves the
OIDC trust of every registry that still needs an upload before either upload
starts, so a missing trusted publisher fails loudly instead of skipping.
`verify-pypi` requires the public file set, wheels and sdist, to be exactly the
bundle's.
"""

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sys
import time
import tomllib
from urllib.error import HTTPError
from urllib.parse import quote
from urllib.request import Request, urlopen
import zipfile

ROOT = Path(__file__).resolve().parent.parent
PYPI_PROJECT = 'status-machina'
NPM_PACKAGE = '@legra-ai/status-machina'
# The declared abi3 wheel platforms; the CI wheel matrix must build exactly these.
WHEEL_TAGS = frozenset({
    'cp310-abi3-manylinux_2_28_x86_64', 'cp310-abi3-manylinux_2_28_aarch64',
    'cp310-abi3-macosx_26_0_arm64', 'cp310-abi3-win_amd64', 'cp310-abi3-win_arm64',
})
MANIFEST = 'manifest.json'
CHUNK = 1 << 20


class ReleaseError(ValueError):
    """A release package, registry state or trust configuration is not publishable."""


def sha256(path):
    with Path(path).open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


class PackageSet:
    """The exact files one release publishes: five wheels, one sdist, one npm tarball."""

    def __init__(self, version, names):
        self.version = version
        self.names = frozenset(names)
        expected = {f'status_machina-{version}-{tag}.whl' for tag in WHEEL_TAGS}
        expected |= {self.sdist, self.npm}
        if self.names != expected:
            missing, extra = sorted(expected - self.names), sorted(self.names - expected)
            raise ReleaseError(f'Release packages differ from the declared set: missing {missing}, extra {extra}')

    @property
    def sdist(self):
        return f'status_machina-{self.version}.tar.gz'

    @property
    def npm(self):
        return f'legra-ai-status-machina-{self.version}.tgz'

    @property
    def python(self):
        return sorted(name for name in self.names if name != self.npm)


class Bundle:
    """release-packages.zip: a manifest naming version, revision and SHA-256 per package."""

    def __init__(self, path):
        self.path = Path(path)

    @classmethod
    def write(cls, path, version, revision, packages):
        if not re.fullmatch(r'[0-9a-f]{40}', revision):
            raise ReleaseError('Expected the complete 40-character tested revision')
        PackageSet(version, (package.name for package in packages))
        manifest = {'version': version, 'revision': revision,
                    'files': {package.name: sha256(package) for package in sorted(packages)}}
        with zipfile.ZipFile(path, 'x', compression=zipfile.ZIP_STORED) as archive:
            archive.writestr(MANIFEST, json.dumps(manifest, sort_keys=True))
            for package in sorted(packages):
                archive.write(package, package.name)
        return cls(path)

    def manifest(self):
        with zipfile.ZipFile(self.path) as archive:
            if archive.namelist().count(MANIFEST) != 1:
                raise ReleaseError('Expected exactly one bundle manifest')
            return json.loads(archive.read(MANIFEST))

    def extract(self, directory, version, revision):
        """Unpack only a bundle of this exact release whose every digest matches."""
        manifest = self.manifest()
        if (manifest['version'], manifest['revision']) != (version, revision):
            raise ReleaseError(f'Bundle is {manifest["version"]} at {manifest["revision"]}, '
                               f'not {version} at {revision}')
        packages = PackageSet(version, manifest['files'])
        directory = Path(directory)
        directory.mkdir(parents=True)
        with zipfile.ZipFile(self.path) as archive:
            names = archive.namelist()
            if len(names) != len(set(names)) or set(names) != {MANIFEST, *packages.names}:
                raise ReleaseError('Bundle contents differ from its manifest')
            for name, expected in manifest['files'].items():
                target = directory / name
                with archive.open(name) as source, target.open('xb') as destination:
                    shutil.copyfileobj(source, destination, CHUNK)
                if sha256(target) != expected:
                    raise ReleaseError(f'Bundle package identity mismatch: {name}')
        return packages


class Http:
    """Anonymous JSON access to public registries; 404 means absent."""

    def json(self, url, *, method='GET', body=None, headers=None):
        data = json.dumps(body).encode() if body is not None else None
        request = Request(url, data=data, method=method,
                          headers={'Accept': 'application/json', 'User-Agent': 'status-machina-release',
                                   **({'Content-Type': 'application/json'} if data else {}),
                                   **(headers or {})})
        try:
            with urlopen(request, timeout=60) as response:
                return json.load(response)
        except HTTPError as error:
            if error.code == 404 and method == 'GET':
                return None
            detail = error.read(64 * 1024).decode(errors='replace')
            raise ReleaseError(f'{method} {url} failed with HTTP {error.code}: {detail}') from error

    def sha256(self, url):
        hasher = hashlib.sha256()
        with urlopen(Request(url, headers={'User-Agent': 'status-machina-release'}), timeout=60) as response:
            # bounded: streamed in 1 MiB chunks.
            while chunk := response.read(CHUNK):
                hasher.update(chunk)
        return hasher.hexdigest()


class PyPI:
    def __init__(self, http):
        self.http = http

    def published(self, version):
        """Filename to {sha256, url} of every file PyPI holds for this version."""
        metadata = self.http.json(f'https://pypi.org/pypi/{PYPI_PROJECT}/{version}/json')
        if metadata is None:
            return {}
        return {entry['filename']: {'sha256': entry['digests']['sha256'], 'url': entry['url']}
                for entry in metadata['urls']}

    def missing(self, version, directory, packages):
        """Packages still to upload; any differing or foreign file fails the release."""
        published = self.published(version)
        foreign = sorted(set(published) - set(packages.python))
        if foreign:
            raise ReleaseError(f'PyPI {version} holds files this release did not test: {foreign}')
        missing = []
        for name in packages.python:
            if name not in published:
                missing.append(name)
            elif published[name]['sha256'] != sha256(Path(directory) / name):
                raise ReleaseError(f'PyPI {name} differs from the tested package')
        return missing

    def verify(self, version, directory, packages):
        published = self.published(version)
        if set(published) != set(packages.python):
            raise NotVisible(f'PyPI {version} files {sorted(published)} are not exactly {packages.python}')
        for name in packages.python:
            expected = sha256(Path(directory) / name)
            if published[name]['sha256'] != expected or self.http.sha256(published[name]['url']) != expected:
                raise ReleaseError(f'Public PyPI {name} differs from the tested package')

    def trust(self, oidc):
        audience = self.http.json('https://pypi.org/_/oidc/audience')['audience']
        result = self.http.json('https://pypi.org/_/oidc/mint-token', method='POST',
                                body={'token': oidc.token(audience)})
        if not result or not result.get('token'):
            raise ReleaseError(f'PyPI trusted publishing is not configured: {result}')


class Npm:
    # npm's staged publishing first exposes a separate `<version>-stage` version,
    # then the real one minutes later. Only the exact version is the release.
    STAGE_SUFFIX = '-stage'

    def __init__(self, http):
        self.http = http

    def metadata(self, version):
        metadata = self.http.json(f'https://registry.npmjs.org/{quote(NPM_PACKAGE, safe="@")}/{version}')
        if metadata is not None and metadata['version'] != version:
            raise ReleaseError(f'npm answered {metadata["version"]} for {version}')
        return metadata

    def missing(self, version, directory, packages):
        """True when nothing is published; waits out a staged publication of this version."""
        metadata = self.metadata(version)
        if metadata is None:
            if self.metadata(version + self.STAGE_SUFFIX) is None:
                return True
            # An earlier attempt's upload is still being staged: never upload twice.
            raise NotVisible(f'npm {NPM_PACKAGE}@{version} is still staged as {version}{self.STAGE_SUFFIX}')
        with (Path(directory) / packages.npm).open('rb') as source:
            integrity = 'sha512-' + base64.b64encode(hashlib.file_digest(source, 'sha512').digest()).decode()
        if metadata['dist']['integrity'] != integrity:
            raise ReleaseError(f'npm {NPM_PACKAGE}@{version} differs from the tested tarball')
        return False

    def trust(self, oidc):
        token = oidc.token('npm:registry.npmjs.org')
        url = ('https://registry.npmjs.org/-/npm/v1/oidc/token/exchange/package/'
               + quote(NPM_PACKAGE, safe=''))
        result = self.http.json(url, method='POST', headers={'Authorization': f'Bearer {token}'})
        if not result or not result.get('token'):
            raise ReleaseError(f'npm trusted publishing is not configured: {result}')


class GitHubOidc:
    """The job's OIDC identity; absent without `id-token: write`."""

    def __init__(self, http, environment):
        self.http = http
        self.url = environment.get('ACTIONS_ID_TOKEN_REQUEST_URL')
        self.bearer = environment.get('ACTIONS_ID_TOKEN_REQUEST_TOKEN')

    def token(self, audience):
        if not self.url or not self.bearer:
            raise ReleaseError('No GitHub OIDC identity: the publishing job needs id-token: write')
        result = self.http.json(f'{self.url}&audience={quote(audience, safe="")}',
                                headers={'Authorization': f'bearer {self.bearer}'})
        return result['value']


class NotVisible(ReleaseError):
    """Uploaded files are not yet all visible in the registry's metadata."""


def await_visibility(check, timeout=300, *, clock=time.monotonic, sleep=time.sleep):
    deadline = clock() + timeout
    while True:
        try:
            return check()
        except NotVisible as error:
            if clock() >= deadline:
                raise ReleaseError(f'Not publicly visible within {timeout}s: {error}') from error
            print(f'Waiting for registry visibility: {error}', flush=True)
            sleep(5)


def crate_version():
    return tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']


def output(name, value):
    with open(os.environ['GITHUB_OUTPUT'], 'a') as stream:
        stream.write(f'{name}={value}\n')


def release_identity():
    return os.environ['RELEASE_VERSION'], os.environ['RELEASE_REVISION']


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('command', choices=['bundle', 'extract', 'plan', 'preflight', 'verify-pypi'])
    parser.add_argument('paths', nargs='*', type=Path)
    options = parser.parse_args(arguments)
    http = Http()
    if options.command == 'bundle':
        source, target = options.paths
        packages = [path for path in source.iterdir() if path.is_file()]
        Bundle.write(target, crate_version(), os.environ['GITHUB_SHA'], packages)
        print(f'Bundled {len(packages)} tested packages of {crate_version()} at {os.environ["GITHUB_SHA"]}')
    elif options.command == 'extract':
        bundle, directory = options.paths
        packages = Bundle(bundle).extract(directory, *release_identity())
        print(f'Recovered {len(packages.names)} tested packages from {bundle}')
    elif options.command == 'plan':
        directory, pending = options.paths
        version, _ = release_identity()
        packages = PackageSet(version, (path.name for path in directory.iterdir()))
        python = PyPI(http).missing(version, directory, packages)
        npm = await_visibility(lambda: Npm(http).missing(version, directory, packages), 600)
        pending.mkdir()
        for name in python:
            shutil.copyfile(directory / name, pending / name)
        print(f'PyPI missing: {python or "none"}; npm missing: {npm}')
        output('python-missing', str(bool(python)).lower())
        output('npm-missing', str(npm).lower())
        output('npm-archive', str(directory / packages.npm))
    elif options.command == 'preflight':
        oidc = GitHubOidc(http, os.environ)
        for registry, needed in [(PyPI(http), os.environ['PYTHON_MISSING']), (Npm(http), os.environ['NPM_MISSING'])]:
            if needed not in ('true', 'false'):
                raise ReleaseError(f'Unexpected publication plan: {needed!r}')
            if needed == 'true':
                registry.trust(oidc)
                print(f'{type(registry).__name__} trusted publishing verified', flush=True)
    else:
        (directory,) = options.paths
        version, _ = release_identity()
        packages = PackageSet(version, (path.name for path in directory.iterdir()))
        await_visibility(lambda: PyPI(http).verify(version, directory, packages))
        print(f'PyPI {PYPI_PROJECT} {version}: every wheel and the sdist match the tested packages')


if __name__ == '__main__':
    try:
        main()
    except ReleaseError as error:
        print(f'::error::{error}', file=sys.stderr)
        sys.exit(1)
