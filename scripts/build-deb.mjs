// Use the system tar/ar tools; no npm packaging dependencies are needed.
import { execFileSync } from 'node:child_process';
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export function build(binary, arch, output = 'dist') {
  if (!['amd64', 'arm64'].includes(arch)) throw new Error('Expected amd64 or arm64');
  const bytes = readFileSync(binary);
  const machine = arch === 'amd64' ? 62 : 183;
  if (bytes.length < 20 || !bytes.subarray(0, 6).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46, 2, 1])) || bytes.readUInt16LE(18) !== machine) {
    throw new Error('Binary architecture does not match package');
  }
  const packageSection = readFileSync('Cargo.toml', 'utf8').match(/^\[package\]\s*\n([\s\S]*?)(?=^\[|$(?![\s\S]))/m)?.[1];
  const version = packageSection?.match(/^version = "([0-9A-Za-z.+-]+)"$/m)?.[1];
  if (!version) throw new Error('Cargo package version not found');
  let epoch = process.env.SOURCE_DATE_EPOCH;
  if (epoch === undefined) {
    try {
      epoch = execFileSync('git', ['log', '-1', '--format=%ct'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
    } catch {
      // Source archives need an explicit epoch for reproducible builds.
      epoch = String(Math.floor(Date.now() / 1000));
    }
  }
  if (!/^\d+$/.test(epoch)) throw new Error('Invalid SOURCE_DATE_EPOCH');
  mkdirSync(output, { recursive: true });
  const destination = resolve(output, `workspace-indicator_${version}-1_${arch}.deb`);
  const tmp = mkdtempSync(join(tmpdir(), 'workspace-indicator-deb-'));
  try {
    const put = (name, data, mode = 0o644) => {
      const path = join(tmp, name);
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, data);
      chmodSync(path, mode);
    };
    put('debian-binary', '2.0\n');
    put('control/control', `Package: workspace-indicator
Version: ${version}-1
Architecture: ${arch}
Maintainer: ZanzyTHEbar <zacariahheim@gmail.com>
Section: x11
Priority: optional
Homepage: https://github.com/ZanzyTHEbar/workspace-indicator
Description: Lightweight workspace OSD for layer-shell Wayland compositors
 Supports Hyprland, Sway, and Niri. Requires a compatible Wayland session.
`);
    put('data/usr/bin/workspace-indicator', bytes, 0o755);
    const unit = readFileSync('systemd/workspace-indicator.service', 'utf8').replace(
      'ExecStart=%h/.local/bin/workspace-indicator', 'ExecStart=/usr/bin/workspace-indicator');
    put('data/usr/lib/systemd/user/workspace-indicator.service', unit);
    put('data/usr/share/doc/workspace-indicator/copyright', readFileSync('LICENSE'));
    for (const source of ['hyprland/workspace-indicator.conf', 'hyprland/workspace-indicator.lua', 'omarchy/theme-set-hook']) {
      put(`data/usr/share/workspace-indicator/${source}`, readFileSync(source));
    }
    for (const name of ['control', 'data']) {
      execFileSync('tar', ['--sort=name', '--format=ustar', '--owner=0', '--group=0', '--numeric-owner', `--mtime=@${epoch}`,
        '-cJf', join(tmp, `${name}.tar.xz`), '-C', join(tmp, name), '.']);
    }
    execFileSync('ar', ['rcD', destination, 'debian-binary', 'control.tar.xz', 'data.tar.xz'], { cwd: tmp });
    return destination;
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  console.log(build(process.argv[2], process.argv[3]));
}
