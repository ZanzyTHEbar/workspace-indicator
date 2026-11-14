# Native font closure

`scripts/build-font-libs.sh` pins source versions and SHA-256 checksums for
Fontconfig 2.18.3, FreeType 2.15.0, and Expat 2.10.0. Zig 0.14.1 provides
architecture-specific musl headers, compiler runtime and C compiler; no host
Fontconfig/FreeType archives are used. GNU-libc development builds use the
same sources compiled separately for GNU libc.

These maintained releases include the FreeType CVE-2026-23865 fix, both
Fontconfig empty-XDG-directory fixes, and Expat's CVE-2026-45186 fix (plus
later security fixes). Release and checksum provenance is in
[`DEPENDENCIES.md`](DEPENDENCIES.md).

Build prerequisites: Rust, a C/C++ host toolchain, Python 3.10+, Ninja, make, pkg-config, gperf,
curl, tar, xz, and ar. A pinned Zig is downloaded and verified if not installed;
`ZIG=/absolute/path/to/zig` selects an existing 0.14.1 binary.
Native build products live under Cargo's output directory. Set
`CARGO_TARGET_DIR` and `TMPDIR` to choose disposable build storage.
Native build trees are separated by target/libc and library version, so updating
a pin cannot reuse objects from the previous version.
Expat and FreeType use Autotools; Fontconfig uses its supported Meson build.
Meson 1.12.1 source is downloaded with a pinned checksum and run locally through
Python, without pip or a global installation. Host CMake packages and Meson
dependency downloads are disabled; only the static library and headers are staged.
The Zig C wrapper removes `-c` from preprocessing-only (`-E`) invocations, matching
GCC/Clang semantics required by Meson's generated-header preprocessing.

Only static libraries are built. HarfBuzz, Brotli/WOFF2, external zlib,
bzip2, PNG, SVG rendering, libxml2, iconv, translations and documentation
are disabled. Ordinary TrueType and OpenType/CFF faces remain supported.
Fontconfig tools are neither installed nor invoked for font resolution.
The normal binary has no shared-library font dependency.

`WORKSPACE_INDICATOR_FONT_PREFIX` plus `WORKSPACE_INDICATOR_FONT_CC` can reuse
an already built, matching-target prefix and C compiler. Do not point these
at host libraries for a musl build.

Licenses: Fontconfig's MIT-like notice, Expat's MIT notice, and the FreeType
License are in `licenses/`. This software uses the FreeType library.
FreeType's bundled gzip support uses zlib (notice also included).
These notices and the Lato license accompany release tarballs, debs and AUR
packages; no native config or font-data installation is necessary.
