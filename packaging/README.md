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

For plugin ownership, first ensure the recipe pins a native release whose help
lists `--plugin-managed`. Older releases lack this flag; if the recipe is too old,
use the [source-build quick start](../README.md#omarchy-quattro-quick-start-manual-dependency-install)
instead. Standalone installation does not require this flag.

```sh
make build-aur
make inspect-aur
make install-aur
```

The local binary package requires Git, Make, and Arch base-devel/makepkg, but no
Rust compiler. There is no published AUR listing at present; this is a local
recipe, not a claim that `yay -S workspace-indicator-bin` works. Use
`make install-aur INSTALL_MODE=plugin` for plugin ownership with a compatible
native release. See the README quick start.

## Omarchy Quattro

The service plugin runs an installed binary as a child of the shell. The Make
build/inspect targets stage just the manifest, entry point, license, and README
for development. Validation delegates to `omarchy plugin validate`. Installation
uses `omarchy plugin add https://github.com/ZanzyTHEbar/workspace-indicator.git --enable`
after preflight; it never copies that staging folder into the user's plugin directory.
Official `omarchy plugin update` works on the resulting Git checkout; remove keeps
its confirmation. An existing non-Git directory must be explicitly removed first;
official remove preserves it in a timestamped backup. See the README migration.

The binary must already be installed with `--plugin-managed` support and on the
shell's PATH. Use `INSTALL_MODE=plugin` with local/tar/package install targets to
avoid standalone setup/service activation. `MANAGE_SERVICE=0` alone suppresses
service actions, not config writes. Disable the standalone service and migrate
Hyprland autostart explicitly before enabling this plugin. Native package updates
are independent of Git plugin updates; restart the plugin after updating the binary.
Removal leaves the separate binary and personal config intact.

## Debian

Prebuilt amd64 and arm64 packages are available on GitHub Releases. Local builds
require Node.js, GNU tar, xz, and binutils `ar`. The Make targets handle the builder,
control/payload inspection, and APT installation/removal. No maintainer script
runs user services as root; Make activates the service from the invoking user's session.

Debian payload timestamps default to the source Git commit time. Set
`SOURCE_DATE_EPOCH` to a nonnegative Unix timestamp to override it (including `0`).
Outside Git, builds use the current build time; set `SOURCE_DATE_EPOCH` explicitly
for reproducible builds from source archives. The outer `ar` archive retains
deterministic metadata.
