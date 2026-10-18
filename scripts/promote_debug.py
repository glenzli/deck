#!/usr/bin/env python3
"""Atomically promote exactly the already-validated Deck bundle, without a rebuild."""
import argparse, hashlib, json, os, shutil, subprocess, time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument("--app",type=Path,required=True);p.add_argument("--evidence",type=Path,required=True);p.add_argument("--root",type=Path,required=True);a=p.parse_args()
def digest(path):
    h=hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda:file.read(1024*1024),b""):h.update(chunk)
    return h.hexdigest()
app=a.app.resolve();binary=app/"Contents/MacOS/deck-desktop";cli=app/"Contents/MacOS/deck-cli"
report=json.loads((a.evidence/"report.json").read_text())
if report.get("status")!="passed" or report.get("binary_sha256")!=digest(binary) or report.get("cli_sha256")!=digest(cli):raise SystemExit("App identity does not match the passed native evidence")
capabilities=json.loads(subprocess.check_output([str(binary),"--handoff-capabilities"]))
if capabilities.get("schema")!="deck.handoff-capabilities/1":raise SystemExit("Validated app lacks the Deck protocol")
a.root.mkdir(parents=True,exist_ok=True);lock=a.root/"promotion.lock"
try:lock.mkdir()
except FileExistsError:raise SystemExit("Another Deck promotion owns the lock")
try:
    release=a.root/"releases"/(time.strftime("%Y%m%dT%H%M%SZ",time.gmtime())+"-"+digest(binary)[:12]);release.mkdir(parents=True)
    target=release/"Deck.app";shutil.copytree(app,target,symlinks=True)
    if digest(target/"Contents/MacOS/deck-desktop")!=report["binary_sha256"] or digest(target/"Contents/MacOS/deck-cli")!=report["cli_sha256"]:raise SystemExit("Copied bundle identity mismatch")
    (release/"validation.json").write_text(json.dumps({"binary_sha256":report["binary_sha256"],"cli_sha256":report["cli_sha256"],"evidence":str(a.evidence.resolve()),"shape_revision":"983022bf35bcfda332ef57aeab81d8015cf5f280"},indent=2)+"\n")
    pending=a.root/(".current-debug-"+str(os.getpid()));pending.symlink_to(release, target_is_directory=True);os.replace(pending,a.root/"current-debug")
    print(json.dumps({"status":"promoted","app":str(target),"current":str(a.root/"current-debug"),"binary_sha256":report["binary_sha256"]}))
finally:lock.rmdir()
