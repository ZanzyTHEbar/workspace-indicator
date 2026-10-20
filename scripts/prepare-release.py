"""Set Cargo version before building both archives."""
import re
import subprocess
import sys
from pathlib import Path


def set_version(content, version):
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version):
        raise ValueError(f"Invalid semantic version: {version}")
    updated, count = re.subn(r'^version = "[^"]+"$', f'version = "{version}"', content, count=1, flags=re.M)
    if count != 1:
        raise ValueError("Cargo package version not found")
    return updated


if __name__ == "__main__":
    manifest = Path("Cargo.toml")
    manifest.write_text(set_version(manifest.read_text(), sys.argv[1]))
    subprocess.run(["cargo", "generate-lockfile"], check=True)
    for arch in ("x86_64", "aarch64"):
        subprocess.run(["make", "dist", f"RUST_TARGET={arch}-unknown-linux-musl", f"ARCH_SUFFIX={arch}"], check=True)
