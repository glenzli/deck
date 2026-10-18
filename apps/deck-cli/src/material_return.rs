//! Verify one explicitly returned Shape image against its task and unchanged Deck card.
use crate::{Result, fail, files, project};
use serde::Deserialize;
use serde_json::{Value, json};
use shape_core::ShapeProject;
use shape_domain::{ArtifactContentContract, ArtifactId, ContentDigest, ProjectId, WorkingInput};
use shape_execution::{
    CapabilityId, ExecutionRequest, Executor, RASTER_IMPORT_CAPABILITY, RasterExecutor,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    task_path: PathBuf,
    result_path: PathBuf,
    expected_request_blake3: String,
    name: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Task {
    schema: String,
    request_id: String,
    target: Target,
    prompt: String,
    canvas: Canvas,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Canvas {
    width: u32,
    height: u32,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    project_id: ProjectId,
    kit_id: ArtifactId,
    card_id: String,
    draft_digest: String,
    artwork: Option<WorkingInput>,
    #[serde(default)]
    card_snapshot: Option<shape_domain::CardKitCard>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    schema: String,
    request_id: String,
    request_blake3: String,
    outcome: String,
    output: Option<Output>,
    error: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    path: PathBuf,
    byte_length: u64,
    blake3: String,
    media_type: String,
    source: WorkingInput,
    contract: ArtifactContentContract,
}

#[allow(
    clippy::too_many_lines,
    reason = "One ordered admission keeps task, returned bytes, exact card and final mutation checks together"
)]
pub fn import(path: &Path, request_path: &Path) -> Result<Value> {
    let request: Request = serde_json::from_slice(&files::read(request_path, 64 * 1024)?)?;
    if !request.task_path.is_absolute() || !request.result_path.is_absolute() {
        return Err(fail(
            "material_task",
            "task and result paths must be absolute",
        ));
    }
    let task_bytes = files::read(&request.task_path, 64 * 1024)?;
    let expected: ContentDigest = request.expected_request_blake3.parse()?;
    if ContentDigest::from_bytes(&task_bytes) != expected {
        return Err(fail(
            "material_task_changed",
            "material task differs from the captured request",
        ));
    }
    let task: Task = serde_json::from_slice(&task_bytes)?;
    if task.schema != "deck.material-task/1"
        || task.request_id.is_empty()
        || task.request_id.len() > 160
        || task.request_id.chars().any(char::is_control)
        || task.prompt.trim().is_empty()
        || task.prompt.len() > 16 * 1024
        || !(1..=4096).contains(&task.canvas.width)
        || !(1..=4096).contains(&task.canvas.height)
        || u64::from(task.canvas.width) * u64::from(task.canvas.height) > 16_777_216
    {
        return Err(fail(
            "material_task",
            "unsupported task identity, prompt or canvas",
        ));
    }
    if let Some(card) = &task.target.card_snapshot
        && (card.id != task.target.card_id
            || card.artwork != task.target.artwork
            || serde_json::to_vec(card)?.len() > 24 * 1024)
    {
        return Err(fail(
            "material_task",
            "card snapshot must match the captured stable id and artwork",
        ));
    }
    task.target.draft_digest.parse::<ContentDigest>()?;
    let response: Response =
        serde_json::from_slice(&files::read(&request.result_path, 128 * 1024)?)?;
    if response.schema != "deck.material-task-result/1"
        || response.request_id != task.request_id
        || response.request_blake3 != request.expected_request_blake3
        || response.outcome != "completed"
        || response.error.is_some()
    {
        return Err(fail(
            "material_receipt",
            "only a completed result for this exact task can be imported",
        ));
    }
    let output = response
        .output
        .ok_or_else(|| fail("material_receipt", "completed result has no image"))?;
    let ArtifactContentContract::ImageRaster(raster) = &output.contract else {
        return Err(fail(
            "material_contract",
            "material return requires a raster contract",
        ));
    };
    if !output.path.is_absolute()
        || output.media_type != "image/png"
        || output.byte_length > 20 * 1024 * 1024
        || output.byte_length == 0
        || raster.width == 0
        || raster.height == 0
        || raster.width > 4096
        || raster.height > 4096
        || u64::from(raster.width) * u64::from(raster.height) > 16_777_216
    {
        return Err(fail(
            "material_contract",
            "returned PNG exceeds the bounded image contract",
        ));
    }
    let bytes = files::read(&output.path, output.byte_length)?;
    let output_digest: ContentDigest = output.blake3.parse()?;
    if bytes.len() as u64 != output.byte_length
        || ContentDigest::from_bytes(&bytes) != output_digest
    {
        return Err(fail(
            "material_changed",
            "returned image bytes changed after review",
        ));
    }
    if bytes.len() < 24
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[12..16] != b"IHDR"
        || u32::from_be_bytes(bytes[16..20].try_into()?) != raster.width
        || u32::from_be_bytes(bytes[20..24].try_into()?) != raster.height
    {
        return Err(fail(
            "material_contract",
            "PNG header differs from its admitted raster dimensions",
        ));
    }
    // Reuse the existing bounded importer in memory before any live project mutation.
    let executor = RasterExecutor::new()?;
    let execution = ExecutionRequest::new(
        shape_domain::TransformationId::new(),
        CapabilityId::new(RASTER_IMPORT_CAPABILITY)?,
        vec![],
        bytes,
        "image/png",
    )?;
    let prepared = executor
        .execute(&execution)
        .map_err(|e| fail("material_contract", e.code))?;
    if prepared.content_contract.as_ref() != Some(&output.contract) {
        return Err(fail(
            "material_contract",
            "actual PNG interpretation differs from its returned contract",
        ));
    }
    project::require_card_project(path)?;
    let state = project::status(path)?;
    if state["project"]["id"] != json!(task.target.project_id) {
        return Err(fail(
            "material_target_changed",
            "this task belongs to another Deck project",
        ));
    }
    let mut core = ShapeProject::open(path)?;
    let drafts = core.card_kit_drafts()?;
    let draft = drafts
        .iter()
        .find(|d| d.artifact_id == task.target.kit_id)
        .ok_or_else(|| {
            fail(
                "material_target_changed",
                "the original saved card draft no longer exists",
            )
        })?;
    let card = draft
        .composition
        .cards
        .iter()
        .find(|c| c.id == task.target.card_id)
        .ok_or_else(|| {
            fail(
                "material_target_changed",
                "the original card no longer exists",
            )
        })?;
    let card_matches = task.target.card_snapshot.as_ref().map_or_else(
        || draft.draft_digest == task.target.draft_digest,
        |snapshot| snapshot == card,
    );
    if !card_matches || card.artwork != task.target.artwork {
        return Err(fail(
            "material_target_changed",
            "the card or its draft changed; choose the returned image explicitly instead of silently applying it",
        ));
    }
    let revision = core.import_raster_bytes(prepared.bytes, request.name)?;
    Ok(
        json!({"source":WorkingInput{artifact_id:revision.artifact_id,revision_id:revision.id},
        "revision":revision,"origin":{"producer":"shape","request_id":task.request_id,"request_blake3":request.expected_request_blake3,
        "source":output.source,"content_blake3":output.blake3},"bound_to_card":false,"accepted_kit":false}),
    )
}
