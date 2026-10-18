use std::{fs, sync::Mutex};

use shape_domain::IntentSpec;
use shape_execution::{ExecutionFailure, ExecutionOutput, ExecutorIdentity};
use uuid::Uuid;

use super::*;

struct FileTaskExecutor {
    identity: ExecutorIdentity,
    seen: Mutex<Vec<(Vec<u8>, Vec<u8>)>>,
    result: Vec<u8>,
}

impl FileTaskExecutor {
    fn new(result: &[u8]) -> Self {
        Self {
            identity: ExecutorIdentity::new("test.agent-file", "1", "test").unwrap(),
            seen: Mutex::new(Vec::new()),
            result: result.to_vec(),
        }
    }
}

impl Executor for FileTaskExecutor {
    fn identity(&self) -> &ExecutorIdentity {
        &self.identity
    }
    fn supports(&self, capability: &CapabilityId) -> bool {
        capability.as_str() == AGENT_FILE_CAPABILITY
    }
    fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionOutput, ExecutionFailure> {
        let bytes = request.inputs[0].bytes().unwrap().to_vec();
        self.seen
            .lock()
            .unwrap()
            .push((bytes, request.instruction.clone()));
        Ok(ExecutionOutput {
            bytes: self.result.clone(),
            media_type: request.output_media_type.clone(),
            executor_job_id: Some("agent_test_job".into()),
            external_provenance: None,
            content_contract: None,
        })
    }
}

fn root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("shape-agent-file-{label}-{}", Uuid::now_v7()))
}

#[test]
fn exact_accepted_bytes_remain_source_until_explicit_new_artifact_accept() {
    let root = root("accept");
    let mut project = ShapeProject::create(&root, "Agent task").unwrap();
    let source = project
        .create_text_document("scene.js", "const x = 1;\n")
        .unwrap();
    let executor = FileTaskExecutor::new(b"const x = 2;\n");
    let candidate = project
        .propose_agent_text_file(
            source.artifact_id,
            source.id,
            "scene-agent.js",
            "Change x to 2",
            &executor,
        )
        .unwrap();
    assert_eq!(candidate.text(), "const x = 2;\n");
    assert_eq!(
        executor.seen.lock().unwrap().as_slice(),
        &[(b"const x = 1;\n".to_vec(), b"Change x to 2".to_vec(),)]
    );
    assert_eq!(project.snapshot().unwrap().artifacts.len(), 1);
    let output = project.accept_agent_text_file(candidate).unwrap();
    assert_ne!(output.artifact_id, source.artifact_id);
    assert_eq!(
        project
            .read_accepted(source.artifact_id)
            .unwrap()
            .unwrap()
            .bytes,
        b"const x = 1;\n"
    );
    assert_eq!(
        project
            .read_accepted(output.artifact_id)
            .unwrap()
            .unwrap()
            .bytes,
        b"const x = 2;\n"
    );
    assert_eq!(
        project
            .transformation(output.transformation_id)
            .unwrap()
            .inputs,
        vec![source.id]
    );
    drop(project);
    assert_eq!(
        ShapeProject::open(&root)
            .unwrap()
            .snapshot()
            .unwrap()
            .artifacts
            .len(),
        2
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_source_or_invalid_output_cannot_be_accepted() {
    let root = root("stale");
    let mut project = ShapeProject::create(&root, "Agent task").unwrap();
    let source = project
        .create_text_document("source.txt", "Original")
        .unwrap();
    let executor = FileTaskExecutor::new(b"Result");
    let candidate = project
        .propose_agent_text_file(
            source.artifact_id,
            source.id,
            "result.txt",
            "Revise",
            &executor,
        )
        .unwrap();
    let edit = project
        .propose_text(
            source.artifact_id,
            Some(source.id),
            "Changed",
            IntentSpec::new("Edit source").unwrap(),
            Vec::new(),
        )
        .unwrap();
    project.accept_text(edit).unwrap();
    assert!(matches!(
        project.accept_agent_text_file(candidate),
        Err(CoreError::StaleCandidate { .. })
    ));
    let empty = FileTaskExecutor::new(b"  \n");
    let current = project
        .read_accepted(source.artifact_id)
        .unwrap()
        .unwrap()
        .revision
        .id;
    assert!(matches!(
        project.propose_agent_text_file(source.artifact_id, current, "empty.txt", "Revise", &empty),
        Err(CoreError::InvalidTextCandidate)
    ));
    assert_eq!(project.snapshot().unwrap().artifacts.len(), 1);
    fs::remove_dir_all(root).unwrap();
}

/// Explicitly opted-in local integration check; never runs in ordinary tests.
#[test]
#[ignore = "requires live Infer Agent ACL, managed credential and cloud task"]
fn live_synthetic_file_task_creates_reviewable_candidate_before_accept() {
    let credential = std::env::var("SHAPE_TEST_INFER_CREDENTIAL").expect("credential path env");
    let endpoint = std::env::var("SHAPE_TEST_INFER_ENDPOINT").unwrap_or_default();
    let marker = format!("SHAPE-AGENT-{}", Uuid::now_v7());
    let root = root("live");
    let mut project = ShapeProject::create(&root, "Synthetic Agent test").unwrap();
    let source = project
        .create_text_document("synthetic.txt", format!("Marker: {marker}\n"))
        .unwrap();
    let executor =
        shape_execution::InferRuntimeAgentFileExecutor::new(&endpoint, credential, "txt").unwrap();
    let candidate = project.propose_agent_text_file(
        source.artifact_id,
        source.id,
        "synthetic-result.txt",
        "Read the source file. Write the result file containing exactly the marker value, and nothing else.",
        &executor,
    ).unwrap();
    assert_eq!(project.snapshot().unwrap().artifacts.len(), 1);
    assert!(candidate.text().contains(&marker));
    assert!(candidate.receipt().executor_job_id.is_some());
    let provenance = candidate.receipt().external_provenance.as_ref().unwrap();
    println!(
        "Agent Job {} used {} / {}",
        candidate.receipt().executor_job_id.as_deref().unwrap(),
        provenance.deployment,
        provenance.model_build
    );
    if let Ok(expected_deployment) = std::env::var("SHAPE_TEST_INFER_EXPECTED_AGENT_DEPLOYMENT") {
        assert_eq!(provenance.deployment, expected_deployment);
    }
    let output = project.accept_agent_text_file(candidate).unwrap();
    assert_eq!(project.snapshot().unwrap().artifacts.len(), 2);
    assert_eq!(
        project
            .read_accepted(source.artifact_id)
            .unwrap()
            .unwrap()
            .bytes,
        format!("Marker: {marker}\n").into_bytes()
    );
    assert!(
        String::from_utf8(
            project
                .read_accepted(output.artifact_id)
                .unwrap()
                .unwrap()
                .bytes
        )
        .unwrap()
        .contains(&marker)
    );
    fs::remove_dir_all(root).unwrap();
}
