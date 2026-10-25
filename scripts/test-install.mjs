// Exercise installation/removal in a disposable HOME, never the live session.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname, resolve } from 'node:path';

const tmp = mkdtempSync(join(tmpdir(), 'workspace-install-test-'));
try {
  const bundle = join(tmp, 'bundle');
  const home = join(tmp, 'home');
  mkdirSync(bundle);
  for (const source of ['Cargo.toml', 'Makefile', 'install.sh', 'scripts/manage-service.sh',
    'scripts/manage-tar.sh', 'scripts/workspace-indicator-focusd', 'systemd', 'hyprland', 'omarchy']) {
    const dest = join(bundle, source);
    mkdirSync(dirname(dest), { recursive: true });
    cpSync(source, dest, { recursive: true });
  }
  writeFileSync(join(bundle, 'workspace-indicator'), '#!/bin/sh\nexit 0\n');
  chmodSync(join(bundle, 'workspace-indicator'), 0o755);
  const config = join(home, '.config');
  mkdirSync(join(config, 'hypr/config'), { recursive: true });
  mkdirSync(join(config, 'omarchy'), { recursive: true });
  writeFileSync(join(config, 'hypr/hyprland.lua'), '-- user settings\n');
  const env = { ...process.env, HOME: home, XDG_CONFIG_HOME: config, MANAGE_SERVICE: '0' };
  // Unset inherited custom paths so all writes stay inside the sandbox.
  for (const key of ['PREFIX', 'BINDIR', 'SYSTEMD_USER_DIR', 'HYPR_DIR', 'HYPR_CONF_DIR', 'OMARCHY_HOOKS_ROOT', 'MAKEFLAGS']) delete env[key];
  const make = (...args) => execFileSync('make', args, { cwd: bundle, env });
  make('install'); // A release bundle must not need cargo, Node, or a source tree.
  const installed = ['.local/bin/workspace-indicator', '.local/bin/workspace-indicator-focusd',
    '.config/systemd/user/workspace-indicator.service', '.config/hypr/config/workspace-indicator.local.conf',
    '.config/hypr/workspace-indicator.lua', '.config/omarchy/hooks/theme-set-workspace-indicator',
    '.config/omarchy/hooks/theme-set.d/workspace-indicator'];
  for (const file of installed) assert.ok(existsSync(join(home, file)), file);
  make('install');
  assert.equal(readFileSync(join(config, 'hypr/hyprland.lua'), 'utf8').match(/pcall\(require/g).length, 1);
  make('uninstall');
  for (const file of installed) assert.ok(!existsSync(join(home, file)), file);
  assert.match(readFileSync(join(config, 'hypr/hyprland.lua'), 'utf8'), /user settings/);
  make('uninstall'); // Repeated removal is harmless.

  const archive = join(tmp, 'release.tar.gz');
  execFileSync('tar', ['-czf', archive, '-C', bundle, '.']);
  make('install-tar', `ARCHIVE=${archive}`);
  assert.ok(existsSync(join(home, '.local/bin/workspace-indicator')));
  make('uninstall-tar');
  assert.ok(!existsSync(join(home, '.local/bin/workspace-indicator')));

  const dryRun = execFileSync('make', ['-n', 'build-deb', 'ARCH=aarch64'], { cwd: resolve('.'), encoding: 'utf8' });
  assert.match(dryRun, /aarch64-unknown-linux-musl/);
  assert.match(dryRun, /build-deb\.mjs .* "arm64"/);
  const packageInstall = execFileSync('make', ['-n', 'install-deb', 'PACKAGE=/tmp/example.deb'], { encoding: 'utf8' });
  assert.ok(!packageInstall.includes('cargo build'));
  assert.match(packageInstall, /apt install "\/tmp\/example.deb"/);

  const tools = join(tmp, 'tools');
  mkdirSync(tools);
  const log = join(tmp, 'systemctl.log');
  writeFileSync(join(tools, 'systemctl'), '#!/bin/bash\nprintf "%s\\n" "$*" >> "$TEST_SERVICE_LOG"\ncase "$*" in\n  *is-active*) exit 1 ;;\n  *is-enabled*) exit "${TEST_SERVICE_ENABLED:-1}" ;;\nesac\n');
  chmodSync(join(tools, 'systemctl'), 0o755);
  const serviceEnv = { ...env, MANAGE_SERVICE: '1', PATH: `${tools}:${env.PATH}`,
    TEST_SERVICE_LOG: log, XDG_RUNTIME_DIR: tmp, WAYLAND_DISPLAY: 'wayland-test' };
  execFileSync('bash', ['scripts/manage-service.sh', 'start'], { env: serviceEnv });
  execFileSync('bash', ['scripts/manage-service.sh', 'stop'], { env: serviceEnv });
  const calls = readFileSync(log, 'utf8');
  assert.match(calls, /--user enable --now workspace-indicator.service/);
  assert.match(calls, /--user disable --now workspace-indicator.service/);
  assert.match(calls, /--user import-environment WAYLAND_DISPLAY/);

  const pluginLog = join(tmp, 'plugin.log');
  for (const tool of ['omarchy', 'omarchy-shell']) {
    writeFileSync(join(tools, tool), '#!/bin/bash\nprintf "%s\\n" "$*" >> "$TEST_PLUGIN_LOG"\n');
    chmodSync(join(tools, tool), 0o755);
  }
  writeFileSync(join(tools, 'workspace-indicator'), '#!/bin/sh\necho --plugin-managed\n');
  chmodSync(join(tools, 'workspace-indicator'), 0o755);
  const pluginEnv = { ...serviceEnv, TEST_PLUGIN_LOG: pluginLog };
  const beforePreflight = readFileSync(log, 'utf8');
  assert.throws(() => execFileSync('make', ['install-omarchy'], {
    env: { ...pluginEnv, TEST_SERVICE_ENABLED: '0' }, stdio: 'pipe'
  }), error => error.status !== 0 && error.stdout.toString().includes('systemctl --user disable --now workspace-indicator.service'));
  assert.ok(!existsSync(pluginLog), 'Enabled-but-stopped service must prevent official plugin add');
  const preflightCalls = readFileSync(log, 'utf8').slice(beforePreflight.length);
  assert.equal(preflightCalls, '--user is-active --quiet workspace-indicator.service\n--user is-enabled --quiet workspace-indicator.service\n', 'Preflight must only query service state');
  execFileSync('make', ['install-omarchy'], { env: pluginEnv });
  const pluginDir = join(config, 'omarchy/plugins/zanzythebar.workspace-indicator');
  assert.ok(!existsSync(pluginDir), 'Make must delegate installation, not copy files');
  mkdirSync(pluginDir, { recursive: true }); // Simulate the official installer owning its destination.
  assert.throws(() => execFileSync('make', ['install-omarchy'], { env: pluginEnv, stdio: 'pipe' }));
  execFileSync('make', ['uninstall-omarchy'], { env: pluginEnv });
  const pluginCalls = readFileSync(pluginLog, 'utf8');
  assert.match(pluginCalls, /plugin add https:\/\/github.com\/ZanzyTHEbar\/workspace-indicator.git --enable/);
  assert.match(pluginCalls, /plugin remove zanzythebar.workspace-indicator\n/);
  assert.ok(!pluginCalls.includes('--yes'), 'Official consent must be preserved');

  const packageLog = join(tmp, 'packages.log');
  for (const tool of ['apt', 'pacman']) {
    writeFileSync(join(tools, tool), `#!/bin/bash\nprintf '%s\\n' '${tool} '"$*" >> "$TEST_PACKAGE_LOG"\n`);
    chmodSync(join(tools, tool), 0o755);
  }
  writeFileSync(join(tools, 'makepkg'), '#!/bin/bash\nif [[ "$*" == *--packagelist* ]]; then printf "%s\\n" "$TEST_PACKAGE"; fi\n');
  chmodSync(join(tools, 'makepkg'), 0o755);
  const packageEnv = { ...serviceEnv, TEST_PACKAGE_LOG: packageLog, TEST_PACKAGE: join(tmp, 'test.pkg.tar.zst') };
  execFileSync('make', ['install-deb', 'uninstall-deb', 'install-aur', 'uninstall-aur',
    `PACKAGE=${join(tmp, 'example.deb')}`, 'SUDO='], { env: packageEnv });
  const packageCalls = readFileSync(packageLog, 'utf8');
  assert.ok(packageCalls.includes(`apt install ${join(tmp, 'example.deb')}`));
  assert.match(packageCalls, /apt remove workspace-indicator/);
  assert.ok(packageCalls.includes(`pacman -U --needed ${packageEnv.TEST_PACKAGE}`));
  assert.match(packageCalls, /pacman -R workspace-indicator-bin/);
  console.log('install checks passed');
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
