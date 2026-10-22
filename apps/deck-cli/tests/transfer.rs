mod support;
use serde_json::{Value, json};
use shape_core::ShapeProject;
use shape_domain::{CardKitRequest, ContentDigest, WorkingInput};
use std::{fs, path::Path, process::Command};
use support::{Fixture, decode, png};

fn manifest(fixture: &Fixture, legacy: bool) -> Value {
    let project_path = fixture.root.join("source.shape");
    let mut project = ShapeProject::create(&project_path, "Shape source").unwrap();
    let art = project.import_raster_bytes(png(3, 2), "Artwork").unwrap();
    let text = project
        .create_text_document("Rules", "Private source text · 春")
        .unwrap();
    let mut composition = json!({"name":"Transferred kit","brief":"Keep all direction","style":"Unchanged ink","cards":[{"id":"stable-one","title":"Title","art_prompt":"Keep this unexecuted prompt","artwork":{"artifact_id":art.artifact_id,"revision_id":art.id},"text_material":{"artifact_id":text.artifact_id,"revision_id":text.id}},{"id":"unfinished","rules":""}]});
    if !legacy {
        composition["layout"] = json!({"mode":"text"});
    }
    let composition: CardKitRequest = serde_json::from_value(composition).unwrap();
    let draft = project.create_card_kit_draft(&composition).unwrap();
    let project_id = project.snapshot().unwrap().metadata.id;
    let materials = [&art,&text].into_iter().enumerate().map(|(i,revision)|{
        let content = project.read_revision_content(revision.id).unwrap();
        let relative = if i == 0 {"materials/001.png"} else {"materials/002.txt"};
        fs::create_dir_all(fixture.root.join("materials")).unwrap();
        fs::write(fixture.root.join(relative),content.bytes).unwrap();
        json!({"source":WorkingInput{artifact_id:revision.artifact_id,revision_id:revision.id},
            "content":revision.content,"contract":revision.content_contract,"relative_path":relative})
    }).collect::<Vec<_>>();
    json!({"schema":"deck.card-transfer/1","request_id":"fixture-transfer",
        "origin":{"producer":"shape","project_id":project_id,"kit_id":draft.artifact_id,"expected_head":null,"draft_digest":draft.draft_digest},
        "composition":composition,"materials":materials})
}
fn import(fixture: &Fixture, destination: &Path, value: &Value, success: bool) -> Value {
    let path = fixture.write("manifest.json", value);
    let digest = ContentDigest::from_bytes(&fs::read(&path).unwrap()).to_string();
    let output = Command::new(env!("CARGO_BIN_EXE_deck-cli"))
        .arg("import-transfer")
        .arg(destination)
        .arg(path)
        .args(["--expected-blake3", &digest])
        .output()
        .unwrap();
    decode(&output, success)
}
fn no_stage(fixture: &Fixture) {
    assert!(!fs::read_dir(&fixture.root).unwrap().any(|p| {
        p.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".deck-import-")
    }));
}

#[test]
fn imports_complete_hidden_materials_and_retries_without_resetting_later_edits() {
    let fixture = Fixture::new();
    let value = manifest(&fixture, false);
    let original_db = fs::read(fixture.root.join("source.shape/project.sqlite")).unwrap();
    let imported = import(&fixture, &fixture.project, &value, true);
    assert_eq!(imported["accepted_kit"], false);
    assert_eq!(
        imported["draft"]["composition"]["cards"][0]["id"],
        "stable-one"
    );
    assert_eq!(
        imported["draft"]["composition"]["cards"][1]["id"],
        "unfinished"
    );
    assert_eq!(imported["draft"]["composition"]["style"], "Unchanged ink");
    assert_eq!(imported["mappings"].as_array().unwrap().len(), 2);
    assert_ne!(
        imported["draft"]["composition"]["cards"][0]["artwork"],
        value["composition"]["cards"][0]["artwork"]
    );
    assert_eq!(
        imported["mappings"][0]["source_content"],
        imported["mappings"][0]["local_content"]
    );
    let status = fixture.run("status", None, true);
    assert_eq!(status["artifacts"].as_array().unwrap().len(), 3);
    assert!(status["kits"][0]["source"].is_null());
    let repeated = import(&fixture, &fixture.project, &value, true);
    assert_eq!(repeated["already_imported"], true);
    assert_eq!(repeated["project_id"], imported["project_id"]);
    assert_eq!(repeated["kit_id"], imported["kit_id"]);
    let mut edited = imported["draft"]["composition"].clone();
    edited["cards"][0]["title"] = "Edited locally".into();
    let saved = fixture.run("card-kit",Some(&json!({"action":"draft_save","artifact_id":imported["kit_id"],"draft_digest":imported["draft"]["draft_digest"],"composition":edited})),true);
    let repeated = import(&fixture, &fixture.project, &value, true);
    assert_eq!(
        repeated["current"]["kits"][0]["draft"]["draft_digest"],
        saved["draft"]["draft_digest"]
    );
    let loaded = fixture.run(
        "card-kit",
        Some(&json!({"action":"draft_load","artifact_id":imported["kit_id"]})),
        true,
    );
    assert_eq!(
        loaded["draft"]["composition"]["cards"][0]["title"],
        "Edited locally"
    );
    let mut other = value.clone();
    other["request_id"] = "another-request".into();
    import(&fixture, &fixture.project, &other, false);
    assert_eq!(
        fs::read(fixture.root.join("source.shape/project.sqlite")).unwrap(),
        original_db
    );
    no_stage(&fixture);
}

#[test]
fn legacy_transfer_preserves_absent_layout_and_incomplete_work_without_acceptance() {
    let fixture = Fixture::new();
    let value = manifest(&fixture, true);
    let imported = import(&fixture, &fixture.project, &value, true);
    assert!(imported["draft"]["composition"].get("layout").is_none());
    assert_eq!(imported["draft"]["composition"]["cards"][1]["title"], "");
    let preview = fixture.run("card-kit",Some(&json!({"action":"preview","artifact_id":imported["kit_id"],"draft_digest":imported["draft"]["draft_digest"]})),true);
    assert_eq!(preview["preview"]["ready_to_export"], false);
    assert!(fixture.run("status", None, true)["kits"][0]["source"].is_null());
    no_stage(&fixture);
}

#[test]
fn shared_back_only_artwork_is_remapped_and_discovered_as_an_image() {
    let fixture = Fixture::new();
    let mut value = manifest(&fixture, false);
    let foreign_artwork = value["composition"]["cards"][0]["artwork"].clone();
    value["composition"]["cards"][0]["artwork"] = Value::Null;
    value["composition"]["back"] = json!({
        "mode":"image", "artwork":foreign_artwork,
        "image_fit":"cover", "background_color":"#F2F2F2"
    });
    let source_database = fs::read(fixture.root.join("source.shape/project.sqlite")).unwrap();
    let imported = import(&fixture, &fixture.project, &value, true);
    let back = &imported["draft"]["composition"]["back"];
    assert_eq!(back["mode"], "image");
    assert_eq!(back["image_fit"], "cover");
    assert_eq!(back["background_color"], "#F2F2F2");
    assert_ne!(back["artwork"], foreign_artwork);
    assert_eq!(back["artwork"], imported["mappings"][0]["local_source"]);
    assert_eq!(
        imported["mappings"][0]["source_content"],
        imported["mappings"][0]["local_content"]
    );
    assert!(imported["draft"]["composition"]["cards"][0]["artwork"].is_null());
    assert_eq!(imported["accepted_kit"], false);
    let status = fixture.run("status", None, true);
    let draft = &status["kits"][0]["draft"];
    let input = draft["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|input| input["source"] == back["artwork"])
        .unwrap();
    assert_eq!(input["status"], "current");
    assert_eq!(input["kind_status"], "compatible");
    assert_eq!(input["expected_kind"], "image_raster");
    assert!(
        !draft["blockers"]
            .as_array()
            .unwrap()
            .contains(&json!("input_role_ambiguous"))
    );
    assert_eq!(
        fs::read(fixture.root.join("source.shape/project.sqlite")).unwrap(),
        source_database
    );
    no_stage(&fixture);
}

#[test]
fn invalid_paths_digests_contracts_and_material_sets_never_publish_a_partial_project() {
    let fixture = Fixture::new();
    let value = manifest(&fixture, false);
    let mut missing = value.clone();
    missing["materials"].as_array_mut().unwrap().pop();
    let mut duplicate = value.clone();
    duplicate["materials"]
        .as_array_mut()
        .unwrap()
        .push(value["materials"][0].clone());
    let mut traversal = value.clone();
    traversal["materials"][0]["relative_path"] = "../outside.png".into();
    let mut wrong_length = value.clone();
    wrong_length["materials"][0]["content"]["byte_length"] = json!(21 * 1024 * 1024);
    let mut wrong_contract = value.clone();
    wrong_contract["materials"][0]["contract"]["contract"]["width"] = 4.into();
    let mut wrong_digest = value.clone();
    wrong_digest["materials"][0]["content"]["digest"] = json!(vec![0_u8; 32]);
    for (i, bad) in [
        missing,
        duplicate,
        traversal,
        wrong_length,
        wrong_contract,
        wrong_digest,
    ]
    .iter()
    .enumerate()
    {
        let destination = fixture.root.join(format!("bad-{i}.deck"));
        import(&fixture, &destination, bad, false);
        assert!(!destination.exists());
        no_stage(&fixture);
    }
    let path = fixture.write("manifest.json", &value);
    let output = Command::new(env!("CARGO_BIN_EXE_deck-cli"))
        .arg("import-transfer")
        .arg(&fixture.project)
        .arg(path)
        .args(["--expected-blake3", &"0".repeat(64)])
        .output()
        .unwrap();
    assert_eq!(decode(&output, false)["code"], "transfer_changed");
    assert!(!fixture.project.exists());
}

#[cfg(unix)]
#[test]
fn material_symlinks_and_special_request_files_are_rejected_before_consumption() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let value = manifest(&fixture, false);
    let original = fixture.root.join("materials/001.png");
    let elsewhere = fixture.root.join("elsewhere.png");
    fs::rename(&original, &elsewhere).unwrap();
    symlink(&elsewhere, &original).unwrap();
    import(&fixture, &fixture.project, &value, false);
    assert!(!fixture.project.exists());
    let fifo = fixture.root.join("request.pipe");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_deck-cli"))
        .arg("create")
        .arg(&fixture.project)
        .arg(fifo)
        .output()
        .unwrap();
    assert_eq!(decode(&output, false)["code"], "file_not_regular");
    no_stage(&fixture);
}

#[test]
fn simultaneous_imports_publish_at_most_one_identity_and_retry_recovers_it() {
    let fixture = Fixture::new();
    let value = manifest(&fixture, false);
    let path = fixture.write("manifest.json", &value);
    let spawn = || {
        Command::new(env!("CARGO_BIN_EXE_deck-cli"))
            .arg("import-transfer")
            .arg(&fixture.project)
            .arg(&path)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    };
    let first = spawn();
    let second = spawn();
    let outputs = [
        first.wait_with_output().unwrap(),
        second.wait_with_output().unwrap(),
    ];
    let successes: Vec<_> = outputs
        .iter()
        .filter(|o| o.status.success())
        .map(|o| decode(o, true))
        .collect();
    assert!(!successes.is_empty());
    assert!(
        successes
            .iter()
            .all(|r| r["project_id"] == successes[0]["project_id"])
    );
    let repeated = import(&fixture, &fixture.project, &value, true);
    assert_eq!(repeated["project_id"], successes[0]["project_id"]);
    assert_eq!(
        fixture.run("status", None, true)["artifacts"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    no_stage(&fixture);
}
