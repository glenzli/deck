//! Bounded local-file intake into existing text and audio creative values.
//! Source paths are never part of accepted history; the exact accepted bytes are.

use std::{fs::File, io::Read, path::Path};

use shape_domain::{
    Artifact, ArtifactContentContract, ArtifactKind, ArtifactRevision, IntentSpec, Transformation,
    TransformationKind,
};
use shape_execution::{
    AUDIO_IMPORT_CAPABILITY, AudioImportExecutor, CapabilityId, ExecutedCandidate,
    ExecutionCoordinator, ExecutionRequest,
};
use shape_store::NewArtifactCommit;

use super::ShapeProject;
use crate::CoreError;

const MAX_TEXT_SOURCE_BYTES: u64 = 1024 * 1024;
const MAX_AUDIO_SOURCE_BYTES: u64 = 128 * 1024 * 1024;

impl ShapeProject {
    /// Imports one UTF-8 text or code file as an accepted, editable text source.
    /// The file is never executed by Shape.
    /// # Errors
    /// Rejects invalid UTF-8, empty or oversized files, or failed publication.
    pub fn import_text_file(
        &mut self,
        source_path: impl AsRef<Path>,
        artifact_name: impl Into<String>,
    ) -> Result<ArtifactRevision, CoreError> {
        let bytes = read_bounded_file(source_path.as_ref(), MAX_TEXT_SOURCE_BYTES)?;
        let text = String::from_utf8(bytes).map_err(|_| CoreError::InvalidImportSource {
            maximum_bytes: MAX_TEXT_SOURCE_BYTES,
        })?;
        self.create_text_document(artifact_name, text)
    }

    /// Imports exact PCM S16 LE WAV bytes as a truthful external-origin audio clip.
    /// # Errors
    /// Rejects unsupported, empty or oversized WAV files before publication.
    pub fn import_audio_wav(
        &mut self,
        source_path: impl AsRef<Path>,
        artifact_name: impl Into<String>,
    ) -> Result<ArtifactRevision, CoreError> {
        let source = read_bounded_file(source_path.as_ref(), MAX_AUDIO_SOURCE_BYTES)?;
        let artifact = Artifact::new(artifact_name, ArtifactKind::AudioClip)?;
        let transformation = Transformation::new(
            TransformationKind::Import,
            artifact.id,
            Vec::new(),
            IntentSpec::new("Import external audio")?,
            Vec::new(),
            Vec::new(),
        )?;
        let request = ExecutionRequest::new(
            transformation.id,
            CapabilityId::new(AUDIO_IMPORT_CAPABILITY)?,
            Vec::new(),
            source,
            "audio/wav",
        )?;
        let ExecutedCandidate { output, receipt } =
            ExecutionCoordinator::execute(&AudioImportExecutor::new()?, &request)?;
        let Some(ArtifactContentContract::AudioClip(contract)) = output.content_contract else {
            return Err(CoreError::MissingAudioOutputContract);
        };
        Ok(self.store.accept_new_artifact(NewArtifactCommit {
            artifact,
            expected_input_heads: Vec::new(),
            transformation,
            receipt,
            output_bytes: output.bytes.into(),
            output_media_type: output.media_type,
            content_contract: Some(ArtifactContentContract::AudioClip(contract)),
        })?)
    }
}

fn read_bounded_file(path: &Path, maximum_bytes: u64) -> Result<Vec<u8>, CoreError> {
    let invalid = || CoreError::InvalidImportSource { maximum_bytes };
    let file = File::open(path).map_err(|_| invalid())?;
    let metadata = file.metadata().map_err(|_| invalid())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum_bytes {
        return Err(invalid());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).map_err(|_| invalid())?);
    file.take(maximum_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() as u64 != metadata.len() {
        return Err(invalid());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
