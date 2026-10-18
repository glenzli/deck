//! Explicit working-copy export and bounded return-file reads for human and CLI consumers.
//! The handoff is external workspace state, never a second accepted-history owner.

use super::{CoreError, MaterialEditCandidate, MaterialEditTicket, ShapeProject};
use serde::{Deserialize, Serialize};
use shape_domain::{
    ArtifactContentContract, ContentDigest, MaterialEditConfiguration, MaterialReturnNormalization,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// A disposable copy plus portable origin ticket. Provider protocols remain unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialEditHandoff {
    pub schema: String,
    pub ticket: MaterialEditTicket,
    pub input_path: PathBuf,
    pub input_blake3: String,
    pub input_contract: ArtifactContentContract,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_materialization: Option<MaterialReturnNormalization>,
}

impl ShapeProject {
    /// Publishes source bytes and then a handoff manifest in a new external directory.
    /// Existing paths are never overwritten. A failed partial export remains recoverable;
    /// only a completed `handoff.json` is a usable handoff.
    /// # Errors
    /// Rejects stale tickets, existing destinations, internal project paths and I/O failure.
    pub fn export_material_edit(
        &self,
        ticket: &MaterialEditTicket,
        destination: &Path,
    ) -> Result<MaterialEditHandoff, CoreError> {
        let source = self.material_edit_source(ticket)?;
        let configuration = MaterialEditConfiguration::from_draft(&ticket.draft)?;
        let parent = destination
            .parent()
            .ok_or(CoreError::InvalidMaterialEdit)?
            .canonicalize()
            .map_err(io)?;
        let name = destination
            .file_name()
            .ok_or(CoreError::InvalidMaterialEdit)?;
        let destination = parent.join(name);
        if destination.starts_with(self.store.root().canonicalize().map_err(io)?) {
            return Err(CoreError::InvalidMaterialEdit);
        }
        fs::create_dir(&destination).map_err(io)?;
        let input_path = destination.join(
            if configuration.editor == shape_domain::MaterialEditor::Shadow {
                "source.png"
            } else {
                "source.wav"
            },
        );
        write_new(&input_path, &source.bytes)?;
        let mut handoff = MaterialEditHandoff {
            schema: "shape.material-handoff/1".into(),
            ticket: ticket.clone(),
            input_path,
            input_blake3: ticket.input_content.digest.to_string(),
            input_contract: source
                .revision
                .content_contract
                .ok_or(CoreError::InvalidMaterialEdit)?,
            input_materialization: None,
        };
        self.material_edit_source(ticket)?;
        if configuration.editor == shape_domain::MaterialEditor::Shadow {
            let (working, normalization) =
                shape_execution::RasterExecutor::materialize_shadow_input(&source.bytes)
                    .map_err(|_| CoreError::InvalidMaterialEdit)?;
            if matches!(
                normalization,
                MaterialReturnNormalization::KnownSrgbIccInput { .. }
            ) {
                write_new(&destination.join("shadow-source.png"), &working)?;
                handoff.input_materialization = Some(normalization);
            }
            write_new(
                &destination.join("shadow-request.json"),
                &serde_json::to_vec_pretty(&self.shadow_material_request(&handoff)?)?,
            )?;
        }
        write_new(
            &destination.join("handoff.json"),
            &serde_json::to_vec_pretty(&handoff)?,
        )?;
        Ok(handoff)
    }

    /// Reads an explicit selected handoff without trusting its output/source identities.
    /// # Errors
    /// Rejects non-regular/symlink/oversized manifests, unknown schema and invalid JSON.
    pub fn read_material_handoff(path: &Path) -> Result<MaterialEditHandoff, CoreError> {
        let handoff: MaterialEditHandoff =
            serde_json::from_slice(&read_regular(path, 128 * 1024)?)?;
        if handoff.schema != "shape.material-handoff/1"
            || handoff.input_blake3 != handoff.ticket.input_content.digest.to_string()
        {
            return Err(CoreError::InvalidMaterialEdit);
        }
        Ok(handoff)
    }

    /// Loads one bounded return file and binds the reviewed digest before decoding.
    /// # Errors
    /// Rejects stale/foreign tickets, non-regular files and changed/invalid returned bytes.
    pub fn propose_material_return_file(
        &self,
        ticket: &MaterialEditTicket,
        path: &Path,
        expected_digest: ContentDigest,
    ) -> Result<MaterialEditCandidate, CoreError> {
        self.material_edit_source(ticket)?;
        self.propose_material_return(
            ticket,
            read_regular(path, 128 * 1024 * 1024)?,
            expected_digest,
        )
    }

    /// Reviews selected bytes under an explicit precision-conversion policy.
    /// # Errors
    /// Rejects stale tickets, unsafe files, unsupported formats, or changed reviewed bytes.
    pub fn material_return_file_with_options(
        &self,
        ticket: &MaterialEditTicket,
        path: &Path,
        expected: Option<ContentDigest>,
        allow_pcm24_conversion: bool,
    ) -> Result<MaterialEditCandidate, CoreError> {
        self.material_edit_source(ticket)?;
        let bytes = read_regular(path, 128 * 1024 * 1024)?;
        let digest = expected.unwrap_or_else(|| ContentDigest::from_bytes(&bytes));
        self.propose_material_return_with_options(ticket, bytes, digest, allow_pcm24_conversion)
    }

    /// Takes an exact selected return-file snapshot for review. Callers carry its digest into Accept.
    /// # Errors
    /// Rejects unsupported files or stale stage bindings.
    pub fn review_material_return_file(
        &self,
        ticket: &MaterialEditTicket,
        path: &Path,
    ) -> Result<MaterialEditCandidate, CoreError> {
        self.material_edit_source(ticket)?;
        let bytes = read_regular(path, 128 * 1024 * 1024)?;
        let digest = ContentDigest::from_bytes(&bytes);
        self.propose_material_return(ticket, bytes, digest)
    }
}

pub(super) fn read_regular(path: &Path, maximum: u64) -> Result<Vec<u8>, CoreError> {
    if !fs::symlink_metadata(path)
        .map_err(io)?
        .file_type()
        .is_file()
    {
        return Err(CoreError::InvalidMaterialEdit);
    }
    #[cfg(unix)]
    let file = File::from(
        rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )
        .map_err(|e| io(e.into()))?,
    );
    #[cfg(not(unix))]
    let file = File::open(path).map_err(io)?;
    let metadata = file.metadata().map_err(io)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err(CoreError::InvalidMaterialEdit);
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len()).map_err(|_| CoreError::InvalidMaterialEdit)?,
    );
    file.take(maximum + 1).read_to_end(&mut bytes).map_err(io)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(CoreError::InvalidMaterialEdit);
    }
    Ok(bytes)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), CoreError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(io)?;
    file.write_all(bytes).map_err(io)?;
    file.sync_all().map_err(io)
}

fn io(error: std::io::Error) -> CoreError {
    CoreError::Store(shape_store::StoreError::Io(error))
}

#[cfg(test)]
mod tests;
