#![allow(
    dead_code,
    reason = "Shared public-command fixtures are used by different integration test executables"
)]
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
#[derive(Debug)]
pub struct Fixture {
    pub root: PathBuf,
    pub project: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        let root = std::env::temp_dir().join(format!("deck-cli-{}", uuid::Uuid::now_v7()));
        fs::create_dir(&root).unwrap();
        Self {
            project: root.join("cards.deck"),
            root,
        }
    }
    pub fn raw(
        &self,
        command: &str,
        project: &Path,
        request: Option<&Value>,
        extra: &[&str],
    ) -> Output {
        let mut process = Command::new(env!("CARGO_BIN_EXE_deck-cli"));
        process.arg(command);
        if command != "capabilities" {
            process.arg(project);
        }
        if let Some(request) = request {
            let path = self
                .root
                .join(format!("request-{}.json", uuid::Uuid::now_v7()));
            fs::write(&path, serde_json::to_vec(request).unwrap()).unwrap();
            process.arg(path);
        }
        process.args(extra).output().unwrap()
    }
    pub fn run(&self, command: &str, request: Option<&Value>, success: bool) -> Value {
        let output = self.raw(command, &self.project, request, &[]);
        decode(&output, success)
    }
    pub fn write(&self, name: &str, value: &Value) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        path
    }
    pub fn create(&self) -> Value {
        let capabilities = self.run("capabilities", None, true);
        self.run(
            "create",
            Some(&capabilities["commands"]["create"]["request"]),
            true,
        )
    }
    pub fn accepted(&self) -> Value {
        let created = self.create();
        let draft = &created["draft"];
        let preview = self.run("card-kit",Some(&json!({"action":"preview","artifact_id":draft["artifact_id"],"draft_digest":draft["draft_digest"]})),true);
        self.run("card-kit",Some(&json!({"action":"accept","artifact_id":draft["artifact_id"],"draft_digest":draft["draft_digest"],"review_digest":preview["review_digest"]})),true)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
pub fn decode(output: &Output, success: bool) -> Value {
    assert_eq!(
        output.status.success(),
        success,
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema"], "deck.cli.result/1");
    assert_eq!(value["status"], if success { "ok" } else { "error" });
    if success {
        value["result"].clone()
    } else {
        value
    }
}
pub fn png(width: u32, height: u32) -> Vec<u8> {
    use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(
            &[20, 80, 160, 255].repeat((width * height) as usize),
            width,
            height,
            ColorType::Rgba8.into(),
        )
        .unwrap();
    bytes
}
