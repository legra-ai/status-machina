"""Resumable multi-registry publication decides only from tested, preserved packages."""
import base64
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location('release_packages', ROOT / 'scripts/release_packages.py')
release = importlib.util.module_from_spec(SPEC)
sys.modules['release_packages'] = release
SPEC.loader.exec_module(release)

VERSION = '1.2.3'
REVISION = 'a' * 40


def package_names(version=VERSION):
    return sorted({f'status_machina-{version}-{tag}.whl' for tag in release.WHEEL_TAGS}
                  | {f'status_machina-{version}.tar.gz', f'legra-ai-status-machina-{version}.tgz'})


class FakeHttp:
    """Registry responses by URL; records every request."""

    def __init__(self, responses=None, downloads=None):
        self.responses = responses or {}
        self.downloads = downloads or {}
        self.requests = []

    def json(self, url, *, method='GET', body=None, headers=None):
        self.requests.append((method, url, body, headers))
        response = self.responses.get((method, url.split('&audience=')[0]))
        if isinstance(response, Exception):
            raise response
        return response

    def sha256(self, url):
        return self.downloads[url]


class Fixture(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.root)
        self.dist = self.root / 'dist'
        self.dist.mkdir()
        for name in package_names():
            (self.dist / name).write_bytes(name.encode())
        self.packages = release.PackageSet(VERSION, package_names())

    def digest(self, name):
        return hashlib.sha256(name.encode()).hexdigest()

    def pypi(self, names, mutate=None):
        files = [{'filename': name, 'digests': {'sha256': self.digest(name)},
                  'url': f'https://files/{name}'} for name in names]
        if mutate:
            files[0]['digests']['sha256'] = '0' * 64
        return {('GET', f'https://pypi.org/pypi/status-machina/{VERSION}/json'): {'urls': files}}

    def npm(self, data=None):
        tarball = data or f'legra-ai-status-machina-{VERSION}.tgz'.encode()
        integrity = 'sha512-' + base64.b64encode(hashlib.sha512(tarball).digest()).decode()
        return {('GET', f'https://registry.npmjs.org/@legra-ai%2Fstatus-machina/{VERSION}'):
                {'dist': {'integrity': integrity}}}


class BundleTests(Fixture):
    def test_bundle_round_trips_only_for_its_own_release_identity(self):
        bundle = release.Bundle.write(self.root / 'release-packages.zip', VERSION, REVISION,
                                      list(self.dist.iterdir()))
        manifest = bundle.manifest()
        self.assertEqual((manifest['version'], manifest['revision']), (VERSION, REVISION))
        self.assertEqual(sorted(manifest['files']), package_names())
        recovered = bundle.extract(self.root / 'out', VERSION, REVISION)
        self.assertEqual(sorted(recovered.names), package_names())
        for name in package_names():
            self.assertEqual((self.root / 'out' / name).read_bytes(), name.encode())
        for version, revision in [('1.2.4', REVISION), (VERSION, 'b' * 40)]:
            with self.subTest(version=version, revision=revision), self.assertRaises(release.ReleaseError):
                bundle.extract(self.root / f'out-{version}-{revision[0]}', version, revision)

    def test_bundle_requires_exactly_the_declared_packages(self):
        cases = {'missing sdist': [n for n in package_names() if not n.endswith('.tar.gz')],
                 'extra wheel': package_names() + [f'status_machina-{VERSION}-cp310-abi3-macosx_11_0_x86_64.whl'],
                 'wrong version': package_names('1.2.4')}
        for label, names in cases.items():
            with self.subTest(label), self.assertRaises(release.ReleaseError):
                release.PackageSet(VERSION, names)
        with self.assertRaises(release.ReleaseError):
            release.Bundle.write(self.root / 'short.zip', VERSION, 'abc', list(self.dist.iterdir()))

    def test_tampered_bundle_is_never_extracted(self):
        path = self.root / 'tampered.zip'
        release.Bundle.write(path, VERSION, REVISION, list(self.dist.iterdir()))
        import zipfile
        with zipfile.ZipFile(path) as source, zipfile.ZipFile(self.root / 'copy.zip', 'w') as target:
            for item in source.namelist():
                data = source.read(item)
                target.writestr(item, b'changed' if item.endswith('.tar.gz') else data)
        with self.assertRaises(release.ReleaseError):
            release.Bundle(self.root / 'copy.zip').extract(self.root / 'out', VERSION, REVISION)


class ResumeTests(Fixture):
    def test_nothing_published_plans_every_package(self):
        http = FakeHttp()
        self.assertEqual(release.PyPI(http).missing(VERSION, self.dist, self.packages), self.packages.python)
        self.assertTrue(release.Npm(http).missing(VERSION, self.dist, self.packages))

    def test_interrupted_pypi_upload_resumes_only_the_missing_files(self):
        uploaded = self.packages.python[:3]
        http = FakeHttp({**self.pypi(uploaded), **self.npm()})
        self.assertEqual(release.PyPI(http).missing(VERSION, self.dist, self.packages), self.packages.python[3:])
        self.assertFalse(release.Npm(http).missing(VERSION, self.dist, self.packages))

    def test_complete_publication_plans_nothing(self):
        http = FakeHttp({**self.pypi(self.packages.python), **self.npm()})
        self.assertEqual(release.PyPI(http).missing(VERSION, self.dist, self.packages), [])
        self.assertFalse(release.Npm(http).missing(VERSION, self.dist, self.packages))

    def test_registry_files_that_differ_from_the_tested_packages_fail(self):
        foreign = f'status_machina-{VERSION}-py3-none-any.whl'
        for label, responses in [('changed wheel', self.pypi(self.packages.python, mutate=True)),
                                 ('foreign file', self.pypi([foreign]))]:
            with self.subTest(label), self.assertRaises(release.ReleaseError):
                release.PyPI(FakeHttp(responses)).missing(VERSION, self.dist, self.packages)
        with self.assertRaises(release.ReleaseError):
            release.Npm(FakeHttp(self.npm(b'other'))).missing(VERSION, self.dist, self.packages)


class VerificationTests(Fixture):
    def test_public_pypi_must_hold_exactly_the_tested_wheels_and_sdist(self):
        downloads = {f'https://files/{name}': self.digest(name) for name in self.packages.python}
        release.PyPI(FakeHttp(self.pypi(self.packages.python), downloads)).verify(VERSION, self.dist, self.packages)
        without_sdist = [name for name in self.packages.python if not name.endswith('.tar.gz')]
        with self.assertRaises(release.NotVisible):
            release.PyPI(FakeHttp(self.pypi(without_sdist), downloads)).verify(VERSION, self.dist, self.packages)
        changed = {**downloads, f'https://files/{self.packages.sdist}': '0' * 64}
        with self.assertRaises(release.ReleaseError):
            release.PyPI(FakeHttp(self.pypi(self.packages.python), changed)).verify(VERSION, self.dist, self.packages)

    def test_visibility_is_polled_only_for_absence_and_bounded(self):
        clock = iter([0, 1, 2, 400])
        calls = []

        def check():
            calls.append(1)
            if len(calls) < 3:
                raise release.NotVisible('indexing')
        release.await_visibility(check, 300, clock=lambda: next(clock), sleep=lambda _: None)
        self.assertEqual(len(calls), 3)
        with self.assertRaises(release.ReleaseError):
            release.await_visibility(lambda: (_ for _ in ()).throw(release.NotVisible('never')), 300,
                                     clock=iter([0, 400]).__next__, sleep=lambda _: None)


class TrustTests(unittest.TestCase):
    ENVIRONMENT = {'ACTIONS_ID_TOKEN_REQUEST_URL': 'https://oidc/?api-version=2.0',
                   'ACTIONS_ID_TOKEN_REQUEST_TOKEN': 'bearer-token'}

    def responses(self, pypi, npm):
        return {('GET', 'https://oidc/?api-version=2.0'): {'value': 'id-token'},
                ('GET', 'https://pypi.org/_/oidc/audience'): {'audience': 'pypi'},
                ('POST', 'https://pypi.org/_/oidc/mint-token'): pypi,
                ('POST', 'https://registry.npmjs.org/-/npm/v1/oidc/token/exchange/package/%40legra-ai%2Fstatus-machina'): npm}

    def test_configured_trust_passes_for_both_registries(self):
        http = FakeHttp(self.responses({'token': 'pypi'}, {'token': 'npm'}))
        oidc = release.GitHubOidc(http, self.ENVIRONMENT)
        release.PyPI(http).trust(oidc)
        release.Npm(http).trust(oidc)
        audiences = [url.split('&audience=')[1] for method, url, *_ in http.requests if url.startswith('https://oidc/')]
        self.assertEqual(audiences, ['pypi', 'npm%3Aregistry.npmjs.org'])
        exchange = [headers for method, url, body, headers in http.requests if 'exchange' in url]
        self.assertEqual(exchange, [{'Authorization': 'Bearer id-token'}])

    def test_missing_trust_or_identity_fails_loudly(self):
        rejected = release.ReleaseError('HTTP 422 invalid-publisher')
        for pypi, npm in [(rejected, {'token': 'npm'}), ({'token': 'pypi'}, release.ReleaseError('HTTP 404')),
                          ({'success': False}, {'token': 'npm'}), ({'token': 'pypi'}, {})]:
            with self.subTest(pypi=pypi, npm=npm), self.assertRaises(release.ReleaseError):
                http = FakeHttp(self.responses(pypi, npm))
                oidc = release.GitHubOidc(http, self.ENVIRONMENT)
                release.PyPI(http).trust(oidc)
                release.Npm(http).trust(oidc)
        with self.assertRaises(release.ReleaseError):
            release.GitHubOidc(FakeHttp(), {}).token('pypi')


class WorkflowTests(unittest.TestCase):
    def test_declared_wheel_tags_are_the_built_matrix(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        body = workflow.split('\n  python-wheels:\n', 1)[1].split('\n  python-sdist:\n', 1)[0]
        self.assertEqual(set(re.findall(r'^            tag: (\S+)$', body, flags=re.MULTILINE)), release.WHEEL_TAGS)

    def test_registry_identities_match_the_package_manifests(self):
        import tomllib
        self.assertEqual(tomllib.loads((ROOT / 'python/pyproject.toml').read_text())['project']['name'],
                         release.PYPI_PROJECT)
        self.assertEqual(json.loads((ROOT / 'npm/package.json').read_text())['name'], release.NPM_PACKAGE)


if __name__ == '__main__':
    unittest.main()
