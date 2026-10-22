//! Deck project admission and direct use of the existing `CardKit` actions.
use crate::{Result, fail, files::ProjectStage};
use serde::Deserialize;
use serde_json::{Value, json};
use shape_core::ShapeProject;
use shape_domain::{
    ArtifactKind, CARD_KIT_DATA_TYPE, CARD_KIT_OPERATOR_TYPE, CardKitRequest, ContentDigest,
    MaterialEditConfiguration, WorkingInput,
};
use std::path::Path;

pub const SHAPE_REVISION: &str = "983022bf35bcfda332ef57aeab81d8015cf5f280";

pub fn status(path: &Path) -> Result<Value> {
    let state = ShapeProject::inspect(path)?;
    let mut unsupported_artifacts = Vec::new();
    let mut unsupported_operators = Vec::new();
    for artifact in &state.snapshot.artifacts {
        if !matches!(
            artifact.kind,
            ArtifactKind::CardKit | ArtifactKind::ImageRaster | ArtifactKind::TextDocument
        ) {
            unsupported_artifacts.push(json!({"artifact_id":artifact.id,"kind":artifact.kind}));
        }
    }
    for graph in &state.working_graphs {
        let kind = state
            .snapshot
            .artifacts
            .iter()
            .find(|a| a.id == graph.context_artifact_id())
            .map(|a| a.kind);
        for node in graph.operators() {
            let card = kind == Some(ArtifactKind::CardKit)
                && node.operator_type().as_str() == CARD_KIT_OPERATOR_TYPE
                && node.output_data_type().as_str() == CARD_KIT_DATA_TYPE
                && graph.operators().len() == 1
                && node
                    .configuration()
                    .is_some_and(|c| CardKitRequest::from_configuration(c).is_ok());
            let shadow = kind == Some(ArtifactKind::ImageRaster)
                && MaterialEditConfiguration::from_draft(node).is_ok_and(|c| {
                    c.editor == shape_domain::MaterialEditor::Shadow && graph.operators().len() == 1
                });
            if !card && !shadow {
                unsupported_operators.push(json!({"artifact_id":graph.context_artifact_id(),"id":node.id(),"operator_type":node.operator_type()}));
            }
        }
    }
    let mut kits = Vec::new();
    for artifact in state
        .snapshot
        .artifacts
        .iter()
        .filter(|a| a.kind == ArtifactKind::CardKit)
    {
        let mut draft = Value::Null;
        if let Some(graph) = state
            .working_graphs
            .iter()
            .find(|g| g.context_artifact_id() == artifact.id)
            && let [node] = graph.operators()
            && node.operator_type().as_str() == CARD_KIT_OPERATOR_TYPE
            && let Some(composition) = node
                .configuration()
                .and_then(|c| CardKitRequest::from_configuration(c).ok())
        {
            let digest = ContentDigest::from_bytes(&serde_json::to_vec(graph)?).to_string();
            let mut blockers = Vec::new();
            if graph.expected_revision_id() != artifact.accepted_revision {
                blockers.push("output_head_changed");
            }
            let inputs = input_versions(&composition, &state.snapshot.artifacts, &mut blockers);
            draft = json!({"draft_digest":digest,"expected_head":graph.expected_revision_id(),"current_head":artifact.accepted_revision,
                "card_ids":composition.cards.iter().map(|c|&c.id).collect::<Vec<_>>(),"inputs":inputs,"blockers":blockers,
                "requests":{"load":{"action":"draft_load","artifact_id":artifact.id},
                    "check":{"action":"preview","artifact_id":artifact.id,"draft_digest":digest}},
                "accept_requires":["draft_digest from current load/save","review_digest from exact preview","explicit accept request"]});
        }
        kits.push(json!({"artifact_id":artifact.id,"name":artifact.name,
            "source":artifact.accepted_revision.map(|revision_id|WorkingInput{artifact_id:artifact.id,revision_id}),"draft":draft}));
    }
    let allowed = unsupported_artifacts.is_empty() && unsupported_operators.is_empty();
    let mut report = json!({"schema":"deck.project-status/1","read_only":true,
        "project":state.snapshot.metadata,"artifacts":state.snapshot.artifacts,"kits":kits,
        "open_guard":{"allowed":allowed,"needs_transfer":!allowed,"unsupported_artifacts":unsupported_artifacts,"unsupported_operators":unsupported_operators},
        "observed":{"scope":"saved_metadata","payloads_checked":false,"transient_candidates":"not_observed","runtime_checked":false},
        "shape_revision":SHAPE_REVISION});
    report["snapshot_digest"] = ContentDigest::from_bytes(&serde_json::to_vec(&report)?)
        .to_string()
        .into();
    Ok(report)
}

pub fn require_card_project(path: &Path) -> Result<()> {
    if status(path)?["open_guard"]["allowed"] != true {
        return Err(fail(
            "needs_transfer",
            "project contains non-card workflows; export the selected kit from Shape as a complete Deck transfer",
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    name: String,
    composition: CardKitRequest,
}
pub fn create(destination: &Path, request: &Path) -> Result<Value> {
    let request: Create = serde_json::from_slice(&crate::files::read(request, 96 * 1024)?)?;
    request
        .composition
        .validate()
        .map_err(|e| fail("invalid_composition", e))?;
    if !request.composition.inputs().is_empty() {
        return Err(fail(
            "create_inputs",
            "new projects cannot resolve foreign pins; import a transfer or import materials before saving",
        ));
    }
    let stage = ProjectStage::new(destination)?;
    let mut project = ShapeProject::create(&stage.path, request.name)?;
    let draft = project.create_card_kit_draft(&request.composition)?;
    let project_id = project.snapshot()?.metadata.id;
    drop(project);
    stage.publish()?;
    Ok(json!({"project_id":project_id,"project":destination.canonicalize()?,"draft":draft}))
}

pub fn card_action(path: &Path, request: &Path) -> Result<Value> {
    let bytes = crate::files::read(request, 96 * 1024)?;
    let text = std::str::from_utf8(&bytes)?;
    require_card_project(path)?;
    let mut project = ShapeProject::open(path)?;
    if serde_json::from_str::<Value>(text)?["action"] == "print_plan" {
        return crate::print::action(&project, text);
    }
    Ok(serde_json::from_str(&project.card_kit_action_json(text)?)?)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportRaster {
    name: String,
    path: std::path::PathBuf,
}
pub fn import_raster(path: &Path, request: &Path) -> Result<Value> {
    let request: ImportRaster = serde_json::from_slice(&crate::files::read(request, 64 * 1024)?)?;
    let bytes = crate::files::read(&request.path, 128 * 1024 * 1024)?;
    require_card_project(path)?;
    let mut project = ShapeProject::open(path)?;
    let revision = project.import_raster_bytes(bytes, request.name)?;
    Ok(
        json!({"source":WorkingInput{artifact_id:revision.artifact_id,revision_id:revision.id},"revision":revision}),
    )
}

fn input_versions(
    composition: &CardKitRequest,
    artifacts: &[shape_domain::Artifact],
    blockers: &mut Vec<&'static str>,
) -> Vec<Value> {
    composition.inputs().into_iter().map(|source| {
        let artifact = artifacts.iter().find(|a|a.id == source.artifact_id);
        let head = artifact.and_then(|a|a.accepted_revision);
        let status = match head {
            None => "missing",
            Some(head) if head == source.revision_id => "current",
            Some(_) => "changed",
        };
        let artwork = composition.uses_artwork(source);
        let text = composition.cards.iter().any(|c|c.text_material == Some(source));
        let expected_kind = match (artwork,text) {
            (true,false) => Some(ArtifactKind::ImageRaster),
            (false,true) => Some(ArtifactKind::TextDocument),
            _ => None,
        };
        let kind_status = match (artifact,expected_kind) {
            (_,None) => "ambiguous_role",
            (None,_) => "not_observed",
            (Some(a),Some(expected)) if a.kind == expected => "compatible",
            _ => "incompatible",
        };
        for (applies,code) in [
            (status == "missing","input_missing"),(status == "changed","input_head_changed"),
            (kind_status == "incompatible","input_kind_mismatch"),(kind_status == "ambiguous_role","input_role_ambiguous"),
        ] {
            if applies && !blockers.contains(&code) { blockers.push(code); }
        }
        json!({"source":source,"current_revision":head,"status":status,"expected_kind":expected_kind,
            "observed_kind":artifact.map(|a|a.kind),"kind_status":kind_status})
    }).collect()
}
