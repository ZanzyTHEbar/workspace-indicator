# Distribution packaging

All build, inspect, install, and uninstall operations are exposed through the
root Makefile. See `make help` and the [install guide](../README.md#build-inspect-install-uninstall).

| Format | Build | Inspect | Install | Uninstall |
| --- | --- | --- | --- | --- |
| User install | `make build` | `make inspect` | `make install` | `make uninstall` |
| Tarball | `make build-tar` | `make inspect-tar` | `make install-tar` | `make uninstall-tar` |
| Debian | `make build-deb` | `make inspect-deb` | `make install-deb` | `make uninstall-deb` |
| AUR | `make build-aur` | `make inspect-aur` | `make install-aur` | `make uninstall-aur` |
| Omarchy | `make build-omarchy` | `make inspect-omarchy` | `make install-omarchy` | `make uninstall-omarchy` |

`ARCH=x86_64|aarch64` selects cross-build architecture. `PACKAGE=/path/to/file.deb`
skips compilation for Debian installs; `ARCHIVE=/path/to/file.tar.gz` selects an
existing tarball. APT/pacman resolve host installation/removal. Build operations
never install system packages. Run `make toolchain` to add the selected musl target
to an existing rustup installation; a suitable cross-linker is also required.

## AUR

```sh
make build-aur
make inspect-aur
make install-aur
```

The binary package requires no Rust compiler.

## Omarchy Quattro

The service plugin runs an installed binary as a child of the shell. The Make
targets stage just the manifest, entry point, license, and README—not a Git clone.
Validation delegates to `omarchy plugin validate`. Installation refuses to replace
an existing directory; removal delegates to `omarchy plugin remove --yes`.

The binary must already be installed and on the shell's PATH. Disable any Hyprland
autostart snippet that starts the systemd service before enabling this plugin.
Removal leaves the separate binary and personal config intact.

## Debian

Prebuilt amd64 and arm64 packages are available on GitHub Releases. Local builds
require Node.js, GNU tar, xz, and binutils `ar`. The Make targets handle the builder,
control/payload inspection, and APT installation/removal. No maintainer script
runs user services as root; Make activates the service from the invoking user's session.
