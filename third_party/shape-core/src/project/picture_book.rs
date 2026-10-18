//! Compose revisioned picture books over accepted materials and export Storybook snapshots.
//! Native reading layout lives in the desktop; Storybook retains its advanced publishing tools.
mod draft;
mod script;
pub use draft::{PictureBookDraft, PictureBookDraftAction};
mod version;
pub use version::PictureBookCandidate;

use super::ShapeProject;
use crate::CoreError;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shape_domain::{ArtifactKind, ContentRef, WorkingInput};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;
use zip::{ZipWriter, write::SimpleFileOptions};

const MAX_INPUT_BYTES: usize = 128 * 1024 * 1024;

pub use shape_domain::{PictureBookPage, PictureBookRequest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PictureBookExport {
    pub schema: String,
    pub destination: PathBuf,
    pub output_blake3: String,
    pub byte_length: u64,
    pub pages: usize,
    pub languages: usize,
}

#[derive(Debug, Error)]
pub enum PictureBookError {
    #[error("invalid picture-book composition: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("picture-book file operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("picture-book archive failed: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("picture-book JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Serialize)]
struct PinnedSource {
    source: WorkingInput,
    content: ContentRef,
}

#[derive(Debug)]
struct Composition {
    project: Value,
    origin: Value,
    assets: Vec<(String, Vec<u8>)>,
    inputs: Vec<PinnedSource>,
}

impl ShapeProject {
    /// Export a new `.scproj` for Storybook's human/AI composition and publication workflow.
    /// Inputs must be current accepted revisions; originals and Shape heads never change.
    /// # Errors
    /// Rejects ambiguous scripts, mismatched language roles, stale/wrong media, limits,
    /// existing destinations, internal project writes, or failure before publication.
    pub fn export_picture_book(
        &self,
        request: &PictureBookRequest,
        destination: &Path,
    ) -> Result<PictureBookExport, PictureBookError> {
        self.export_book_snapshot(request, destination, true)
    }

    fn export_book_snapshot(
        &self,
        request: &PictureBookRequest,
        destination: &Path,
        require_current: bool,
    ) -> Result<PictureBookExport, PictureBookError> {
        validate_request(request)?;
        if destination.extension().and_then(|s| s.to_str()) != Some("scproj") {
            return Err(invalid("destination must end in .scproj"));
        }
        let parent = destination
            .parent()
            .ok_or_else(|| invalid("destination needs a parent"))?
            .canonicalize()?;
        let destination = parent.join(
            destination
                .file_name()
                .ok_or_else(|| invalid("destination needs a file name"))?,
        );
        if destination.starts_with(self.store.root().canonicalize()?) || destination.try_exists()? {
            return Err(invalid(
                "destination must be a new file outside the Shape project",
            ));
        }
        let composition = self.compose_picture_book(request, require_current)?;
        let temporary =
            Temporary(parent.join(format!(".shape-book-{}.partial", uuid::Uuid::now_v7())));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut archive = ZipWriter::new(options.open(&temporary.0)?);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, value) in [
            ("project.json", &composition.project),
            ("shape-origin.json", &composition.origin),
        ] {
            archive.start_file(name, options)?;
            archive.write_all(&serde_json::to_vec_pretty(value)?)?;
        }
        for (name, bytes) in &composition.assets {
            archive.start_file(name, options)?;
            archive.write_all(bytes)?;
        }
        let file = archive.finish()?;
        file.sync_all()?;
        drop(file);
        if require_current {
            for input in &composition.inputs {
                self.verify_book_head(input.source)?;
            }
        }
        let (byte_length, output_blake3) = digest_file(&temporary.0)?;
        // Same-directory hard-link publication is atomic and refuses an existing target.
        fs::hard_link(&temporary.0, &destination)?;
        Ok(PictureBookExport {
            schema: "shape.picture-book-export/1".into(),
            destination,
            output_blake3,
            byte_length,
            pages: request.pages.len(),
            languages: request.languages.len(),
        })
    }

    fn verify_book_head(&self, source: WorkingInput) -> Result<(), PictureBookError> {
        let current = self
            .store
            .artifact(source.artifact_id)
            .map_err(CoreError::from)?;
        if current.as_ref().and_then(|a| a.accepted_revision) != Some(source.revision_id) {
            return Err(invalid(
                "an input head changed; choose its current version explicitly",
            ));
        }
        Ok(())
    }

    fn book_input(
        &self,
        source: WorkingInput,
        kind: ArtifactKind,
        inputs: &mut Vec<PinnedSource>,
        remaining: &mut usize,
        require_current: bool,
    ) -> Result<Vec<u8>, PictureBookError> {
        if require_current {
            self.verify_book_head(source)?;
        }
        let artifact = self
            .store
            .artifact(source.artifact_id)
            .map_err(CoreError::from)?
            .ok_or_else(|| invalid("input does not exist"))?;
        let revision = self.revision(source.revision_id)?;
        if artifact.kind != kind || revision.artifact_id != source.artifact_id {
            return Err(invalid("input media or artifact identity does not match"));
        }
        let size = usize::try_from(revision.content.byte_length)
            .map_err(|_| invalid("input too large"))?;
        *remaining = remaining
            .checked_sub(size)
            .ok_or_else(|| invalid("total input bytes exceed 128 MiB"))?;
        let content = self.read_revision_content(source.revision_id)?;
        if kind == ArtifactKind::ImageRaster && content.revision.content.media_type != "image/png" {
            return Err(invalid("image pages require Shape canonical PNG input"));
        }
        inputs.push(PinnedSource {
            source,
            content: content.revision.content,
        });
        Ok(content.bytes)
    }

    fn compose_picture_book(
        &self,
        request: &PictureBookRequest,
        require_current: bool,
    ) -> Result<Composition, PictureBookError> {
        let mut inputs = Vec::new();
        let mut remaining = MAX_INPUT_BYTES;
        let mut languages = BTreeMap::new();
        let mut parsed_languages = BTreeMap::new();
        let mut title_role = None;
        for (language, source) in &request.languages {
            let bytes = self.book_input(
                *source,
                ArtifactKind::TextDocument,
                &mut inputs,
                &mut remaining,
                require_current,
            )?;
            let text = String::from_utf8(bytes).map_err(|_| invalid("script must be UTF-8"))?;
            let parsed = script::parse(&text, request.pages.len())?;
            if title_role.is_some_and(|has_title| has_title != parsed.has_title) {
                return Err(invalid(
                    "all language scripts must agree whether a title page exists",
                ));
            }
            title_role = Some(parsed.has_title);
            let contributors = if parsed.author.is_empty() {
                vec![]
            } else {
                vec![json!({"role":"author", "name":parsed.author})]
            };
            let mut cover = text_settings(u32::from(request.font_size) * 3 / 2);
            if !parsed.author.is_empty() {
                // Storybook anchors both layers at the bottom; separate the cover credit.
                let lines = parsed.author.lines().count().max(1);
                cover["offset_y"] = json!(
                    -((i64::try_from(lines).map_err(|_| invalid("too many credit lines"))? * 24
                        + 24)
                        .max(64))
                );
            }
            languages.insert(language, json!({"script":text,
                "cover_text_settings":cover, "title_text_settings":text_settings(u32::from(request.font_size) * 5 / 4),
                "inner_text_settings":text_settings(u32::from(request.font_size)), "author_text_settings":text_settings(16),
                "page_text_overrides":{}, "publication_metadata": {"version":1,
                    "language":language,"title":parsed.page_text.get(&0),"contributors":contributors}}));
            parsed_languages.insert(language, parsed);
        }
        let mut visible = Vec::new();
        let mut page_settings = BTreeMap::new();
        let mut assets = Vec::new();
        for (index, page) in request.pages.iter().enumerate() {
            let (path, print_only) = match page {
                PictureBookPage::Image { source, print_only } => {
                    let bytes = self.book_input(
                        *source,
                        ArtifactKind::ImageRaster,
                        &mut inputs,
                        &mut remaining,
                        require_current,
                    )?;
                    let digest = shape_domain::ContentDigest::from_bytes(&bytes);
                    // Repeated artwork still receives distinct editable page identities.
                    let path = format!("images/{index:04}-{digest}.png");
                    assets.push((path.clone(), bytes));
                    (path, *print_only)
                }
                PictureBookPage::Blank { print_only } => {
                    (format!("blank://shape-{index}"), *print_only)
                }
            };
            if print_only {
                page_settings.insert(index.to_string(), json!({"print_only":true}));
            }
            visible.push(path);
        }
        let timestamp = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|_| invalid("timestamp out of range"))?;
        let project = json!({"schema_version":"20260907.01", "project_name":request.name,
            "last_modified":timestamp,"visible_images":visible,"trashed_images":[],
            "source_url_map":{},"default_language":request.default_language,"languages":languages,
            "image_adjustments":{},"canvas_width":request.canvas_width,"canvas_height":request.canvas_height,
            "page_settings":page_settings});
        let origin = json!({"schema":"shape.picture-book-origin/1", "project_id":self.snapshot()?.metadata.id,
            "request":request,"inputs":inputs,"parsed_scripts":parsed_languages,
            "workflow":"accepted Shape materials -> editable Storybook composition -> explicit publication"});
        Ok(Composition {
            project,
            origin,
            assets,
            inputs,
        })
    }
}

fn text_settings(size: u32) -> Value {
    json!({"font_family":"sans-serif","font_size":size,"text_color":"#182a37","has_shadow":false,
        "has_backdrop":true,"readability_mode":"wash","readability_strength":55,"max_width_percent":90,
        "text_effects":{"outline":0,"halo":0,"backdrop":"wash","strength":55,"seed":1,"roughness":35,"feather":80,"color":"#ffffff"},
        "offset_x":0,"offset_y":0,"paper_alignment":"left","auto_snap_content":true})
}
fn invalid(message: &'static str) -> PictureBookError {
    PictureBookError::Invalid(message)
}
fn validate_request(r: &PictureBookRequest) -> Result<(), PictureBookError> {
    r.validate().map_err(invalid)
}

fn digest_file(path: &Path) -> Result<(u64, String), std::io::Error> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok((len, hasher.finalize().to_hex().to_string()))
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests;
