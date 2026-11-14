# Native source provenance

These are released upstream sources, not development snapshots or local
security backports. SHA-256 pins are enforced by `scripts/build-font-libs.sh`.
They address the identified vulnerabilities; this is not a claim that upstream
libraries are free of all vulnerabilities.

| Library | Release | Archive SHA-256 |
| --- | --- | --- |
| FreeType | 2.15.0 (2026-10-12) | `93cd47eb719c325107f2d3cf087d06299e944c3b48a40b55537302304dea1dd5` |
| Fontconfig | 2.18.3 (2026-08-06) | `4f7b554a38cdf78c033f666c8871f3749e14a094f65a07f630c91ed0b43d35e3` |
| Expat | 2.10.0 (2026-11-02) | `c624ba2998fc65df1163a49641ff3f9ab73138b11641d92c86bee0fb01b1bc7f` |

## FreeType

[Upstream release archive and detached signature](https://download.savannah.gnu.org/releases/freetype/)
and [release notes](https://github.com/freetype/freetype/blob/VER-2-15-0/docs/CHANGES).
The archive signature was verified against Werner Lemberg's key fingerprint
`E30674707856409FF1948010BE6C3AAC63AD8E3F`, also published in
[Arch's maintained FreeType recipe](https://gitlab.archlinux.org/archlinux/packaging/packages/freetype2/-/blob/main/PKGBUILD).
The archive also matched that recipe's published BLAKE2 checksum; SHA-256 above
was computed from this authenticated archive.

The release contains
[`fc85a255849229c024c8e65f536fe1875d84841c`](https://github.com/freetype/freetype/commit/fc85a255849229c024c8e65f536fe1875d84841c),
the variable-font variation-store overflow fix for CVE-2026-23865.
[The CNA record](https://github.com/CVEProject/cvelistV5/blob/main/cves/2026/23xxx/CVE-2026-23865.json)
identifies 2.14.2 as the first fixed release; 2.15.0 includes later security fixes.

## Fontconfig

[Upstream release metadata](https://gitlab.freedesktop.org/fontconfig/fontconfig/-/releases/2.18.3)
links the release archive and its
[published SHA-256 file](https://gitlab.freedesktop.org/api/v4/projects/890/packages/generic/fontconfig/2.18.3/fontconfig-2.18.3.tar.xz.sha256sum).
The build uses that release asset, not an automatically generated Git archive.

The release contains both empty-XDG-data-directory fixes:
[`bba8646fdacc58d9d5ccb0e37527627b28a52e9b`](https://gitlab.freedesktop.org/fontconfig/fontconfig/-/commit/bba8646fdacc58d9d5ccb0e37527627b28a52e9b)
and
[`4a579aca645fd70da6739c493fd13643aad33250`](https://gitlab.freedesktop.org/fontconfig/fontconfig/-/commit/4a579aca645fd70da6739c493fd13643aad33250).
Its `FcConfigXdgDataDirs` guards both an empty variable and empty path components
before indexing the final character.

## Expat

[Upstream release metadata](https://github.com/libexpat/libexpat/releases/tag/R_2_10_0)
publishes the archive's SHA-256 asset digest.
[Release notes](https://github.com/libexpat/libexpat/blob/R_2_10_0/expat/Changes)
identify 2.8.1 as fixing CVE-2026-45186 via
[PR 1216](https://github.com/libexpat/libexpat/pull/1216); 2.10.0 includes that
merge (`9bdfbc77e3355405ceefbe59420abed953a5657e`) and subsequent security fixes.
Fontconfig continues using the static Expat XML backend.

## Build tool

Fontconfig 2.18.3 requires Meson >= 1.11.0. Its released Autotools configure
script fails cross-compilation, so the build uses Meson 1.12.1's supported
cross-file interface with Ninja. No upstream source patches are carried.
The Zig 0.14.1 wrapper preserves preprocessing-only semantics when Meson supplies
both `-E` and `-c`; otherwise Zig attempts to compile the gperf input as C.
[PyPI's versioned metadata](https://pypi.org/pypi/meson/1.12.1/json)
publishes the source archive SHA-256:
`ab0a6ca09f8ef70c564c8241fb5a23957886a0b53fb58412b5e07eaf07dba743`.
This is a build-time Python tool only, not a runtime dependency.
