#!/usr/bin/env bash
# Build the native font closure with musl, never host pkg-config archives.
set -eu -o pipefail
arch=${1:?Expected x86_64 or aarch64}
root=${2:?Expected an absolute build directory}
abi=${3:-musl}
case "$arch" in x86_64|aarch64) ;; *) echo "Unsupported architecture: $arch" >&2; exit 1 ;; esac
case "$abi" in musl|gnu) ;; *) echo "Unsupported libc: $abi" >&2; exit 1 ;; esac
case "$root" in /*) ;; *) echo 'Build directory must be absolute' >&2; exit 1 ;; esac
mkdir -p "$root/downloads"
if test -n "${ZIG:-}"; then
    zig=$ZIG
elif command -v zig >/dev/null; then
    zig=$(command -v zig)
else
    host=$(uname -m)
    case "$host" in
        x86_64) sha=24aeeec8af16c381934a6cd7d95c807a8cb2cf7df9fa40d359aa884195c4716c ;;
        aarch64) sha=f7a654acc967864f7a050ddacfaa778c7504a0eca8d2b678839c21eea47c992b ;;
        *) echo "Unsupported build host: $host" >&2; exit 1 ;;
    esac
    archive=$root/downloads/zig-$host-0.14.1.tar.xz
    if ! test -f "$archive"; then
        curl --fail --location --retry 2 "https://ziglang.org/download/0.14.1/zig-$host-linux-0.14.1.tar.xz" -o "$archive"
    fi
    echo "$sha  $archive" | sha256sum --check -
    if ! test -d "$root/zig-$host-linux-0.14.1"; then
        tar -xJf "$archive" -C "$root"
    fi
    zig=$root/zig-$host-linux-0.14.1/zig
fi
test "$("$zig" version)" = 0.14.1
command -v gperf >/dev/null
command -v ninja >/dev/null
target=$arch-$abi
mkdir -p "$root/src" "$root/$target/bin" "$root/$target/prefix"
root=$(cd "$root" && pwd)
prefix=$root/$target/prefix
export ZIG_GLOBAL_CACHE_DIR=$root/zig-cache
export ZIG_LOCAL_CACHE_DIR=$root/$target/zig-cache
export CCACHE_DIR=${CCACHE_DIR:-$root/host-ccache}
export PKG_CONFIG_LIBDIR=$prefix/lib/pkgconfig
export PKG_CONFIG_PATH=
export CC=$root/$target/bin/cc
export AR=$root/$target/bin/ar
export RANLIB=$root/$target/bin/ranlib
cat > "$CC" <<EOF
#!/bin/bash
# Meson passes -E and -c together; Zig 0.14.1 lets -c override preprocessing.
preprocess=false
for arg; do
    if [[ "\$arg" == -E ]]; then preprocess=true; fi
done
args=()
for arg; do
    if \$preprocess && [[ "\$arg" == -c ]]; then continue; fi
    args+=("\$arg")
done
exec "$zig" cc -target "$arch-linux-$abi" "\${args[@]}"
EOF
printf '#!/bin/sh\nexec "%s" ar "$@"\n' "$zig" > "$AR"
printf '#!/bin/sh\nexec "%s" ranlib "$@"\n' "$zig" > "$RANLIB"
chmod +x "$CC" "$AR" "$RANLIB"
export CFLAGS='-O2 -fPIC'
if test "$abi" = musl; then
    export LDFLAGS=-static
else
    export LDFLAGS=
fi
jobs=${JOBS:-4}

fetch() {
    local name=$1 url=$2 sha=$3 extension=${4:-xz}
    if ! test -f "$root/downloads/$name.tar.$extension"; then
        curl --fail --location --retry 2 "$url" -o "$root/downloads/$name.tar.$extension"
    fi
    echo "$sha  $root/downloads/$name.tar.$extension" | sha256sum --check -
    if ! test -d "$root/src/$name"; then
        tar -xf "$root/downloads/$name.tar.$extension" -C "$root/src"
    fi
}

fetch expat-2.10.0 https://github.com/libexpat/libexpat/releases/download/R_2_10_0/expat-2.10.0.tar.xz \
    c624ba2998fc65df1163a49641ff3f9ab73138b11641d92c86bee0fb01b1bc7f
fetch freetype-2.15.0 https://download.savannah.gnu.org/releases/freetype/freetype-2.15.0.tar.xz \
    93cd47eb719c325107f2d3cf087d06299e944c3b48a40b55537302304dea1dd5
fetch fontconfig-2.18.3 https://gitlab.freedesktop.org/api/v4/projects/890/packages/generic/fontconfig/2.18.3/fontconfig-2.18.3.tar.xz \
    4f7b554a38cdf78c033f666c8871f3749e14a094f65a07f630c91ed0b43d35e3
fetch meson-1.12.1 https://files.pythonhosted.org/packages/f9/c9/8c9983f4f3d9c4e22fd76bcf8cd053e4472aefae45fd044669b6daa34b53/meson-1.12.1.tar.gz \
    ab0a6ca09f8ef70c564c8241fb5a23957886a0b53fb58412b5e07eaf07dba743 gz

build=$("$root/src/expat-2.10.0/conftools/config.guess")
common=(--build="$build" --host="$arch-linux-$abi" --prefix="$prefix" --libdir="$prefix/lib"
    --enable-static --disable-shared --disable-dependency-tracking)
# A pin change must not reuse objects from the previous library version.
mkdir -p "$root/$target/expat-2.10.0" "$root/$target/freetype-2.15.0"
(
    cd "$root/$target/expat-2.10.0"
    "$root/src/expat-2.10.0/configure" "${common[@]}" \
        --without-xmlwf --without-examples --without-tests --without-docbook
    make -j"$jobs"
    make install
)
(
    cd "$root/$target/freetype-2.15.0"
    "$root/src/freetype-2.15.0/configure" "${common[@]}" \
        --with-zlib=no --with-bzip2=no --with-png=no --with-harfbuzz=no \
        --with-brotli=no --with-librsvg=no
    sed -i '/^#define TT_CONFIG_OPTION_COLOR_LAYERS$/d; /^#define FT_CONFIG_OPTION_SVG$/d' ftoption.h
    make -j"$jobs"
    make install
)
(
    meson=(python3 "$root/src/meson-1.12.1/meson.py")
    build_dir=$root/$target/fontconfig-2.18.3-meson
    cross_file=$root/$target/fontconfig-cross.ini
    # No host CMake packages or implicit dependency downloads in a cross build.
    cat > "$cross_file" <<EOF
[binaries]
c = '$CC'
ar = '$AR'
pkg-config = '$(command -v pkg-config)'
cmake = '/bin/false'
[host_machine]
system = 'linux'
cpu_family = '$arch'
cpu = '$arch'
endian = 'little'
[properties]
needs_exe_wrapper = true
pkg_config_libdir = ['$prefix/lib/pkgconfig']
EOF
    reconfigure=()
    if test -f "$build_dir/meson-private/coredata.dat"; then
        reconfigure=(--reconfigure)
    fi
    "${meson[@]}" setup "${reconfigure[@]}" "$build_dir" "$root/src/fontconfig-2.18.3" \
        --cross-file "$cross_file" --prefix "$prefix" --libdir lib --sysconfdir /etc \
        --buildtype plain --wrap-mode nodownload -Ddefault_library=static -Dprefer_static=true \
        -Ddoc=disabled -Dnls=disabled -Dtests=disabled -Dtests-external-fonts=disabled \
        -Dtools=disabled -Dcache-build=disabled -Diconv=disabled -Dfontations=disabled \
        -Dxml-backend=expat -Dcache-dir=/var/cache/fontconfig
    "${meson[@]}" compile -C "$build_dir" -j "$jobs" fontconfig
    # Do not run meson install: it also installs system Fontconfig configuration.
    install -m644 "$build_dir/libfontconfig.a" "$prefix/lib/"
    install -d "$prefix/include/fontconfig"
    install -m644 "$build_dir/fontconfig/fontconfig.h" \
        "$root/src/fontconfig-2.18.3/fontconfig/fcfreetype.h" \
        "$root/src/fontconfig-2.18.3/fontconfig/fcprivate.h" "$prefix/include/fontconfig/"
    install -m644 "$build_dir/meson-private/fontconfig.pc" "$prefix/lib/pkgconfig/"
)
printf 'Native font libraries: %s\n' "$prefix"
