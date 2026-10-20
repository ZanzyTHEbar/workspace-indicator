"""Build a static-binary Debian package without installing cargo-deb."""
import io
import os
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from pathlib import Path


def build(binary, arch, output):
    if arch not in ("amd64", "arm64"):
        raise ValueError("Expected amd64 or arm64")
    binary = Path(binary)
    header = binary.read_bytes()[:20]
    machine = 62 if arch == "amd64" else 183
    if header[:6] != b"\x7fELF\x02\x01" or int.from_bytes(header[18:20], "little") != machine:
        raise ValueError("Binary architecture does not match package")
    version = tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"]
    output = Path(output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    destination = output / f"workspace-indicator_{version}-1_{arch}.deb"
    control = f"""Package: workspace-indicator
Version: {version}-1
Architecture: {arch}
Maintainer: ZanzyTHEbar <zacariahheim@gmail.com>
Section: x11
Priority: optional
Homepage: https://github.com/ZanzyTHEbar/workspace-indicator
Description: Lightweight workspace OSD for layer-shell Wayland compositors
 Supports Hyprland, Sway, and Niri. Requires a compatible Wayland session.
"""
    unit = Path("systemd/workspace-indicator.service").read_text().replace(
        "ExecStart=%h/.local/bin/workspace-indicator", "ExecStart=/usr/bin/workspace-indicator")
    files = {
        "usr/bin/workspace-indicator": (binary.read_bytes(), 0o755),
        "usr/lib/systemd/user/workspace-indicator.service": (unit.encode(), 0o644),
        "usr/share/doc/workspace-indicator/copyright": (Path("LICENSE").read_bytes(), 0o644),
    }
    for source in ("hyprland/workspace-indicator.conf", "hyprland/workspace-indicator.lua", "omarchy/theme-set-hook"):
        files[f"usr/share/workspace-indicator/{source}"] = (Path(source).read_bytes(), 0o644)
    epoch = int(os.environ.get("SOURCE_DATE_EPOCH", "0"))

    def archive(path, entries):
        with tarfile.open(path, "w:xz", format=tarfile.USTAR_FORMAT) as tar:
            for name, (data, mode) in sorted(entries.items()):
                info = tarfile.TarInfo(name)
                info.size, info.mode, info.mtime = len(data), mode, epoch
                tar.addfile(info, io.BytesIO(data))

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        (tmp / "debian-binary").write_text("2.0\n")
        archive(tmp / "control.tar.xz", {"control": (control.encode(), 0o644)})
        archive(tmp / "data.tar.xz", files)
        subprocess.run(["ar", "rcD", str(destination), "debian-binary", "control.tar.xz", "data.tar.xz"], cwd=tmp, check=True)
    return destination


if __name__ == "__main__":
    print(build(sys.argv[1], sys.argv[2], "dist"))
