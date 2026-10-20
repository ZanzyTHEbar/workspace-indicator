"""Check plugin layout and actual Debian archive payload, without installing."""
import importlib.util
import io
import json
import subprocess
import tarfile
import tempfile
from pathlib import Path

plugin = json.loads(Path("manifest.json").read_text())
assert plugin["schemaVersion"] == 1
assert plugin["kinds"] == ["service"]
assert Path(plugin["entryPoints"]["service"]).is_file()
spec = importlib.util.spec_from_file_location("deb", "scripts/build-deb.py")
deb = importlib.util.module_from_spec(spec)
spec.loader.exec_module(deb)
binary = Path("target/x86_64-unknown-linux-musl/release/workspace-indicator")
with tempfile.TemporaryDirectory() as tmp:
    package = deb.build(binary, "amd64", tmp)
    control = subprocess.check_output(["ar", "p", package, "control.tar.xz"])
    with tarfile.open(fileobj=io.BytesIO(control), mode="r:xz") as archive:
        assert b"Architecture: amd64\n" in archive.extractfile("control").read()
    data = subprocess.check_output(["ar", "p", package, "data.tar.xz"])
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:xz") as archive:
        unit = archive.extractfile("usr/lib/systemd/user/workspace-indicator.service").read()
        assert b"ExecStart=/usr/bin/workspace-indicator\n" in unit
        assert archive.getmember("usr/bin/workspace-indicator").mode == 0o755
        assert "usr/share/doc/workspace-indicator/copyright" in archive.getnames()
    try:
        deb.build(binary, "arm64", tmp)
    except ValueError:
        pass
    else:
        raise AssertionError("Architecture mismatch accepted")
print("packaging checks passed")
