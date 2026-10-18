#!/usr/bin/env python3
"""Locate the one locked Shape source through Cargo, never a developer checkout."""
import argparse, json, subprocess
from pathlib import Path
PIN = "983022bf35bcfda332ef57aeab81d8015cf5f280"
parser = argparse.ArgumentParser()
parser.add_argument("--root", type=Path, required=True)
parser.add_argument("--cargo", default="cargo")
args = parser.parse_args()
result = subprocess.run([args.cargo, "metadata", "--locked", "--all-features", "--format-version", "1", "--manifest-path", str(args.root / "Cargo.toml")], check=True, capture_output=True, text=True)
packages = json.loads(result.stdout)["packages"]
matching = [p for p in packages if p["name"] == "shape-desktop-bridge" and p.get("source", "").endswith("#" + PIN)]
if len(matching) != 1:
    raise SystemExit("Expected exactly one Shape desktop bridge at the declared git revision")
source = Path(matching[0]["manifest_path"]).resolve().parents[2]
if not (source / "apps/desktop/src/card_kit_controller.cpp").is_file():
    raise SystemExit("Pinned Shape source is incomplete")
print(source)
