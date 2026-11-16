"""The complete maintenance path must be wired, pinned and credential-free on PRs."""
from pathlib import Path
import json
import os
import subprocess
import sys
import textwrap
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
# Every native build, packaging and installed-artifact job; each gates publication.
NATIVE_JOBS = ('python-bindings', 'node-bindings', 'python-wheels', 'python-sdist', 'node-addons',
               'npm-package', 'python-installed', 'node-installed')
RUNNERS = '[ubuntu-24.04, ubuntu-24.04-arm, macos-latest, windows-latest, windows-11-arm]'


def job_body(workflow, job):
    return re.split(r'^  [a-z-]+:\n', workflow.split(f'\n  {job}:\n', 1)[1], maxsplit=1, flags=re.MULTILINE)[0]


class MaintenancePolicyTests(unittest.TestCase):
    def test_status_machina_uses_authorized_patch_only_releases(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn('      release-bump: patch', workflow)
        readme = (ROOT / 'README.md').read_text()
        self.assertIn('patch-only', readme)
        self.assertIn('breaking-change markers', readme)

    def test_shared_policy_updates_are_grouped_without_semver_filters(self):
        config = (ROOT / '.github/dependabot.yml').read_text()
        actions = config.split('  - package-ecosystem: github-actions\n', 1)[1]
        self.assertIn('      shared-policy:\n', actions)
        self.assertLess(actions.index('      shared-policy:\n'),
                        actions.index('      compatible-updates:\n'))
        group = actions.split('      shared-policy:\n', 1)[1].split('      compatible-updates:\n', 1)[0]
        self.assertIn('patterns: ["legra-ai/github-actions*"]', group)
        self.assertIn('applies-to: version-updates', group)
        self.assertNotIn('update-types:', group)

    def test_completion_gate_rejects_every_incomplete_pipeline_result(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn('  test:\n', workflow)
        gate = workflow.split('  test:\n', 1)[1]
        self.assertIn('if: always()', gate)
        jobs = {f'{job.upper().replace("-", "_")}_RESULT': job for job in ('prepare', 'ci', *NATIVE_JOBS)}
        needs = re.search(r'^    needs: \[([^\]]*)\]\n', gate, flags=re.MULTILINE)
        self.assertEqual({name.strip() for name in needs.group(1).split(',')}, set(jobs.values()))
        for variable, job in jobs.items():
            self.assertIn(f'{variable}: ${{{{ needs.{job}.result }}}}', gate)
        script = textwrap.dedent(gate.split("python3 - <<'PYTHON'\n", 1)[1].split('          PYTHON', 1)[0])
        passing = dict.fromkeys(jobs, 'success')
        cases = [passing] + [{**passing, variable: outcome} for variable in jobs
                             for outcome in ('failure', 'cancelled', 'skipped')]
        for results in cases:
            with self.subTest(**results):
                result = subprocess.run([sys.executable, '-c', script], capture_output=True,
                                        env={**os.environ, **results})
                self.assertEqual(result.returncode == 0, results == passing)

    def test_release_path_waits_for_every_build_job(self):
        # Either all builds are ok or nothing gets published: the shared release
        # pipeline must start only after every native build passed for this revision.
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        jobs = {match.group(1): match.start()
                for match in re.finditer(r'^  ([a-z][a-z-]*):\n', workflow, flags=re.MULTILINE)}
        bodies = {}
        ordered = sorted(jobs.items(), key=lambda item: item[1])
        for (name, start), following in zip(ordered, ordered[1:] + [(None, len(workflow))]):
            bodies[name] = workflow[start:following[1]]
        release = [name for name, body in bodies.items() if 'cargo-maintenance.yml@' in body]
        self.assertEqual(release, ['ci'])
        shared = bodies['ci']
        needs = re.search(r'^    needs: \[([^\]]*)\]\n', shared, flags=re.MULTILINE)
        self.assertIsNotNone(needs, 'the release call must declare its build dependencies')
        required = {name.strip() for name in needs.group(1).split(',')}
        builds = {name for name, body in bodies.items()
                  if name not in ('ci', 'test') and re.search(r'^    steps:\n', body, flags=re.MULTILINE)}
        self.assertEqual(builds, {'prepare', *NATIVE_JOBS})
        self.assertEqual(required, builds)
        # always() or a result filter would let a failed, cancelled or skipped build through.
        condition = re.search(r'^    if: (.*)\n', shared, flags=re.MULTILINE).group(1)
        self.assertNotIn('always()', condition)
        self.assertNotIn('needs.', condition)
        self.assertNotIn('continue-on-error', workflow)
        for job in NATIVE_JOBS:
            self.assertNotRegex(bodies[job], r'^    if:', f'{job} must run on every event')

    def test_native_bindings_are_built_and_tested_on_every_runner(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        for job, commands in [
            ('python-bindings', ['cargo test --locked -p status-machina-bridge -p status-machina-pyo3',
                                 'uv sync --locked', 'uv run --locked maturin develop',
                                 'uv run --locked python -m unittest discover -s tests -v',
                                 'uv run --locked mypy']),
            ('node-bindings', ['cargo test --locked -p status-machina-napi',
                               'pnpm install --frozen-lockfile', 'pnpm build', 'pnpm test',
                               'pnpm typecheck']),
        ]:
            with self.subTest(job=job):
                body = re.split(r'^  [a-z-]+:\n', workflow.split(f'\n  {job}:\n', 1)[1],
                                maxsplit=1, flags=re.MULTILINE)[0]
                self.assertIn('os: [ubuntu-24.04, macos-latest, windows-latest]', body)
                for command in commands:
                    self.assertIn(f'run: {command}\n', body)

    def test_maturin_develop_uses_the_locked_uv_installer(self):
        project = tomllib.loads((ROOT / 'python/pyproject.toml').read_text())
        # maturin develop prefers the environment's uv module over an older uv on PATH,
        # whose `uv pip install` lacks the --group option that maturin passes.
        self.assertIn('uv>=0.6.7', project['dependency-groups']['dev'])
        self.assertTrue(project['tool']['mypy']['warn_unused_ignores'])

    def test_dependency_merger_has_a_separate_identity_and_requires_success(self):
        merge = (ROOT / '.github/workflows/dependabot-merge.yml').read_text()
        self.assertIn("if: github.event.workflow_run.conclusion == 'success'", merge)
        self.assertIn('secrets.RELEASE_MERGER_APP_ID', merge)
        self.assertIn('secrets.RELEASE_MERGER_APP_KEY', merge)
        self.assertNotIn('RELEASE_WRITER', merge)
        action = merge.split('uses: legra-ai/github-actions/.github/actions/merge-tested-dependabot@', 1)[1]
        self.assertIn('app-id: ${{ secrets.RELEASE_MERGER_APP_ID }}', action)
        self.assertIn('token: ${{ steps.merger.outputs.token }}', action)

    def test_private_main_cannot_activate_publication(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        shared = workflow.split('  ci:', 1)[1].split('  test:', 1)[0]
        self.assertIn("if: github.event.repository.private == false || github.event_name == 'pull_request'", shared)

    def test_one_pinned_release_pipeline(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertRegex(workflow, r'cargo-maintenance.yml@[0-9a-f]{40}')
        self.assertNotIn('policy-revision:', workflow)
        self.assertIn('schedule:', workflow)
        self.assertIn('workflow_dispatch:', workflow)
        for path in (ROOT / '.github/workflows').glob('*.yml'):
            self.assertNotRegex(path.read_text(), r'cargo-auto-version-bump.yml|cargo-crate-ci.yml|--auto --rebase')

    def test_shared_actions_use_one_revision(self):
        # The shared pipeline publishes only from the version commit's own run;
        # mixed pins could pair a newer action with an older release workflow.
        pins = set()
        for path in (ROOT / '.github/workflows').glob('*.yml'):
            pins.update(re.findall(r'legra-ai/github-actions/\S+@([0-9a-f]{40})', path.read_text()))
        self.assertEqual(len(pins), 1, pins)

    def test_daily_scoped_maintenance(self):
        config = (ROOT / '.github/dependabot.yml').read_text()
        self.assertEqual(config.count('interval: daily'), 2)
        self.assertNotIn('include: scope', config)
        self.assertIn('prefix: "build(deps)"', config)
        self.assertIn('security-updates:', config)
        self.assertIn('dependency-type: all', config)
        merge = (ROOT / '.github/workflows/dependabot-merge.yml').read_text()
        self.assertIn('merge-tested-dependabot@', merge)
        self.assertIn('actions/create-github-app-token@', merge)

    def test_compiler_and_formatter_pins_match_hooks(self):
        package = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']
        compiler = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['channel']
        self.assertEqual(compiler, package['rust-version'])
        self.assertRegex((ROOT / 'rustfmt-toolchain').read_text().strip(), r'^nightly-\d{4}-\d{2}-\d{2}$')
        hook = (ROOT / '.githooks/pre-commit').read_text()
        self.assertIn('scripts/fmt.sh --check', hook)
        self.assertNotIn('+nightly ', hook)

    def test_windows_checkout_preserves_shell_and_toolchain_lf(self):
        self.assertIn('* text=auto eol=lf', (ROOT / '.gitattributes').read_text())

    def test_package_contains_integration_tests_and_licenses(self):
        package = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']
        if (ROOT / 'build.rs').exists():
            self.assertIn('/build.rs', package['include'])
        self.assertIn('tests/**', package['include'])
        # Anchored, so the binding packages' license copies stay out of the crate.
        self.assertIn('/LICENSE-MIT', package['include'])
        self.assertIn('/LICENSE-APACHE', package['include'])
        self.assertTrue(any((ROOT / 'tests').glob('*.rs')))


class NativeArtifactTests(unittest.TestCase):
    def setUp(self):
        self.workflow = (ROOT / '.github/workflows/ci.yml').read_text()

    def test_every_declared_target_is_built_on_its_own_runner(self):
        targets = json.loads((ROOT / 'npm/package.json').read_text())['napi']['targets']
        runners = {'x86_64-unknown-linux-gnu': 'ubuntu-24.04', 'aarch64-unknown-linux-gnu': 'ubuntu-24.04-arm',
                   'aarch64-apple-darwin': 'macos-latest', 'x86_64-pc-windows-msvc': 'windows-latest',
                   'aarch64-pc-windows-msvc': 'windows-11-arm'}
        self.assertEqual(set(targets), set(runners))
        for job in ('python-wheels', 'node-addons'):
            body = job_body(self.workflow, job)
            built = dict(re.findall(r'- target: (\S+)\n\s+os: (\S+)\n', body))
            self.assertEqual(built, runners, job)
            self.assertIn("MACOSX_DEPLOYMENT_TARGET: '26.0'", body)
            self.assertIn('verify_glibc.py --maximum 2.28', body)
            self.assertIn('verify_macos_minimum.py --minimum 26.0', body)
            self.assertIn('if-no-files-found: error', body)
        self.assertIn('--zig --compatibility manylinux_2_28', job_body(self.workflow, 'python-wheels'))
        self.assertIn('scripts/zig_cc.py" cc -target %s.2.28', job_body(self.workflow, 'node-addons'))

    def test_artifact_contents_are_checked_before_upload(self):
        for job, command in [('python-wheels', 'package_contents.py wheel'), ('python-sdist', 'package_contents.py sdist'),
                             ('npm-package', 'package_contents.py npm')]:
            body = job_body(self.workflow, job)
            self.assertLess(body.index(command), body.index('actions/upload-artifact@'), job)

    def test_installed_suites_run_without_rust_or_package_sources(self):
        for job, suite, commands in [
            ('python-installed', '/python/tests/', ['-m unittest discover -s tests -v',
                                                    '-m mypy --warn-unused-ignores tests/typing']),
            ('node-installed', '/npm/tests/', ['node --test tests/*.test.mjs', 'pnpm exec tsc -p tests/typing']),
        ]:
            with self.subTest(job=job):
                body = job_body(self.workflow, job)
                self.assertIn(f'os: {RUNNERS}', body)
                self.assertIn('sparse-checkout-cone-mode: false', body)
                self.assertIn(suite, body)
                self.assertNotRegex(body, r'sparse-checkout: \|\n(?:\s+/\S+\n)*\s+/(?:python|npm)/(?:status_machina|lib|index)')
                self.assertIn('rustup self uninstall -y', body)
                self.assertIn('brew uninstall --force --ignore-dependencies rustup', body)
                self.assertIn('hash -r', body)
                self.assertLess(body.index('rustup self uninstall -y'), body.index(commands[0]))
                self.assertIn('Require the installed package', body)
                for command in commands:
                    self.assertIn(command, body)
        python = job_body(self.workflow, 'python-installed')
        self.assertIn("python: ['3.10', '3.11', '3.12', '3.13', '3.14']", python)
        self.assertIn("--only-binary :all:", python)
        self.assertIn("node: ['22', '24']", job_body(self.workflow, 'node-installed'))

    def test_sdist_builds_outside_git_with_hooks_enabled_and_refuses_free_threading(self):
        body = job_body(self.workflow, 'python-sdist')
        self.assertIn('env -u CI -u GITHUB_ACTIONS RUSTUP_TOOLCHAIN="$minimum"', body)
        self.assertIn('must not be inside a Git repository', body)
        self.assertIn("python-version: '3.14t'", body)
        self.assertIn('is not a supported wheel on this platform', body)
        self.assertIn('does not support free-threaded CPython', body)
        self.assertIn('#[cfg(Py_GIL_DISABLED)]\ncompile_error!(',
                      (ROOT / 'python/status-machina-pyo3/src/lib.rs').read_text())

    def test_package_license_copies_match_the_repository(self):
        project = tomllib.loads((ROOT / 'python/pyproject.toml').read_text())['project']
        self.assertEqual(project['license'], 'MIT OR Apache-2.0')
        self.assertEqual(project['license-files'], ['licenses/LICENSE-MIT', 'licenses/LICENSE-APACHE'])
        files = json.loads((ROOT / 'npm/package.json').read_text())['files']
        for name in ('LICENSE-MIT', 'LICENSE-APACHE'):
            self.assertIn(name, files)
            for copy in (ROOT / 'python/licenses' / name, ROOT / 'npm' / name):
                self.assertEqual(copy.read_bytes(), (ROOT / name).read_bytes(), copy)


class CommitMessageTests(unittest.TestCase):
    def test_breaking_scope_and_headline_validation(self):
        import os
        import subprocess
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cog = root / 'cog'
            # Isolate the scope/headline gate; cocogitto has its own type parser.
            cog.write_text('#!/bin/sh\nexit 0\n', newline='\n')
            cog.chmod(0o755)
            message = root / 'message'
            env = {**os.environ, 'PATH': str(root) + os.pathsep + os.environ['PATH']}
            for headline, body, expected in [
                ('fix(ci)!: enforce release identity', '', 0),
                ('fix(ci): enforce release identity', '', 0),
                ('build(deps): update serde', '', 0),
                ('ci(deps-dev): update action', '', 0),
                ('fix(api): repair', '\nBREAKING CHANGE: new wire shape\n', 0),
                ('fix: missing scope', '', 1),
                ('oops(api): invalid type', '', 1),
                ('bad headline', '\nfix(api): not the headline\n', 1),
                ('fix(api): ', '', 1),
                ('unscoped headline', '\nfix(ci): forged body\n', 1),
            ]:
                with self.subTest(headline=headline):
                    message.write_text(headline + '\n' + body, newline='\n')
                    # Windows also provides a WSL bash; use Git's own shell.
                    shell = (Path(subprocess.check_output(['git', '--exec-path'], text=True).strip()).parents[2]
                             / 'bin/bash.exe') if os.name == 'nt' else 'bash'
                    result = subprocess.run([str(shell), str(ROOT / '.githooks/commit-msg'), str(message)],
                                            env=env, capture_output=True, text=True)
                    self.assertEqual(result.returncode, expected, result.stdout + result.stderr)


class BuildOwnershipTests(unittest.TestCase):
    def test_hook_dependency_excludes_recursive_installers(self):
        manifest = tomllib.loads((ROOT / 'Cargo.toml').read_text())
        requirement = manifest['build-dependencies']['rhusky']
        self.assertRegex(requirement, r'^2\.[0-9]+\.[0-9]+$')
        self.assertGreaterEqual(tuple(map(int, requirement.split('.'))), (2, 0, 1))

    def test_hook_ownership_is_an_explicit_local_argument(self):
        build = (ROOT / 'build.rs').read_text()
        self.assertIn('install_from_build_script(env!("CARGO_MANIFEST_DIR"))', build)
        config = ROOT / '.cargo/config.toml'
        if config.exists():
            self.assertNotIn('RHUSKY_REPOSITORY_ROOT', config.read_text())


if __name__ == '__main__':
    unittest.main()
