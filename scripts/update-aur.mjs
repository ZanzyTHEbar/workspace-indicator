import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

function replaceLine(text, pattern, value) {
  if ([...text.matchAll(pattern)].length !== 1) throw new Error(`Expected one field: ${pattern}`);
  return text.replace(pattern, () => value);
}

export function updateAur(pkgbuild, srcinfo, version, archives) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error('Expected a stable semantic version');
  const name = pkgbuild.match(/^pkgname=([\w-]+)$/m)?.[1];
  const url = pkgbuild.match(/^url="(https:\/\/[^"\s]+)"$/m)?.[1];
  if (!name || !url) throw new Error('Missing AUR package name or URL');
  pkgbuild = replaceLine(pkgbuild, /^pkgver=.*$/gm, `pkgver=${version}`);
  pkgbuild = replaceLine(pkgbuild, /^pkgrel=.*$/gm, 'pkgrel=1');
  srcinfo = replaceLine(srcinfo, /^\tpkgver = .*$/gm, `\tpkgver = ${version}`);
  srcinfo = replaceLine(srcinfo, /^\tpkgrel = .*$/gm, '\tpkgrel = 1');
  for (const arch of ['x86_64', 'aarch64']) {
    const bytes = archives[arch];
    if (!Buffer.isBuffer(bytes) || bytes.length === 0) throw new Error(`Missing archive: ${arch}`);
    const hash = createHash('sha256').update(bytes).digest('hex');
    pkgbuild = replaceLine(pkgbuild, new RegExp(`^sha256sums_${arch}=.*$`, 'gm'), `sha256sums_${arch}=('${hash}')`);
    srcinfo = replaceLine(srcinfo, new RegExp(`^\\tsha256sums_${arch} = .*$`, 'gm'), `\tsha256sums_${arch} = ${hash}`);
    srcinfo = replaceLine(srcinfo, new RegExp(`^\\tsource_${arch} = .*$`, 'gm'),
      `\tsource_${arch} = ${name}-${version}-${arch}.tar.gz::${url}/releases/download/v${version}/workspace-indicator-linux-${arch}.tar.gz`);
  }
  return { pkgbuild, srcinfo };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const directory = process.argv[3] ?? '.';
  const archives = Object.fromEntries(['x86_64', 'aarch64'].map(arch =>
    [arch, readFileSync(join(directory, `workspace-indicator-linux-${arch}.tar.gz`))]));
  const result = updateAur(readFileSync('packaging/aur/PKGBUILD', 'utf8'),
    readFileSync('packaging/aur/.SRCINFO', 'utf8'), process.argv[2], archives);
  writeFileSync('packaging/aur/PKGBUILD', result.pkgbuild);
  writeFileSync('packaging/aur/.SRCINFO', result.srcinfo);
}
