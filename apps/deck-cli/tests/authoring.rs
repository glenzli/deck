mod support;
use serde_json::{Value, json};
use shape_core::ShapeProject;
use shape_domain::{
    Artifact, ArtifactKind, ArtifactWorkingGraph, OperatorDataTypeId, OperatorTypeId, RasterCrop,
    WorkingInput,
};
use std::fs;
use support::{Fixture, png};

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "One independent-process lifecycle proves discovery, saved draft CAS, explicit acceptance, historical read and export use the same source"
)]
fn ai_follows_discovered_requests_and_reopens_exact_accepted_versions() {
    let fixture = Fixture::new();
    let caps = fixture.run("capabilities", None, true);
    assert_eq!(
        caps["shape_revision"],
        "983022bf35bcfda332ef57aeab81d8015cf5f280"
    );
    assert_eq!(caps["boundaries"]["runtime_checked"], false);
    assert_eq!(
        caps["commands"]["import-material-return"]["request"]["task_path"],
        "<original deck.material-task/1 JSON>"
    );
    assert_eq!(
        caps["commands"]["print-pdf"]["request"]["schema"],
        "deck.pdf-export/1"
    );
    let created = fixture.run("create", Some(&caps["commands"]["create"]["request"]), true);
    let initial_status = fixture.run("status", None, true);
    assert_eq!(initial_status["open_guard"]["allowed"], true);
    assert!(initial_status["kits"][0]["source"].is_null());
    assert_eq!(fixture.run("status", None, true), initial_status);
    let load_request = initial_status["kits"][0]["draft"]["requests"]["load"].clone();
    let loaded = fixture.run("card-kit", Some(&load_request), true);
    assert_eq!(loaded["draft"], created["draft"]);
    let mut composition = loaded["draft"]["composition"].clone();
    composition["cards"][0]["rules"] = "SPRING\n春".into();
    composition["layout"]["text_scale_percent"] = 200.into();
    let mut save = caps["commands"]["card-kit"]["requests"]["save"].clone();
    save["artifact_id"] = created["draft"]["artifact_id"].clone();
    save["draft_digest"] = loaded["draft"]["draft_digest"].clone();
    save["composition"] = composition.clone();
    let saved = fixture.run("card-kit", Some(&save), true);
    fixture.run("card-kit", Some(&save), false);
    let status = fixture.run("status", None, true);
    assert_eq!(
        status["kits"][0]["draft"]["draft_digest"],
        saved["draft"]["draft_digest"]
    );
    let preview = fixture.run(
        "card-kit",
        Some(&status["kits"][0]["draft"]["requests"]["check"]),
        true,
    );
    assert_eq!(preview["preview"]["ready_to_export"], true);
    assert!(
        preview["preview"]["svg"]
            .as_str()
            .unwrap()
            .contains("SPRING")
    );
    let mut accept = caps["commands"]["card-kit"]["requests"]["accept"].clone();
    accept["artifact_id"] = created["draft"]["artifact_id"].clone();
    accept["draft_digest"] = saved["draft"]["draft_digest"].clone();
    accept["review_digest"] = "incorrect".into();
    fixture.run("card-kit", Some(&accept), false);
    assert!(fixture.run("status", None, true)["kits"][0]["source"].is_null());
    accept["review_digest"] = preview["review_digest"].clone();
    let accepted = fixture.run("card-kit", Some(&accept), true);
    assert_ne!(
        accepted["draft"]["draft_digest"],
        saved["draft"]["draft_digest"]
    );
    let source = accepted["source"].clone();
    let read = fixture.run(
        "card-kit",
        Some(&json!({"action":"read","source":source})),
        true,
    );
    assert_eq!(read["composition"], composition);

    let mut next = accepted["draft"]["composition"].clone();
    next["cards"][0]["rules"] = "SUMMER\n夏".into();
    let saved_next = fixture.run("card-kit",Some(&json!({"action":"draft_save","artifact_id":accepted["draft"]["artifact_id"],"draft_digest":accepted["draft"]["draft_digest"],"composition":next})),true);
    let preview_next = fixture.run("card-kit",Some(&json!({"action":"preview","artifact_id":saved_next["draft"]["artifact_id"],"draft_digest":saved_next["draft"]["draft_digest"]})),true);
    let accepted_next = fixture.run("card-kit",Some(&json!({"action":"accept","artifact_id":saved_next["draft"]["artifact_id"],"draft_digest":saved_next["draft"]["draft_digest"],"review_digest":preview_next["review_digest"]})),true);
    assert_ne!(accepted_next["source"], source);
    let old = fixture.run(
        "card-kit",
        Some(&json!({"action":"read","source":source})),
        true,
    );
    assert_eq!(old["composition"], composition);
    let plan = fixture.run(
        "card-kit",
        Some(&json!({"action":"print_plan","source":source})),
        true,
    );
    assert_eq!(plan["print_plan"]["source"], source);
    assert_eq!(plan["print_plan"]["page_count"], 1);
    let destination = fixture.root.join("cards.zip");
    let export_request = json!({"action":"export","source":source,"destination":destination});
    let exported = fixture.run("card-kit", Some(&export_request), true);
    let hashed = support::decode(&fixture.raw("hash-file", &destination, None, &[]), true);
    assert_eq!(hashed["blake3"], exported["output_blake3"]);
    assert_eq!(hashed["byte_length"], exported["byte_length"]);
    let before = fs::read(&destination).unwrap();
    fixture.run("card-kit", Some(&export_request), false);
    assert_eq!(fs::read(destination).unwrap(), before);
}

#[test]
fn discovery_reports_stale_material_pins_without_refreshing_or_accepting() {
    let fixture = Fixture::new();
    let created = fixture.create();
    let image_path = fixture.root.join("image.png");
    fs::write(&image_path, png(2, 1)).unwrap();
    let imported = fixture.run(
        "import-raster",
        Some(&json!({"name":"Art","path":image_path})),
        true,
    );
    let source: WorkingInput = serde_json::from_value(imported["source"].clone()).unwrap();
    let mut composition = created["draft"]["composition"].clone();
    composition["cards"][0]["artwork"] = imported["source"].clone();
    let saved = fixture.run("card-kit",Some(&json!({"action":"draft_save","artifact_id":created["draft"]["artifact_id"],"draft_digest":created["draft"]["draft_digest"],"composition":composition})),true);
    let mut project = ShapeProject::open(&fixture.project).unwrap();
    let crop = project
        .propose_raster_crop(
            source.artifact_id,
            source.revision_id,
            RasterCrop::new(0, 0, 1, 1, 2, 1).unwrap(),
        )
        .unwrap();
    let newer = project.accept_raster_crop(crop).unwrap();
    drop(project);
    let status = fixture.run("status", None, true);
    let draft = &status["kits"][0]["draft"];
    assert_eq!(draft["inputs"][0]["source"], imported["source"]);
    assert_eq!(draft["inputs"][0]["current_revision"], json!(newer.id));
    assert_eq!(draft["inputs"][0]["status"], "changed");
    assert_eq!(draft["blockers"], json!(["input_head_changed"]));
    assert_eq!(draft["draft_digest"], saved["draft"]["draft_digest"]);
    fixture.run("card-kit", Some(&draft["requests"]["check"]), false);
    assert_eq!(fixture.run("status", None, true), status);
}

#[test]
fn mixed_project_guard_is_read_only_and_keeps_unconfigured_speech_untouched() {
    let fixture = Fixture::new();
    let mut project = ShapeProject::create(&fixture.project, "Mixed source").unwrap();
    let text = project
        .create_text_document("Script", "A spoken line.")
        .unwrap();
    let output = Artifact::new("Speech", ArtifactKind::AudioClip).unwrap();
    let mut graph = ArtifactWorkingGraph::new_source(output.id);
    graph
        .add_bound_operator(
            OperatorTypeId::new("audio.speech_synthesize").unwrap(),
            OperatorDataTypeId::new("text.document").unwrap(),
            OperatorDataTypeId::new("audio.clip").unwrap(),
            WorkingInput {
                artifact_id: text.artifact_id,
                revision_id: text.id,
            },
        )
        .unwrap();
    project
        .create_source_artifact_draft(&output, &graph)
        .unwrap();
    drop(project);
    let database = fs::read(fixture.project.join("project.sqlite")).unwrap();
    let manifest = fs::read(fixture.project.join("manifest.json")).unwrap();
    let status = fixture.run("status", None, true);
    assert_eq!(status["read_only"], true);
    assert_eq!(status["open_guard"]["allowed"], false);
    assert_eq!(status["open_guard"]["needs_transfer"], true);
    assert_eq!(
        status["open_guard"]["unsupported_operators"][0]["operator_type"],
        "audio.speech_synthesize"
    );
    let rejected = fixture.run("card-kit", Some(&json!({"action":"draft_list"})), false);
    assert_eq!(rejected["code"], "needs_transfer");
    assert_eq!(
        fs::read(fixture.project.join("project.sqlite")).unwrap(),
        database
    );
    assert_eq!(
        fs::read(fixture.project.join("manifest.json")).unwrap(),
        manifest
    );
    let inspection = ShapeProject::inspect(&fixture.project).unwrap();
    assert_eq!(inspection.working_graphs[0], graph);
}

#[test]
fn unavailable_and_old_schema_status_never_creates_or_migrates_projects() {
    let fixture = Fixture::new();
    fixture.run("status", None, false);
    assert!(!fixture.project.exists());
    let _ = fixture.create();
    let path = fixture.project.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["metadata"]["schema_revision"] = "20260810.1".into();
    let old = serde_json::to_vec(&manifest).unwrap();
    fs::write(&path, &old).unwrap();
    let database = fs::read(fixture.project.join("project.sqlite")).unwrap();
    fixture.run("status", None, false);
    assert_eq!(fs::read(path).unwrap(), old);
    assert_eq!(
        fs::read(fixture.project.join("project.sqlite")).unwrap(),
        database
    );
}

#[test]
fn status_distinguishes_missing_material_from_an_existing_wrong_media_pin() {
    let fixture = Fixture::new();
    let created = fixture.create();
    let mut project = ShapeProject::open(&fixture.project).unwrap();
    let text = project
        .create_text_document("Text material", "This is not artwork.")
        .unwrap();
    drop(project);
    let mut composition = created["draft"]["composition"].clone();
    composition["cards"][0]["artwork"] = json!(WorkingInput {
        artifact_id: text.artifact_id,
        revision_id: text.id
    });
    let saved = fixture.run("card-kit",Some(&json!({"action":"draft_save","artifact_id":created["draft"]["artifact_id"],"draft_digest":created["draft"]["draft_digest"],"composition":composition})),true);
    let status = fixture.run("status", None, true);
    assert_eq!(
        status["kits"][0]["draft"]["blockers"],
        json!(["input_kind_mismatch"])
    );
    let mut missing = saved["draft"]["composition"].clone();
    missing["cards"][0]["artwork"] = json!(WorkingInput {
        artifact_id: shape_domain::ArtifactId::new(),
        revision_id: shape_domain::RevisionId::new()
    });
    fixture.run("card-kit",Some(&json!({"action":"draft_save","artifact_id":created["draft"]["artifact_id"],"draft_digest":saved["draft"]["draft_digest"],"composition":missing})),true);
    let status = fixture.run("status", None, true);
    assert_eq!(
        status["kits"][0]["draft"]["blockers"],
        json!(["input_missing"])
    );
    assert!(status["kits"][0]["source"].is_null());
    fixture.run(
        "card-kit",
        Some(&status["kits"][0]["draft"]["requests"]["check"]),
        false,
    );
}
