#![cfg(unix)]
mod support;
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt};
use support::{Fixture, decode};

#[test]
fn pdf_proxy_normalizes_defaults_and_alias_then_verifies_the_native_protocol_bytes() {
    let fixture = Fixture::new();
    let accepted = fixture.accepted();
    let alias = if fixture.project.starts_with("/private/tmp") {
        std::path::PathBuf::from(fixture.project.to_string_lossy().replacen(
            "/private/tmp",
            "/tmp",
            1,
        ))
    } else {
        fixture.project.clone()
    };
    let destination = fixture.root.join("protocol-only.pdf");
    let request = fixture.write(
        "pdf.json",
        &json!({"schema":"deck.pdf-export/1","project":alias,
        "source":accepted["source"],"destination":destination}),
    );
    // Protocol fixture only. The native steward separately tests real QPdfWriter output.
    let writer = fixture.root.join("protocol-writer.py");
    fs::write(&writer,r"#!/usr/bin/env python3
import json,pathlib,sys
args=sys.argv
request=json.loads(pathlib.Path(args[args.index('--export-pdf')+1]).read_text())
assert isinstance(request['options'],dict)
assert request['project']==str(pathlib.Path(request['project']).resolve())
output=pathlib.Path(request['destination'])
output.write_bytes(b'%PDF-1.7\nprotocol fixture\n')
receipt={'schema':'deck.pdf-export-result/1','status':'completed','project':request['project'],'source':request['source'],'output':{'kind':'pdf','path':str(output),'byte_length':output.stat().st_size,'media_type':'application/pdf'}}
pathlib.Path(args[args.index('--result')+1]).write_text(json.dumps(receipt))
").unwrap();
    fs::set_permissions(&writer, fs::Permissions::from_mode(0o700)).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_deck-cli"))
        .arg("print-pdf")
        .arg(&fixture.project)
        .arg(&request)
        .arg("--desktop")
        .arg(&writer)
        .output()
        .unwrap();
    let result = decode(&output, true);
    assert_eq!(result["independently_verified"], true);
    let hashed = decode(&fixture.raw("hash-file", &destination, None, &[]), true);
    assert_eq!(result["output"]["blake3"], hashed["blake3"]);
    let second = std::process::Command::new(env!("CARGO_BIN_EXE_deck-cli"))
        .arg("print-pdf")
        .arg(&fixture.project)
        .arg(request)
        .arg("--desktop")
        .arg(writer)
        .output()
        .unwrap();
    assert_eq!(decode(&second, false)["code"], "destination_exists");
}
