#!/usr/bin/env python3
"""Validate pinned CardKit owners and identify explicit local adaptations."""
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
base = root / "third_party"
manifest = json.loads((base / "shape-cardkit-provenance.json").read_text())
actual = {}
for name, original in manifest["files"].items():
    path = base / name
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if name not in manifest["adapted_files"] and digest != original:
        raise SystemExit("Unrecorded CardKit adaptation: " + name)
    actual[name] = digest
for crate in ("shape-core", "shape-domain"):
    license = base / crate / "LICENSE"
    actual[str(license.relative_to(base))] = hashlib.sha256(license.read_bytes()).hexdigest()
    if actual[str(license.relative_to(base))] != manifest["license_sha256"]:
        raise SystemExit("CardKit MIT license changed")
    for path in (base / crate).rglob("*"):
        if path.is_file() and str(path.relative_to(base)) not in actual:
            raise SystemExit("Unrecorded CardKit source: " + str(path))
identity = hashlib.sha256(json.dumps(actual, sort_keys=True).encode()).hexdigest()
print(json.dumps({"base_revision": manifest["revision"], "source_sha256": identity,
                  "adaptations": manifest["adapted_files"], "files": len(actual)}))
