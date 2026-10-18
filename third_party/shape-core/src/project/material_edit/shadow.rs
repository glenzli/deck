//! Strict adapter for Shadow's completed external-stage result; ordinary files remain manual returns.
use super::{
    CoreError, MaterialEditCandidate, MaterialEditHandoff, ShapeProject, files::read_regular,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use shape_domain::{
    ArtifactContentContract, ContentDigest, MaterialEditConfiguration, MaterialEditor,
    MaterialReturnNormalization, TransformationOperation,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResultFile {
    schema: String,
    request_id: String,
    input: Input,
    outcome: String,
    errors: Vec<Value>,
    artifact: Option<Artifact>,
    render: Option<Render>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    path: PathBuf,
    revision: String,
    sha256: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Artifact {
    path: PathBuf,
    sha256: String,
    byte_length: String,
    width: u32,
    height: u32,
    format: String,
    color_space: String,
    bit_depth: u8,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Render {
    session_id: String,
    photo_id: String,
    representation_id: String,
    recipe_commit_id: String,
    recipe_snapshot_digest: String,
}

impl ShapeProject {
    /// Prepares Shadow's native request over the exact exported source, without launching it.
    /// # Errors
    /// Rejects stale/foreign stages, changed copies and non-Shadow handoffs.
    pub fn shadow_material_request(
        &self,
        handoff: &MaterialEditHandoff,
    ) -> Result<Value, CoreError> {
        let ticket = &handoff.ticket;
        let configuration = MaterialEditConfiguration::from_draft(&ticket.draft)?;
        if handoff.schema != "shape.material-handoff/1"
            || configuration.editor != MaterialEditor::Shadow
            || handoff.input_blake3 != ticket.input_content.digest.to_string()
        {
            return Err(CoreError::InvalidMaterialEdit);
        }
        let source = self.material_edit_source(ticket)?;
        let exported = read_regular(&handoff.input_path, 64 * 1024 * 1024)?;
        if exported != source.bytes {
            return Err(CoreError::InvalidMaterialEdit);
        }
        let directory = handoff
            .input_path
            .parent()
            .ok_or(CoreError::InvalidMaterialEdit)?;
        if source.revision.content_contract.as_ref() != Some(&handoff.input_contract) {
            return Err(CoreError::InvalidMaterialEdit);
        }
        let (working, normalization) =
            shape_execution::RasterExecutor::materialize_shadow_input(&source.bytes)
                .map_err(|_| CoreError::InvalidMaterialEdit)?;
        let input = match (&handoff.input_materialization, normalization) {
            (None, MaterialReturnNormalization::Canonical) => handoff.input_path.clone(),
            (Some(recorded), known @ MaterialReturnNormalization::KnownSrgbIccInput { .. })
                if *recorded == known =>
            {
                directory.join("shadow-source.png")
            }
            _ => return Err(CoreError::InvalidMaterialEdit),
        };
        if read_regular(&input, 64 * 1024 * 1024)? != working {
            return Err(CoreError::InvalidMaterialEdit);
        }
        let request_digest = if handoff.input_materialization.is_some() {
            ContentDigest::from_bytes(&serde_json::to_vec(&(
                ticket,
                &handoff.input_materialization,
            ))?)
        } else {
            ContentDigest::from_bytes(&serde_json::to_vec(ticket)?)
        };
        Ok(json!({"schema":"shadow.external-edit/1",
            "requestId":format!("shape-{request_digest}"),
            "input":{"path":input,"revision":ticket.draft.input().ok_or(CoreError::InvalidMaterialEdit)?.revision_id.to_string(),
                "sha256":sha256(&working)}, "output":{"path":directory.join("edited.png")}}))
    }

    /// Verifies a completed native Shadow return before it can become a candidate.
    /// # Errors
    /// Rejects failed/cancelled/incomplete results, altered identities, hashes, paths, media,
    /// stale stages and a return changed since explicit review.
    pub fn propose_shadow_return(
        &self,
        handoff: &MaterialEditHandoff,
        result_path: &Path,
        expected_digest: Option<ContentDigest>,
    ) -> Result<MaterialEditCandidate, CoreError> {
        let request = self.shadow_material_request(handoff)?;
        let result: ResultFile = serde_json::from_slice(&read_regular(result_path, 128 * 1024)?)?;
        let invalid = || CoreError::InvalidMaterialEdit;
        if result.schema != "shadow.external-edit-result/1"
            || result.outcome != "completed"
            || !result.errors.is_empty()
            || result.request_id != request["requestId"]
            || result.input.path
                != Path::new(request["input"]["path"].as_str().ok_or_else(invalid)?)
            || result.input.revision != request["input"]["revision"]
            || result.input.sha256 != request["input"]["sha256"]
        {
            return Err(invalid());
        }
        let artifact = result.artifact.ok_or_else(invalid)?;
        let render = result.render.ok_or_else(invalid)?;
        if artifact.path != Path::new(request["output"]["path"].as_str().ok_or_else(invalid)?)
            || artifact.format != "png"
            || artifact.color_space != "sRGB"
            || artifact.bit_depth != 8
            || artifact.width == 0
            || artifact.height == 0
            || [
                &render.session_id,
                &render.photo_id,
                &render.representation_id,
                &render.recipe_commit_id,
            ]
            .iter()
            .any(|id| id.is_empty() || id.len() > 256 || id.chars().any(char::is_control))
            || !digest_string(&render.recipe_snapshot_digest)
            || !digest_string(&artifact.sha256)
        {
            return Err(invalid());
        }
        let bytes = read_regular(&artifact.path, 128 * 1024 * 1024)?;
        if artifact.byte_length != bytes.len().to_string() || artifact.sha256 != sha256(&bytes) {
            return Err(invalid());
        }
        let digest = expected_digest.unwrap_or_else(|| ContentDigest::from_bytes(&bytes));
        let mut candidate = self.propose_material_return(&handoff.ticket, bytes, digest)?;
        if let Some(normalization) = handoff.input_materialization
            && let Some(TransformationOperation::MaterialEdit(operation)) =
                &mut candidate.transformation.operation
        {
            operation.normalization = normalization;
        }
        let ArtifactContentContract::ImageRaster(contract) = candidate.contract() else {
            return Err(invalid());
        };
        if contract.width != artifact.width || contract.height != artifact.height {
            return Err(invalid());
        }
        Ok(candidate)
    }
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn digest_string(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests;
