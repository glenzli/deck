#!/usr/bin/env python3
"""Extract Deck's actual sources and restore supplied and Deck-owned translations."""
import json, subprocess, xml.etree.ElementTree as ET
from pathlib import Path
root=Path(__file__).resolve().parents[1]
shape=Path(subprocess.check_output(["python3",str(root/"scripts/locate_shape_source.py"),"--root",str(root)],text=True).strip())
path=root/"apps/desktop/translations/shape_zh_CN.ts"
upstream=ET.parse(root/"third_party/shape-ui/upstream_zh_CN.ts").getroot()
known={}
for context in upstream.findall("context"):
    for m in context.findall("message"):
        t=m.find("translation")
        if t is not None and t.get("type") not in ("unfinished","obsolete","vanished"):
            known[(context.findtext("name"),m.findtext("source"))]="".join(t.itertext())
local=json.loads((root/"apps/desktop/translations/deck_zh_CN.json").read_text())
cpp=["desktop_backend.cpp","card_kit_controller.cpp","card_print_controller.cpp","material_edit_controller.cpp","ui_preferences.cpp","audio_preview_controller.cpp"]
subprocess.run(["lupdate",str(root/"apps/desktop/qml"),str(root/"apps/desktop/src"),*[str(shape/"apps/desktop/src"/name) for name in cpp],"-no-obsolete","-locations","none","-ts",str(path)],check=True)
tree=ET.parse(path)
for context in tree.getroot().findall("context"):
    for message in context.findall("message"):
        source=message.findtext("source");translation=message.find("translation")
        value=local.get(source) or known.get((context.findtext("name"),source))
        if value:
            if translation is None:translation=ET.SubElement(message,"translation")
            translation.attrib.pop("type",None);translation.text=value
ET.indent(tree,space="    ")
tree.write(path,encoding="utf-8",xml_declaration=True)
subprocess.run(["python3",str(root/"scripts/check_translations.py")],check=True)
