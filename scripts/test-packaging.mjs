import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { build } from './build-deb.mjs';

const plugin = JSON.parse(readFileSync('manifest.json', 'utf8'));
assert.equal(plugin.schemaVersion, 1);
assert.deepEqual(plugin.kinds, ['service']);
assert.ok(existsSync(plugin.entryPoints.service));
assert.ok(['x64', 'arm64'].includes(process.arch), 'Unsupported test host architecture');
const arch = process.arch === 'arm64' ? 'arm64' : 'amd64';
const rustArch = process.arch === 'arm64' ? 'aarch64' : 'x86_64';
const binary = `target/${rustArch}-unknown-linux-musl/release/workspace-indicator`;
const tmp = mkdtempSync(join(tmpdir(), 'workspace-packaging-test-'));
try {
  const pkg = build(binary, arch, tmp);
  const control = execFileSync('ar', ['p', pkg, 'control.tar.xz']);
  const data = execFileSync('ar', ['p', pkg, 'data.tar.xz']);
  const extract = (archive, name) => execFileSync('tar', ['-xJO', name], { input: archive, maxBuffer: 16 * 1024 * 1024 });
  assert.match(extract(control, './control').toString(), new RegExp(`^Architecture: ${arch}$`, 'm'));
  assert.match(extract(data, './usr/lib/systemd/user/workspace-indicator.service').toString(), /^ExecStart=\/usr\/bin\/workspace-indicator$/m);
  assert.deepEqual(extract(data, './usr/bin/workspace-indicator'), readFileSync(binary));
  assert.deepEqual(extract(data, './usr/share/doc/workspace-indicator/copyright'), readFileSync('LICENSE'));
  const listing = execFileSync('tar', ['-tvJ', '--numeric-owner'], { input: data }).toString();
  assert.match(listing, /^-rwxr-xr-x\s+0\/0\s+.*\.\/usr\/bin\/workspace-indicator$/m);
  assert.throws(() => build(binary, arch === 'amd64' ? 'arm64' : 'amd64', tmp), /architecture/);
  assert.throws(() => build(binary, 'invalid', tmp), /Expected amd64 or arm64/);
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
console.log('packaging checks passed');
