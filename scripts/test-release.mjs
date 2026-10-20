import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { setVersion } from './prepare-release.mjs';
import { updateAur } from './update-aur.mjs';
import { createHash } from 'node:crypto';

assert.equal(setVersion('[package]\nversion = "0.4.0"\n', '1.2.3'), '[package]\nversion = "1.2.3"\n');
for (const bad of ['', 'v1.2.3', '1.2; touch /tmp/bad']) {
  assert.throws(() => setVersion('version = "0.4.0"', bad));
}
assert.throws(() => setVersion('[package]\nname = "example"', '1.2.3'));
const config = JSON.parse(readFileSync('.releaserc.json', 'utf8'));
assert.deepEqual(config.branches, ['main']);
assert.equal(config.plugins[0][0], '@semantic-release/commit-analyzer');
assert.ok(config.plugins[0][1].releaseRules.some(rule =>
  rule.type === 'build' && rule.scope === 'debian' && rule.release === 'patch'));
assert.ok(!JSON.stringify(config).includes('@semantic-release/npm'));
assert.equal(config.plugins.find(plugin => Array.isArray(plugin) && plugin[0] === '@semantic-release/exec')[1].prepareCmd,
  'make prepare-release RELEASE_VERSION=${nextRelease.version}');
const archives = { x86_64: Buffer.from('x86 release'), aarch64: Buffer.from('ARM release') };
const updated = updateAur(readFileSync('packaging/aur/PKGBUILD', 'utf8'),
  readFileSync('packaging/aur/.SRCINFO', 'utf8'), '9.8.7', archives);
assert.match(updated.pkgbuild, /^pkgver=9\.8\.7$/m);
assert.match(updated.srcinfo, /^\tpkgver = 9\.8\.7$/m);
for (const arch of Object.keys(archives)) {
  const hash = createHash('sha256').update(archives[arch]).digest('hex');
  assert.ok(updated.pkgbuild.includes(`sha256sums_${arch}=('${hash}')`));
  assert.ok(updated.srcinfo.includes(`sha256sums_${arch} = ${hash}`));
  assert.ok(updated.srcinfo.includes(`workspace-indicator-bin-9.8.7-${arch}.tar.gz::`));
  assert.ok(updated.srcinfo.includes(`/releases/download/v9.8.7/workspace-indicator-linux-${arch}.tar.gz`));
}
assert.throws(() => updateAur(updated.pkgbuild, updated.srcinfo, 'latest', archives));
assert.throws(() => updateAur(updated.pkgbuild, updated.srcinfo, '9.8.7', { x86_64: archives.x86_64 }));
assert.ok(config.plugins.find(plugin => Array.isArray(plugin) && plugin[0] === '@semantic-release/git')[1].assets.includes('packaging/aur/PKGBUILD'));
assert.ok(config.plugins.find(plugin => Array.isArray(plugin) && plugin[0] === '@semantic-release/git')[1].assets.includes('packaging/aur/.SRCINFO'));
console.log('release checks passed');
