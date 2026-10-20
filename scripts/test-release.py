import importlib.util
import json
from pathlib import Path

spec = importlib.util.spec_from_file_location("release", "scripts/prepare-release.py")
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)
assert release.set_version('[package]\nversion = "0.4.0"\n', "1.2.3") == '[package]\nversion = "1.2.3"\n'
for bad in ("", "v1.2.3", "1.2; touch /tmp/bad"):
    try:
        release.set_version('version = "0.4.0"', bad)
    except ValueError:
        pass
    else:
        raise AssertionError(bad)
config = json.loads(Path(".releaserc.json").read_text())
assert config["branches"] == ["main"]
analyzer = config["plugins"][0]
assert analyzer[0] == "@semantic-release/commit-analyzer"
assert {"type": "build", "scope": "debian", "release": "patch"} in analyzer[1]["releaseRules"]
assert "@semantic-release/npm" not in str(config)
print("release checks passed")
