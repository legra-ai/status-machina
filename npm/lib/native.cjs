'use strict';
const { join } = require('node:path');
if (Number(process.versions.node.split('.')[0]) < 22) throw new Error('Status Machina requires Node.js 22 or newer');
const platform = process.platform;
const arch = process.arch;
let suffix;
if (platform === 'darwin' && (arch === 'x64' || arch === 'arm64')) suffix = `darwin-${arch}`;
else if (platform === 'win32' && (arch === 'x64' || arch === 'arm64')) suffix = `win32-${arch}-msvc`;
else if (platform === 'linux' && (arch === 'x64' || arch === 'arm64')) {
  if (process.report.getReport().header.glibcVersionRuntime === undefined) throw new Error('This Status Machina package supports glibc Linux; no musl artifact is declared');
  suffix = `linux-${arch}-gnu`;
} else throw new Error(`Unsupported Status Machina platform: ${platform}/${arch}`);
// Audited module-loading boundary: Node's native addon loader is synchronous.
// No application filesystem operations or source-build fallback are performed.
module.exports = require(join(__dirname, '..', `status_machina.${suffix}.node`));
