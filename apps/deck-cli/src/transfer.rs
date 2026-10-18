//! Atomic, retry-safe intake of one selected kit and all exact pinned material bytes.
use crate::{
    Result, fail,
    files::{self, ProjectStage},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shape_core::ShapeProject;
use shape_domain::{
    ArtifactContentContract, ArtifactId, CardKitRequest, ContentDigest, ContentRef, ProjectId,
    RevisionId, WorkingInput,
};
use std::{collections::BTreeSet, fs, path::Path};

const RECEIPT: &str = "deck-transfer.json";
const MAX_MANIFEST: u64 = 256 * 1024;
const MAX_MATERIALS: u64 = 128 * 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub producer: String,
    pub project_id: ProjectId,
    pub kit_id: ArtifactId,
    pub expected_head: Option<RevisionId>,
    pub draft_digest: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Material {
    source: WorkingInput,
    content: ContentRef,
    contract: Option<ArtifactContentContract>,
    relative_path: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    request_id: String,
    origin: Origin,
    composition: CardKitRequest,
    materials: Vec<Material>,
}
impl Manifest {
    fn validate(&self) -> Result<()> {
        if self.schema != "deck.card-transfer/1"
            || self.origin.producer != "shape"
            || self.request_id.is_empty()
            || self.request_id.len() > 160
            || self.request_id.chars().any(char::is_control)
            || self.materials.len() > 64
        {
            return Err(fail(
                "transfer_schema",
                "unsupported or unbounded Deck transfer identity",
            ));
        }
        self.origin.draft_digest.parse::<ContentDigest>()?;
        self.composition
            .validate()
            .map_err(|e| fail("invalid_composition", e))?;
        let key = |s: WorkingInput| (s.artifact_id, s.revision_id);
        let expected: BTreeSet<_> = self.composition.inputs().into_iter().map(key).collect();
        let supplied: BTreeSet<_> = self.materials.iter().map(|m| key(m.source)).collect();
        if expected != supplied || supplied.len() != self.materials.len() {
            return Err(fail(
                "transfer_materials",
                "materials must exactly cover composition inputs once each",
            ));
        }
        let mut total = 0_u64;
        let mut paths = BTreeSet::new();
        for material in &self.materials {
            let is_image = self
                .composition
                .cards
                .iter()
                .any(|c| c.artwork == Some(material.source));
            let is_text = self
                .composition
                .cards
                .iter()
                .any(|c| c.text_material == Some(material.source));
            if is_image == is_text
                || !paths.insert(&material.relative_path)
                || material.relative_path.len() > 240
            {
                return Err(fail(
                    "transfer_materials",
                    "material role and relative file path must be unambiguous",
                ));
            }
            let limit = if is_image { 20 * 1024 * 1024 } else { 8192 };
            if material.content.byte_length == 0 || material.content.byte_length > limit {
                return Err(fail(
                    "transfer_budget",
                    "material exceeds its image (20 MiB) or text (8 KiB) limit",
                ));
            }
            if is_image {
                if material.content.media_type != "image/png"
                    || !matches!(
                        material.contract,
                        Some(ArtifactContentContract::ImageRaster(_))
                    )
                {
                    return Err(fail(
                        "transfer_contract",
                        "artwork requires PNG bytes and an ImageRaster contract",
                    ));
                }
            } else if !material.content.media_type.starts_with("text/")
                || !matches!(
                    material.contract,
                    None | Some(ArtifactContentContract::TextDocument(_))
                )
            {
                return Err(fail(
                    "transfer_contract",
                    "rules require UTF-8 text and an optional text format contract",
                ));
            }
            total = total
                .checked_add(material.content.byte_length)
                .ok_or_else(|| fail("transfer_budget", "material byte sum overflow"))?;
            if total > MAX_MATERIALS {
                return Err(fail("transfer_budget", "transfer materials exceed 128 MiB"));
            }
        }
        Ok(())
    }
}

pub fn import(destination: &Path, path: &Path, expected: Option<&str>) -> Result<Value> {
    let bytes = files::read(path, MAX_MANIFEST)?;
    let digest = ContentDigest::from_bytes(&bytes);
    if let Some(expected) = expected {
        let expected: ContentDigest = expected.parse()?;
        if digest != expected {
            return Err(fail(
                "transfer_changed",
                "manifest differs from the requested BLAKE3",
            ));
        }
    }
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    manifest.validate()?;
    if fs::symlink_metadata(destination).is_ok() {
        return retry(destination, &digest.to_string());
    }
    let stage = ProjectStage::new(destination)?;
    let result = populate(&stage.path, &manifest, path, &digest.to_string())?;
    // Receipt is part of the same staged directory publication as SQLite and CAS.
    fs::write(
        stage.path.join(RECEIPT),
        serde_json::to_vec_pretty(&result)?,
    )?;
    stage.publish()?;
    Ok(result)
}

fn populate(stage: &Path, manifest: &Manifest, path: &Path, digest: &str) -> Result<Value> {
    let root = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut project = ShapeProject::create(stage, &manifest.composition.name)?;
    let project_id = project.snapshot()?.metadata.id;
    let mut composition = manifest.composition.clone();
    let mut mappings = Vec::with_capacity(manifest.materials.len());
    for (index, material) in manifest.materials.iter().enumerate() {
        let bytes =
            files::read_material(root, &material.relative_path, material.content.byte_length)?;
        if bytes.len() as u64 != material.content.byte_length
            || ContentDigest::from_bytes(&bytes) != material.content.digest
        {
            return Err(fail(
                "transfer_changed",
                "material bytes differ from their declared length or BLAKE3",
            ));
        }
        let revision = match &material.contract {
            Some(ArtifactContentContract::ImageRaster(_)) => {
                let revision = project
                    .import_raster_bytes(bytes, format!("Imported artwork {}", index + 1))?;
                if revision.content_contract != material.contract {
                    return Err(fail(
                        "transfer_contract",
                        "decoded image contract differs from the transfer",
                    ));
                }
                revision
            }
            None | Some(ArtifactContentContract::TextDocument(_)) => {
                if let Some(ArtifactContentContract::TextDocument(format)) = &material.contract
                    && !format.accepts(&bytes)
                {
                    return Err(fail(
                        "transfer_contract",
                        "text bytes do not satisfy their declared format",
                    ));
                }
                let text = String::from_utf8(bytes)?;
                if text
                    .chars()
                    .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                {
                    return Err(fail(
                        "transfer_contract",
                        "rules material contains unsupported control characters",
                    ));
                }
                project.create_text_document(format!("Imported text {}", index + 1), text)?
            }
            _ => return Err(fail("transfer_contract", "unsupported material contract")),
        };
        let local = WorkingInput {
            artifact_id: revision.artifact_id,
            revision_id: revision.id,
        };
        for card in &mut composition.cards {
            if card.artwork == Some(material.source) {
                card.artwork = Some(local);
            }
            if card.text_material == Some(material.source) {
                card.text_material = Some(local);
            }
        }
        mappings.push(json!({"source":material.source,"source_content":material.content,"source_contract":material.contract,
            "local_source":local,"local_content":revision.content,"local_contract":revision.content_contract}));
    }
    let draft = project.create_card_kit_draft(&composition)?;
    // Validate all local references without adopting the incomplete kit or requiring print readiness.
    let _ = project.propose_card_kit(draft.artifact_id, &draft.draft_digest)?;
    Ok(
        json!({"schema":"deck.transfer-import/1","project_id":project_id,"kit_id":draft.artifact_id,
        "request_id":manifest.request_id,"manifest_blake3":digest,"origin":manifest.origin,
        "draft":draft,"mappings":mappings,"already_imported":false,"accepted_kit":false}),
    )
}

fn retry(destination: &Path, digest: &str) -> Result<Value> {
    if fs::symlink_metadata(destination)?.file_type().is_symlink() {
        return Err(fail(
            "destination_exists",
            "project destination is a symbolic link",
        ));
    }
    let mut receipt: Value = serde_json::from_slice(
        &files::read(&destination.join(RECEIPT), MAX_MANIFEST).map_err(|_| {
            fail(
                "destination_exists",
                "destination exists without a matching Deck import receipt",
            )
        })?,
    )?;
    if receipt["schema"] != "deck.transfer-import/1" || receipt["manifest_blake3"] != digest {
        return Err(fail(
            "destination_exists",
            "destination belongs to another import; it was not changed",
        ));
    }
    let state = crate::project::status(destination)?;
    if receipt["project_id"] != state["project"]["id"]
        || !state["kits"]
            .as_array()
            .is_some_and(|kits| kits.iter().any(|k| k["artifact_id"] == receipt["kit_id"]))
    {
        return Err(fail(
            "transfer_identity",
            "import receipt does not identify this project's card kit",
        ));
    }
    receipt["already_imported"] = true.into();
    // Original imported draft remains evidence; current state is returned separately, never reset.
    receipt["current"] = state;
    Ok(receipt)
}
