//! Atomic no-clobber offline material-kit export over one exact accepted version.
use std::{
    fmt::Write as _,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::json;
use shape_domain::{ArtifactKind, CardKitRequest, WorkingInput};
use zip::{ZipWriter, write::SimpleFileOptions};

use super::{
    CardKitError, Material, ShapeProject, invalid,
    layout::shows_artwork,
    preview::{ensure_export_ready, escape, render_card},
};

#[derive(Debug, Clone, Serialize)]
pub struct CardKitExport {
    pub schema: String,
    pub destination: PathBuf,
    pub output_blake3: String,
    pub byte_length: u64,
    pub cards: usize,
    pub copies: u64,
    pub print_ready: bool,
}

impl ShapeProject {
    /// Exports a complete accepted kit with local SVG/HTML previews and verified exact materials.
    /// Missing artwork/text or overflowing layout blocks publication. Historical pins stay historical.
    /// # Errors
    /// Rejects incomplete kits, corrupt pins, existing destinations, project-internal paths and I/O failure.
    pub fn export_card_kit(
        &self,
        source: WorkingInput,
        destination: &Path,
    ) -> Result<CardKitExport, CardKitError> {
        let request = self.read_card_kit(source)?;
        let materials = self.card_kit_materials(&request, false)?;
        ensure_export_ready(&request, &materials)?;
        let repeated_image_bytes = request
            .cards
            .iter()
            .filter(|c| shows_artwork(&request, c))
            .filter_map(|c| c.artwork)
            .map(|s| {
                materials
                    .iter()
                    .find(|m| m.source == s)
                    .map_or(0, |m| m.bytes.len())
            })
            .sum::<usize>();
        if repeated_image_bytes > 96 * 1024 * 1024 {
            return Err(invalid(
                "self-contained card previews exceed the 96 MiB artwork budget; split the kit or use smaller accepted artwork",
            ));
        }
        if destination.extension().and_then(|s| s.to_str()) != Some("zip") {
            return Err(invalid("card kit destination must end in .zip"));
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
        let temporary =
            Temporary(parent.join(format!(".shape-card-kit-{}.partial", uuid::Uuid::now_v7())));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = write_package(options.open(&temporary.0)?, source, &request, &materials)?;
        file.sync_all()?;
        drop(file);
        let mut file = File::open(&temporary.0)?;
        let byte_length = file.metadata()?.len();
        let mut hasher = blake3::Hasher::new();
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        fs::hard_link(&temporary.0, &destination)?;
        Ok(CardKitExport {
            schema: "shape.card-kit-export/1".into(),
            destination,
            output_blake3: hasher.finalize().to_hex().to_string(),
            byte_length,
            cards: request.cards.len(),
            copies: request.cards.iter().map(|c| u64::from(c.quantity)).sum(),
            print_ready: false,
        })
    }
}

fn write_package(
    file: File,
    source: WorkingInput,
    request: &CardKitRequest,
    materials: &[Material],
) -> Result<File, CardKitError> {
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let files = request
        .cards
        .iter()
        .enumerate()
        .map(|(i, c)| json!({"id":c.id,"svg":format!("cards/{:03}.svg",i+1),"quantity":c.quantity}))
        .collect::<Vec<_>>();
    let pins=materials.iter().map(|m|json!({"source":m.source,"content":m.content,"file":format!("materials/{}.{}",m.content.digest,if m.kind==ArtifactKind::ImageRaster{"png"}else{"txt"})})).collect::<Vec<_>>();
    archive.start_file("manifest.json", options)?;
    archive.write_all(&serde_json::to_vec_pretty(&json!({"schema":"shape.card-kit-package/1","source":source,"composition":request,"cards":files,"materials":pins,"output_kind":"card_material_kit","print_ready":false}))?)?;
    archive.start_file("README.txt", options)?;
    archive.write_all(b"Shape card material kit\nOpen index.html to inspect each card offline. Cards contain separate text and accepted artwork.\nThis is a reviewable card material kit, not print imposition, a game engine, or evidence that AI generated artwork.\nEach SVG preview embeds its artwork and remains viewable independently.\nThe manifest pins exact accepted revisions. Text and images in materials are exact verified source bytes.\n")?;
    let mut html = format!(
        r#"<!doctype html><html lang="en"><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src 'self' data:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"><title>{}</title><style>body{{font:16px system-ui;background:#eee9df;color:#29251e;margin:32px}}main{{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:24px}}figure{{margin:0;background:white;padding:16px;border-radius:12px}}img{{width:100%;max-height:650px;object-fit:contain}}figcaption{{margin-top:12px}}</style><h1>{}</h1><p>Offline card material kit. Review preview text and artwork before production. This package is not print-ready.</p><main>"#,
        escape(&request.name),
        escape(&request.name)
    );
    for (i, card) in request.cards.iter().enumerate() {
        let rendered = render_card(request, card, materials);
        if rendered.overflow {
            return Err(invalid("card layout changed during export"));
        }
        let path = format!("cards/{:03}.svg", i + 1);
        archive.start_file(&path, options)?;
        archive.write_all(rendered.svg.as_bytes())?;
        write!(html, r#"<figure><img src="{}" alt="{}"><figcaption>{} · {} copies · {}</figcaption></figure>"#,path,escape(&card.title),escape(&card.title),card.quantity,escape(&card.id)).expect("String writes are infallible");
    }
    html.push_str("</main></html>");
    archive.start_file("index.html", options)?;
    archive.write_all(html.as_bytes())?;
    let mut published = std::collections::BTreeSet::new();
    for material in materials {
        let suffix = if material.kind == ArtifactKind::ImageRaster {
            "png"
        } else {
            "txt"
        };
        let path = format!("materials/{}.{suffix}", material.content.digest);
        if published.insert(path.clone()) {
            archive.start_file(path, options)?;
            archive.write_all(&material.bytes)?;
        }
    }
    Ok(archive.finish()?)
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
