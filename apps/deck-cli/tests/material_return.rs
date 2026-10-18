mod support;
use serde_json::{Value, json};
use shape_core::ShapeProject;
use shape_domain::{ContentDigest, WorkingInput};
use std::fs;
use support::{Fixture, png};

fn returned_task(fixture: &Fixture) -> (Value, Value, Value) {
    let created = fixture.create();
    let mut composition = created["draft"]["composition"].clone();
    composition["cards"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"other","rules":"Another card"}));
    let saved = fixture.run("card-kit",Some(&json!({"action":"draft_save","artifact_id":created["draft"]["artifact_id"],"draft_digest":created["draft"]["draft_digest"],"composition":composition})),true);
    let source_path = fixture.root.join("shape-source.shape");
    let mut source = ShapeProject::create(&source_path, "Returned material fixture").unwrap();
    let art = source.import_raster_bytes(png(2, 2), "Image").unwrap();
    let bytes = source.read_revision_content(art.id).unwrap().bytes;
    let output = fixture.root.join("returned.png");
    fs::write(&output, &bytes).unwrap();
    let task = json!({"schema":"deck.material-task/1","request_id":"task-1",
        "target":{"project_id":created["project_id"],"kit_id":saved["draft"]["artifact_id"],"card_id":"card-1",
            "draft_digest":saved["draft"]["draft_digest"],"artwork":null,"card_snapshot":saved["draft"]["composition"]["cards"][0]},
        "prompt":"A flowering tree.","canvas":{"width":750,"height":1050}});
    let task_path = fixture.write("task.json", &task);
    let digest = ContentDigest::from_bytes(&fs::read(&task_path).unwrap()).to_string();
    let response = json!({"schema":"deck.material-task-result/1","request_id":"task-1","request_blake3":digest,
        "outcome":"completed","output":{"path":output,"byte_length":bytes.len(),"blake3":ContentDigest::from_bytes(&bytes).to_string(),
            "media_type":"image/png","source":WorkingInput{artifact_id:art.artifact_id,revision_id:art.id},"contract":art.content_contract},"error":null});
    let response_path = fixture.write("result.json", &response);
    let import = json!({"task_path":task_path,"result_path":response_path,"expected_request_blake3":digest,"name":"Returned artwork"});
    (saved, import, response)
}

#[test]
fn exact_original_card_return_allows_other_card_edits_and_never_binds_or_accepts() {
    let fixture = Fixture::new();
    let (saved, import, response) = returned_task(&fixture);
    let mut edited = saved["draft"]["composition"].clone();
    edited["cards"][1]["rules"] = "Edit another card while waiting.".into();
    edited["cards"].as_array_mut().unwrap().swap(0, 1);
    let next = fixture.run("card-kit",Some(&json!({"action":"draft_save","artifact_id":saved["draft"]["artifact_id"],"draft_digest":saved["draft"]["draft_digest"],"composition":edited})),true);
    let before = fixture.run("status", None, true);
    let imported = fixture.run("import-material-return", Some(&import), true);
    assert_ne!(imported["source"], response["output"]["source"]);
    assert_eq!(imported["bound_to_card"], false);
    assert_eq!(imported["accepted_kit"], false);
    let after = fixture.run("status", None, true);
    assert_eq!(
        after["artifacts"].as_array().unwrap().len(),
        before["artifacts"].as_array().unwrap().len() + 1
    );
    assert_eq!(
        after["kits"][0]["draft"]["draft_digest"],
        next["draft"]["draft_digest"]
    );
    let loaded = fixture.run(
        "card-kit",
        Some(&json!({"action":"draft_load","artifact_id":saved["draft"]["artifact_id"]})),
        true,
    );
    assert!(loaded["draft"]["composition"]["cards"][1]["artwork"].is_null());
    assert_eq!(
        loaded["draft"]["composition"]["cards"][0]["rules"],
        "Edit another card while waiting."
    );
}

#[test]
fn changed_original_card_or_corrupt_receipt_rejects_before_creating_material() {
    let fixture = Fixture::new();
    let (saved, import, mut response) = returned_task(&fixture);
    let before = fixture.run("status", None, true);
    response["output"]["contract"]["contract"]["width"] = 4096.into();
    fixture.write("result.json", &response);
    let rejected = fixture.run("import-material-return", Some(&import), false);
    assert_eq!(rejected["code"], "material_contract");
    assert_eq!(fixture.run("status", None, true), before);
    response["output"]["contract"]["contract"]["width"] = 2.into();
    response["request_id"] = "another-task".into();
    fixture.write("result.json", &response);
    assert_eq!(
        fixture.run("import-material-return", Some(&import), false)["code"],
        "material_receipt"
    );
    assert_eq!(fixture.run("status", None, true), before);
    response["request_id"] = "task-1".into();
    fixture.write("result.json", &response);
    let mut edited = saved["draft"]["composition"].clone();
    edited["cards"][0]["rules"] = "Changed original card.".into();
    fixture.run("card-kit",Some(&json!({"action":"draft_save","artifact_id":saved["draft"]["artifact_id"],"draft_digest":saved["draft"]["draft_digest"],"composition":edited})),true);
    let changed = fixture.run("status", None, true);
    assert_eq!(
        fixture.run("import-material-return", Some(&import), false)["code"],
        "material_target_changed"
    );
    assert_eq!(fixture.run("status", None, true), changed);
    assert!(
        fixture.root.join("returned.png").is_file(),
        "manual import remains possible"
    );
}
