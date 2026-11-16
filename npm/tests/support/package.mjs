// Locate the package under test by its name, never by a path into the checkout:
// in the repository the name resolves to this package itself (self-reference),
// and in the installed-artifact suites it resolves to the installed npm tarball.
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';

const require = createRequire(import.meta.url);

/** Root directory of the resolved `@legra-ai/status-machina` package. */
export const packageRoot = dirname(require.resolve('@legra-ai/status-machina'));

/** Load one of the package's private CommonJS modules for white-box tests. */
export function requireInternal(path) {
  return require(join(packageRoot, path));
}
