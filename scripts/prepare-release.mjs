import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export function setVersion(content, version) {
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Invalid semantic version: ${version}`);
  }
  if (!/^version = "[^"]+"$/m.test(content)) throw new Error('Cargo package version not found');
  return content.replace(/^version = "[^"]+"$/m, `version = "${version}"`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const version = process.argv[2];
  writeFileSync('Cargo.toml', setVersion(readFileSync('Cargo.toml', 'utf8'), version));
  if (existsSync('manifest.json')) {
    const plugin = JSON.parse(readFileSync('manifest.json', 'utf8'));
    plugin.version = version;
    writeFileSync('manifest.json', JSON.stringify(plugin, null, 2) + '\n');
  }
  const run = (command, args) => execFileSync(command, args, { stdio: 'inherit' });
  run('cargo', ['generate-lockfile']);
  for (const arch of ['x86_64', 'aarch64']) {
    run('make', ['build-tar', 'build-deb', `ARCH=${arch}`]);
  }
  run('make', ['update-aur']);
  run('make', ['test-packaging']);
}
