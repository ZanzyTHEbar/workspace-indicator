# Distribution packaging

## AUR

`packaging/aur/PKGBUILD` targets immutable v0.4.0 assets for x86_64 and
aarch64, with verified SHA-256 hashes. Run `makepkg --verifysource`,
`makepkg`, and `makepkg --printsrcinfo > .SRCINFO` from that directory.
Publish only PKGBUILD and .SRCINFO to the AUR repository using your AUR account.
The package has not yet been submitted to AUR.

The distribution unit starts `/usr/bin/workspace-indicator`; the tarball
unit intentionally starts `~/.local/bin/workspace-indicator`.

## Omarchy Quattro plugin

The root `manifest.json` declares a `service` entry point, `Service.qml`.
It runs the installed binary as a child of the shell. Disabling/removing the
plugin stops its owned child process. It does not download code or elevate privileges.

1. Install the binary using the AUR recipe or a release tarball.
2. Disable any systemd autostart: `systemctl --user disable --now workspace-indicator.service`.
3. Install this repository as a plugin using Omarchy's plugin manager.
4. Run `omarchy plugin validate <plugin-directory>` and enable
   `zanzythebar.workspace-indicator`.
5. Test disable/re-enable, shell restart, theme changes, and removal on Quattro.

Choose either the plugin or the systemd unit as process owner, not both.
Existing Hyprland autostart snippets that start the service must also be disabled
when using the plugin. Keybindings and blur rules can still be used.
Uninstalling the plugin leaves the separately installed binary and user configuration intact.

Marketplace submission requires a public repository, root manifest, README,
license, and maintainer approval. Submit through
https://plugins.omarchy.org/publish.html only after Quattro runtime validation.
This integration has not yet been runtime-tested on Quattro or submitted.

## Debian

Build the static binary, then:

```sh
python3 scripts/build-deb.py target/x86_64-unknown-linux-musl/release/workspace-indicator amd64
python3 scripts/build-deb.py target/aarch64-unknown-linux-musl/release/workspace-indicator arm64
```

Requires Python 3 and binutils `ar`. Output is under `dist/`.
Inspect with `dpkg-deb --info` and `dpkg-deb --contents`, then install with
`sudo apt install ./dist/workspace-indicator_*_amd64.deb`.

The package does not run user services as root from maintainer scripts. After
installing, configure the compositor integration, import the session environment,
and explicitly enable the user unit if desired:

```sh
systemctl --user daemon-reload
systemctl --user import-environment WAYLAND_DISPLAY XDG_RUNTIME_DIR SWAYSOCK NIRI_SOCKET HYPRLAND_INSTANCE_SIGNATURE
systemctl --user enable --now workspace-indicator.service
```

Prebuilt amd64 and arm64 `.deb` packages are published on GitHub Releases
starting with [v0.4.2](https://github.com/ZanzyTHEbar/workspace-indicator/releases/tag/v0.4.2).
Release packaging is automatic on semantic-version releases.

Requires a Wayland session with layer-shell support and a compatible Hyprland,
Sway, or Niri IPC socket. No X11 support. `.deb` files are published on GitHub
Releases; no signed APT repository or official Debian archive listing is provided.
