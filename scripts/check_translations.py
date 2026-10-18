#!/usr/bin/env python3
"""Require finished Simplified Chinese for every extracted desktop source identity."""
import xml.etree.ElementTree as ET
from pathlib import Path
path=Path(__file__).resolve().parents[1]/"apps/desktop/translations/shape_zh_CN.ts"
root=ET.parse(path).getroot();missing=[];count=0
for context in root.findall("context"):
    for message in context.findall("message"):
        translation=message.find("translation")
        if translation is not None and translation.get("type") in ("obsolete","vanished"):continue
        count+=1
        if translation is None or translation.get("type")=="unfinished" or not "".join(translation.itertext()).strip():missing.append((context.findtext("name"),message.findtext("source")))
if missing:raise SystemExit("Unfinished Chinese translations: "+repr(missing))
print(f"Finished Simplified Chinese: {count} identities")
