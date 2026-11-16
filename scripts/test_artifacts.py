"""Artifact verifiers must reject every artifact that differs from its contract."""

import importlib.util
import io
import json
import platform
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / 'scripts'))


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


glibc = load('verify_glibc')
macos = load('verify_macos_minimum')
contents = load('package_contents')
versions = load('sync_binding_versions')
VERSION = versions.crate_version(ROOT)


class GlibcCeilingTests(unittest.TestCase):
    def test_rejects_symbols_newer_than_the_ceiling(self):
        with self.assertRaisesRegex(ValueError, '2.34.*2.28'):
            glibc.require_glibc(iter(['symbol GLIBC_2.17', 'symbol GLIBC_2.34']), (2, 28))

    def test_compares_versions_numerically_and_per_symbol(self):
        self.assertEqual(glibc.require_glibc(iter(['GLIBC_2.9', 'GLIBC_2.28']), (2, 28)), (2, 28))
        with self.assertRaisesRegex(ValueError, '2.29'):
            glibc.require_glibc(iter(['GLIBC_2.2.5 GLIBC_2.29']), (2, 28))
        with self.assertRaisesRegex(ValueError, '2.28.1'):
            glibc.require_glibc(iter(['GLIBC_2.28.1']), (2, 28))

    def test_rejects_binaries_without_glibc_symbols(self):
        with self.assertRaisesRegex(ValueError, 'No GLIBC'):
            glibc.require_glibc(iter(['not an ELF file']), (2, 28))


class MacosMinimumTests(unittest.TestCase):
    BUILD = ['Load command 9\n', '      cmd LC_BUILD_VERSION\n', '  cmdsize 32\n', ' platform 1\n',
             '    minos 26.0\n', '      sdk 26.2\n']

    def test_accepts_exactly_the_tested_minimum(self):
        self.assertEqual(macos.require_minimum(iter(self.BUILD), '26.0'), (26, 0, 0))

    def test_rejects_an_older_or_newer_declared_minimum(self):
        older = [line.replace('26.0', '11.0') for line in self.BUILD]
        with self.assertRaisesRegex(ValueError, '11.0.0 differs from the tested minimum 26.0'):
            macos.require_minimum(iter(older), '26.0')
        with self.assertRaisesRegex(ValueError, 'differs'):
            macos.require_minimum(iter(self.BUILD), '27.0')

    def test_rejects_missing_or_ambiguous_deployment_targets(self):
        with self.assertRaisesRegex(ValueError, 'none'):
            macos.minimum_version(iter(['      cmd LC_SEGMENT_64\n']))
        legacy = ['      cmd LC_VERSION_MIN_MACOSX\n', '  version 10.13\n']
        with self.assertRaisesRegex(ValueError, 'one macOS deployment target'):
            macos.minimum_version(iter(self.BUILD + legacy))

    def test_wheels_are_checked_through_their_native_members(self):
        with tempfile.TemporaryDirectory() as directory:
            wheel = Path(directory) / 'pure.whl'
            with zipfile.ZipFile(wheel, 'w') as archive:
                archive.writestr('status_machina/__init__.py', '')
            with self.assertRaisesRegex(ValueError, 'no native module'):
                with macos.native_binaries([wheel]):
                    pass


class ZigArgumentTests(unittest.TestCase):
    def test_removes_only_the_unsupported_gnu_linker_optimization(self):
        zig = load('zig_cc')
        self.assertEqual(zig.translate(['cc', '-O3', '-Wl,-O1', '-Wl,--gc-sections', 'main.o']),
                         ['cc', '-O3', '-Wl,--gc-sections', 'main.o'])
        self.assertEqual(zig.translate(['ar', 'rcs', 'lib.a', '-Wl,-O1']), ['ar', 'rcs', 'lib.a', '-Wl,-O1'])
        self.assertEqual(zig.translate(['cc', '-Wl,--fix-cortex-a53-843419', '-Wl,--as-needed']),
                         ['cc', '-Wl,--as-needed'])



@unittest.skipUnless(shutil.which('zig') and sys.platform == 'linux', 'Zig is installed in the Linux artifact jobs')
class ZigLinkerTests(unittest.TestCase):
    ADAPTER = ROOT / 'scripts/zig_cc.py'

    def test_links_against_the_declared_glibc_ceiling(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'main.c'
            source.write_text('#include <stdio.h>\nint main(void) { puts("ok"); return 0; }\n')
            binary = Path(directory) / 'main'
            target = f'{platform.machine()}-linux-gnu.2.28'
            result = subprocess.run([str(self.ADAPTER), 'cc', '-target', target, '-Wl,-O1',
                                     '-Wl,--fix-cortex-a53-843419',
                                     str(source), '-o', str(binary)], text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            highest = glibc.require_glibc(glibc.symbol_lines(binary), (2, 28))
            self.assertLessEqual(highest, (2, 28))

    def test_real_compiler_errors_still_fail(self):
        result = subprocess.run([str(self.ADAPTER), 'cc', '-target', 'invalid-target'],
                                text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)


class Fixture:
    """Build the exact artifacts the checkout's contracts describe, then perturb them."""

    def __init__(self, directory):
        self.directory = Path(directory)
        self.checkout = contents.Checkout(ROOT)
        self.python = versions.python_version(VERSION)

    def wheel(self, tag='cp310-abi3-win_amd64', drop=(), add=None, metadata=None):
        dist = f'status_machina-{self.python}'
        path = self.directory / f'{dist}-{tag}.whl'
        members = {name: (ROOT / 'python' / name).read_bytes() for name in contents.python_sources(self.checkout)}
        members['status_machina/_native.pyd' if '-win_' in tag else 'status_machina/_native.abi3.so'] = b'native'
        for name in contents.LICENSES:
            members[f'{dist}.dist-info/licenses/licenses/{name}'] = (ROOT / name).read_bytes()
        members[f'{dist}.dist-info/METADATA'] = (metadata or self.metadata()).encode()
        members[f'{dist}.dist-info/WHEEL'] = f'Wheel-Version: 1.0\nRoot-Is-Purelib: false\nTag: {tag}\n'.encode()
        members[f'{dist}.dist-info/RECORD'] = b''
        members.update(add or {})
        with zipfile.ZipFile(path, 'w') as archive:
            for name, data in members.items():
                if name not in drop:
                    archive.writestr(name, data)
        return path

    def metadata(self, version=None):
        return ('Metadata-Version: 2.4\nName: status-machina\n'
                f'Version: {version or self.python}\nRequires-Dist: typing-extensions>=4.12\n'
                'License-File: licenses/LICENSE-MIT\nLicense-File: licenses/LICENSE-APACHE\n'
                'License-Expression: MIT OR Apache-2.0\nRequires-Python: >=3.10\n\n')

    def sdist(self, drop=(), add=None):
        top = f'status_machina-{self.python}'
        members = {name: (ROOT / name).read_bytes() for name in
                   self.checkout.tracked('src', 'tests', 'bindings/core', 'python/status-machina-pyo3')}
        members.update({name: (ROOT / 'python' / name).read_bytes()
                        for name in contents.python_sources(self.checkout)})
        for name in ('pyproject.toml', 'Cargo.toml', 'Cargo.lock', 'README.md', 'build.rs'):
            members[name] = b''
        for name in contents.LICENSES:
            members[name] = members[f'licenses/{name}'] = (ROOT / name).read_bytes()
        members['PKG-INFO'] = self.metadata().encode()
        members.update(add or {})
        return self._tar(self.directory / f'{top}.tar.gz', top, members, drop)

    def npm(self, drop=(), add=None, manifest=None):
        package = {name.removeprefix('npm/'): (ROOT / name).read_bytes() for name in self.checkout.tracked('npm/lib')}
        for name in ('README.md', 'index.mjs', 'index.cjs', 'index.d.ts'):
            package[name] = (ROOT / 'npm' / name).read_bytes()
        for name in contents.LICENSES:
            package[name] = (ROOT / name).read_bytes()
        package.update({name: b'addon' for name in contents.NpmTarball(self.checkout, None).addons()})
        package['package.json'] = json.dumps(manifest or self.checkout.npm_manifest()).encode()
        package.update(add or {})
        return self._tar(self.directory / f'legra-ai-status-machina-{VERSION}.tgz', 'package', package, drop)

    @staticmethod
    def _tar(path, top, members, drop):
        with tarfile.open(path, 'w:gz') as archive:
            for name, data in members.items():
                if name in drop:
                    continue
                info = tarfile.TarInfo(f'{top}/{name}')
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        return path


class PackageContentsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.fixture = Fixture(self.temporary.name)
        self.checkout = self.fixture.checkout

    def tearDown(self):
        self.temporary.cleanup()

    def wheel(self, **changes):
        tag = changes.pop('tag', 'cp310-abi3-win_amd64')
        return contents.Wheel(self.checkout, self.fixture.wheel(tag=tag, **changes), tag)

    def test_complete_wheels_pass_for_every_native_module_kind(self):
        for tag in ('cp310-abi3-win_arm64', 'cp310-abi3-manylinux_2_28_aarch64', 'cp310-abi3-macosx_26_0_arm64'):
            with self.subTest(tag=tag):
                self.wheel(tag=tag).check(VERSION)

    def test_wheels_reject_missing_modules_licenses_and_stray_files(self):
        dist = f'status_machina-{self.fixture.python}.dist-info'
        for changes in [{'drop': ['status_machina/py.typed']}, {'drop': ['status_machina/_native.pyd']},
                        {'drop': [f'{dist}/licenses/licenses/LICENSE-MIT']},
                        {'add': {'status_machina/__pycache__/model.cpython-314.pyc': b''}},
                        {'add': {f'{dist}/licenses/licenses/LICENSE-MIT': b'changed'}},
                        {'metadata': self.fixture.metadata(version='0.0.1')}]:
            with self.subTest(changes=changes), self.assertRaises(contents.ContentsError):
                self.wheel(**changes).check(VERSION)

    def test_wheels_reject_a_tag_other_than_the_declared_target(self):
        artifact = self.fixture.wheel(tag='cp310-abi3-manylinux_2_39_x86_64')
        with self.assertRaisesRegex(contents.ContentsError, 'manylinux_2_28'):
            contents.Wheel(self.checkout, artifact, 'cp310-abi3-manylinux_2_28_x86_64').check(VERSION)
        with self.assertRaises(contents.ContentsError):
            self.wheel(tag='cp312-cp312-win_amd64').check(VERSION)

    def test_source_distribution_requires_every_build_source(self):
        contents.SourceDistribution(self.checkout, self.fixture.sdist()).check(VERSION)
        for changes in [{'drop': ['Cargo.lock']}, {'drop': ['src/lib.rs']}, {'drop': ['licenses/LICENSE-APACHE']},
                        {'drop': ['python/status-machina-pyo3/build.rs']}, {'drop': ['status_machina/py.typed']},
                        {'add': {'status_machina/_native.abi3.so': b''}}, {'add': {'.git/config': b''}},
                        {'add': {'npm/status_machina.linux-x64-gnu.node': b''}},
                        {'add': {'target/release/lib_native.so': b''}}, {'add': {'LICENSE-MIT': b'changed'}},
                        {'add': {'PKG-INFO': self.fixture.metadata(version='0.0.1').encode()}}]:
            with self.subTest(changes=changes), self.assertRaises(contents.ContentsError):
                contents.SourceDistribution(self.checkout, self.fixture.sdist(**changes)).check(VERSION)

    def test_npm_tarball_is_exact(self):
        contents.NpmTarball(self.checkout, self.fixture.npm()).check(VERSION)
        manifest = self.checkout.npm_manifest()
        for changes in [{'drop': ['status_machina.win32-arm64-msvc.node']}, {'drop': ['LICENSE-APACHE']},
                        {'drop': ['lib/native.cjs']}, {'add': {'status_machina.darwin-x64.node': b''}},
                        {'add': {'tests/workers.test.mjs': b''}}, {'add': {'LICENSE-MIT': b'changed'}},
                        {'manifest': {**manifest, 'version': '0.0.1'}},
                        {'manifest': {**manifest, 'engines': {'node': '>=22'}}},
                        {'manifest': {**manifest, 'scripts': {'postinstall': 'napi build'}}}]:
            with self.subTest(changes=changes), self.assertRaises(contents.ContentsError):
                contents.NpmTarball(self.checkout, self.fixture.npm(**changes)).check(VERSION)

    def test_every_declared_napi_target_has_an_addon_name(self):
        self.assertEqual(len(contents.NpmTarball(self.checkout, None).addons()), 5)


class VersionSynchronizationTests(unittest.TestCase):
    def copy(self, directory):
        root = Path(directory)
        for name in ['Cargo.toml', *versions.FILES]:
            (root / name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, root / name)
        return root

    def test_every_binding_manifest_matches_the_crate(self):
        self.assertEqual(versions.mismatches(ROOT, VERSION), [])

    def test_release_bump_rewrites_every_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.copy(directory)
            cargo = (root / 'Cargo.toml').read_text()
            (root / 'Cargo.toml').write_text(cargo.replace(f'version = "{VERSION}"', 'version = "9.8.7-rc.2"', 1))
            versions.synchronize(root, '9.8.7-rc.2')
            self.assertEqual(versions.mismatches(root, '9.8.7-rc.2'), [])
            self.assertIn('version = "9.8.7rc2"', (root / 'python/pyproject.toml').read_text())
            self.assertEqual(json.loads((root / 'npm/package.json').read_text())['version'], '9.8.7-rc.2')
            self.assertEqual(versions.main(['--check', '--root', str(root)]), 0)

    def test_rejects_a_version_other_than_the_crate(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.copy(directory)
            with self.assertRaisesRegex(ValueError, 'differs from the core crate'):
                versions.synchronize(root, '9.9.9')
            with self.assertRaisesRegex(ValueError, 'Unsupported'):
                versions.synchronize(root, 'v1')

    def test_release_preparation_runs_the_hook(self):
        manifest = (ROOT / 'Cargo.toml').read_text()
        self.assertIn('pre_bump_hooks = ["python3 scripts/sync_binding_versions.py {{version}}"]', manifest)
        for name in versions.FILES:
            self.assertIn(f'"{name}"', manifest.split('additional_files = [', 1)[1].split(']', 1)[0])


if __name__ == '__main__':
    unittest.main()
