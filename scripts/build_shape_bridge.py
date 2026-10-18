#!/usr/bin/env python3
"""Build the pinned bridge and project only headers from that Cargo invocation."""
import argparse, json, os, shutil, subprocess
from pathlib import Path
parser = argparse.ArgumentParser()
parser.add_argument("--source", type=Path, required=True)
parser.add_argument("--workspace-root", type=Path, required=True)
parser.add_argument("--target", type=Path, required=True)
parser.add_argument("--include", type=Path, required=True)
parser.add_argument("--cargo", default="cargo")
parser.add_argument("--release", action="store_true")
a = parser.parse_args()
command = [a.cargo,"build","--locked","--manifest-path",str(a.workspace_root/"Cargo.toml"),"--package","deck-cli","--features","desktop","--target-dir",str(a.target),"--message-format=json-render-diagnostics"]
if a.release: command.append("--release")
env = os.environ.copy()
env.setdefault("CARGO_BUILD_JOBS", "2")
p = subprocess.Popen(command, stdout=subprocess.PIPE, text=True, env=env)
outputs = set()
archives = set()
for line in p.stdout:
    try: value = json.loads(line)
    except json.JSONDecodeError: continue
    if value.get("reason") == "build-script-executed" and "shape-desktop-bridge" in value.get("package_id", ""):
        outputs.add(Path(value["out_dir"])/"cxxbridge/include")
    if value.get("reason") == "compiler-artifact" and value.get("target", {}).get("name") == "shape_desktop_bridge":
        archives.update(Path(name) for name in value.get("filenames", []) if name.endswith((".a", ".lib")))
    if value.get("reason") == "compiler-message":
        rendered = value.get("message", {}).get("rendered")
        if rendered: print(rendered, end="", flush=True)
if p.wait(): raise SystemExit("Pinned Shape bridge build failed")
if len(archives) != 1: raise SystemExit("Expected one current bridge archive")
archive = archives.pop()
destination = a.target / ("release" if a.release else "debug") / ("shape_desktop_bridge.lib" if os.name == "nt" else "libshape_desktop_bridge.a")
if archive.resolve() != destination.resolve(): shutil.copyfile(archive, destination)
if len(outputs) != 1: raise SystemExit("Expected one current CXX build-script output")
origin = outputs.pop()
for name in ("rust/cxx.h", "shape-desktop-bridge/src/lib.rs.h"):
    src, dst = origin/name, a.include/name
    if not src.is_file(): raise SystemExit("Current CXX header missing: "+name)
    dst.parent.mkdir(parents=True, exist_ok=True)
    if dst.is_symlink(): dst.unlink()
    if not dst.is_file() or src.read_bytes() != dst.read_bytes(): shutil.copyfile(src, dst)
