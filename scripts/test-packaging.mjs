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
const originalEpoch = process.env.SOURCE_DATE_EPOCH;
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
  const timestampListing = archive => execFileSync('tar', ['-tvJ', '--full-time', '--utc'], { input: archive }).toString();
  const epoch = originalEpoch ?? execFileSync('git', ['log', '-1', '--format=%ct'], { encoding: 'utf8' }).trim();
  const timestamp = new Date(Number(epoch) * 1000).toISOString().slice(0, 19).replace('T', ' ');
  assert.ok(timestampListing(data).includes(timestamp), 'Payload should use the explicit epoch or source commit time');
  process.env.SOURCE_DATE_EPOCH = '0';
  const reproducible = readFileSync(build(binary, arch, tmp));
  const overriddenData = execFileSync('ar', ['p', pkg, 'data.tar.xz']);
  assert.ok(timestampListing(overriddenData).includes('1970-01-01 00:00:00'), 'Explicit zero epoch should be preserved');
  assert.deepEqual(readFileSync(build(binary, arch, tmp)), reproducible, 'Same source and epoch should produce identical packages');
  assert.throws(() => build(binary, arch === 'amd64' ? 'arm64' : 'amd64', tmp), /architecture/);
  assert.throws(() => build(binary, 'invalid', tmp), /Expected amd64 or arm64/);
} finally {
  if (originalEpoch === undefined) delete process.env.SOURCE_DATE_EPOCH;
  else process.env.SOURCE_DATE_EPOCH = originalEpoch;
  rmSync(tmp, { recursive: true, force: true });
}
console.log('packaging checks passed');
