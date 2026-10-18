//! Delegate PDF painting to Deck's same Qt writer and independently verify its published bytes.
use crate::{Result, fail, files, project};
use serde::Deserialize;
use serde_json::{Value, json};
use shape_core::ShapeProject;
use shape_domain::WorkingInput;
use std::{
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    project: PathBuf,
    source: WorkingInput,
    #[serde(default)]
    options: shape_core::CardPrintRequestOptions,
    destination: PathBuf,
}

#[allow(
    clippy::too_many_lines,
    reason = "One native export lifecycle binds its request snapshot, process result and independent byte readback"
)]
pub fn export(path: &Path, request_path: &Path, desktop: Option<&Path>) -> Result<Value> {
    let request_bytes = files::read(request_path, 64 * 1024)?;
    let request: Request = serde_json::from_slice(&request_bytes)?;
    let canonical_project = path.canonicalize()?;
    let mut normalized: Value = serde_json::from_slice(&request_bytes)?;
    normalized["project"] = json!(canonical_project);
    if normalized.get("options").is_none() {
        normalized["options"] = json!({});
    }
    if request.schema != "deck.pdf-export/1"
        || !request.project.is_absolute()
        || !request.destination.is_absolute()
        || request.project.canonicalize()? != path.canonicalize()?
    {
        return Err(fail(
            "pdf_request",
            "PDF request must name this exact project and an absolute output path",
        ));
    }
    project::require_card_project(path)?;
    if fs::symlink_metadata(&request.destination).is_ok() {
        return Err(fail("destination_exists", "PDF destination already exists"));
    }
    let target_parent = request
        .destination
        .parent()
        .ok_or_else(|| fail("pdf_request", "PDF output needs a parent"))?
        .canonicalize()?;
    if target_parent.starts_with(path.canonicalize()?) {
        return Err(fail(
            "pdf_request",
            "PDF output must be outside the project bundle",
        ));
    }
    // Check the historical pin without constructing or duplicating the large print SVG plan.
    let core = ShapeProject::open(path)?;
    let _ = core.read_card_kit(request.source)?;
    drop(core);
    let executable = desktop
        .map(Path::to_path_buf)
        .or_else(|| env::var_os("DECK_DESKTOP_EXECUTABLE").map(PathBuf::from))
        .or_else(|| {
            env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("deck-desktop")))
        })
        .ok_or_else(|| {
            fail(
                "pdf_writer_unavailable",
                "select the Deck desktop executable",
            )
        })?;
    if !executable.is_file() {
        return Err(fail(
            "pdf_writer_unavailable",
            "Deck's native PDF writer is unavailable; print_plan alone is not a PDF",
        ));
    }
    let temp = ReceiptDirectory::new()?;
    let input_path = temp.0.join("request.json");
    let result_path = temp.0.join("result.json");
    // Snapshot the admitted exact request, rather than let a later file change alter the writer.
    fs::write(&input_path, serde_json::to_vec(&normalized)?)?;
    let mut child = Command::new(executable)
        .arg("--export-pdf")
        .arg(&input_path)
        .arg("--result")
        .arg(&result_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let started = Instant::now();
    let exit = loop {
        if let Some(exit) = child.try_wait()? {
            break exit;
        }
        if started.elapsed() > Duration::from_mins(5) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(fail(
                "pdf_timeout",
                "PDF writer timed out; inspect the output before retrying",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut receipt: Value =
        serde_json::from_slice(&files::read(&result_path, 256 * 1024).map_err(|_| {
            fail(
                "pdf_receipt",
                "PDF writer did not publish a bounded result receipt",
            )
        })?)?;
    if !exit.success()
        || receipt["schema"] != "deck.pdf-export-result/1"
        || receipt["status"] != "completed"
        || receipt["source"] != json!(request.source)
        || receipt["project"] != json!(canonical_project)
        || receipt["output"]["kind"] != "pdf"
        || receipt["output"]["media_type"] != "application/pdf"
    {
        return Err(fail(
            "pdf_receipt",
            "PDF writer did not confirm this exact project and accepted source",
        ));
    }
    let returned = receipt["output"]["path"]
        .as_str()
        .ok_or_else(|| fail("pdf_receipt", "PDF receipt lacks an output path"))?;
    if Path::new(returned).canonicalize()? != request.destination.canonicalize()? {
        return Err(fail("pdf_receipt", "PDF receipt names another output path"));
    }
    let verified = files::hash_report(&request.destination)?;
    if receipt["output"]["byte_length"] != verified["byte_length"]
        || receipt["output"]
            .get("blake3")
            .is_some_and(|digest| *digest != verified["blake3"])
    {
        return Err(fail(
            "pdf_changed",
            "PDF bytes differ from the native result receipt",
        ));
    }
    let mut magic = [0_u8; 5];
    fs::File::open(&request.destination)?.read_exact(&mut magic)?;
    if &magic != b"%PDF-" {
        return Err(fail(
            "pdf_invalid",
            "published output does not contain a PDF header",
        ));
    }
    receipt["output"]["blake3"] = verified["blake3"].clone();
    receipt["independently_verified"] = true.into();
    // Deserialize options even though the native owner remains the only planner/painter.
    let _ = request.options;
    Ok(receipt)
}

#[derive(Debug)]
struct ReceiptDirectory(PathBuf);
impl ReceiptDirectory {
    fn new() -> Result<Self> {
        let path = env::temp_dir().join(format!("deck-pdf-{}", uuid::Uuid::now_v7()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for ReceiptDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
