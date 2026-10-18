//! Recoverable book editing; script checkpoints never create accepted Text revisions.
use super::{PictureBookCandidate, PictureBookError, ShapeProject, invalid};
use crate::CoreError;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shape_domain::{
    Artifact, ArtifactId, ArtifactKind, ArtifactWorkingGraph, ContentDigest, OperatorDataTypeId,
    OperatorTypeId, PictureBookDraftCheckpoint, PictureBookDraftState, RevisionId, WorkingInput,
    WorkingOperatorDraft,
};

#[derive(Debug, Clone, Serialize)]
pub struct PictureBookDraft {
    pub artifact_id: ArtifactId,
    pub expected_head: Option<RevisionId>,
    pub source: Option<WorkingInput>,
    pub draft_digest: String,
    pub state: PictureBookDraftState,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum PictureBookDraftAction {
    DraftNew {
        state: PictureBookDraftState,
    },
    DraftList,
    DraftLoad {
        artifact_id: ArtifactId,
    },
    DraftSave {
        artifact_id: ArtifactId,
        draft_digest: String,
        state: PictureBookDraftState,
    },
    DraftDiscard {
        artifact_id: ArtifactId,
        draft_digest: String,
    },
}

pub(super) fn book_node(
    graph: &ArtifactWorkingGraph,
) -> Result<&WorkingOperatorDraft, PictureBookError> {
    if graph.operators().len() != 1 {
        return Err(invalid("book workspace needs one composition draft"));
    }
    let node = &graph.operators()[0];
    if node.operator_type().as_str() != "book.compose"
        || node.output_data_type().as_str() != "book.picture"
    {
        return Err(invalid("artifact does not contain a book workspace draft"));
    }
    Ok(node)
}
pub(super) fn checkpoint(
    node: &WorkingOperatorDraft,
) -> Result<PictureBookDraftCheckpoint, PictureBookError> {
    PictureBookDraftCheckpoint::from_configuration(
        node.configuration()
            .ok_or_else(|| invalid("book draft configuration is missing"))?,
    )
    .map_err(invalid)
}
fn digest(graph: &ArtifactWorkingGraph) -> Result<String, PictureBookError> {
    Ok(ContentDigest::from_bytes(&serde_json::to_vec(graph)?).to_string())
}

impl ShapeProject {
    /// # Errors
    /// Rejects oversized JSON, malformed states, changed drafts and persistence failures.
    pub fn picture_book_draft_action_json(
        &mut self,
        request: &str,
    ) -> Result<String, PictureBookError> {
        if request.len() > 2 * 1024 * 1024 {
            return Err(invalid("book draft action exceeds 2 MiB"));
        }
        let result = self.picture_book_draft_action(serde_json::from_str(request)?)?;
        Ok(serde_json::to_string(&result)?)
    }
    /// # Errors
    /// Routes both adapters through exact Working Graph CAS and verified script checkpoint recovery.
    pub fn picture_book_draft_action(
        &mut self,
        action: PictureBookDraftAction,
    ) -> Result<Value, PictureBookError> {
        match action {
            PictureBookDraftAction::DraftNew { state } => {
                Ok(json!({"draft":self.create_picture_book_draft(&state)?}))
            }
            PictureBookDraftAction::DraftList => Ok(json!({"drafts":self.picture_book_drafts()?})),
            PictureBookDraftAction::DraftLoad { artifact_id } => {
                Ok(json!({"draft":self.open_picture_book_draft(artifact_id)?}))
            }
            PictureBookDraftAction::DraftSave {
                artifact_id,
                draft_digest,
                state,
            } => {
                Ok(json!({"draft":self.save_picture_book_draft(artifact_id,&draft_digest,&state)?}))
            }
            PictureBookDraftAction::DraftDiscard {
                artifact_id,
                draft_digest,
            } => {
                self.discard_picture_book_draft(artifact_id, &draft_digest)?;
                Ok(json!({"discarded":true}))
            }
        }
    }
    fn book_draft_projection(
        &self,
        graph: &ArtifactWorkingGraph,
    ) -> Result<PictureBookDraft, PictureBookError> {
        let checkpoint = checkpoint(book_node(graph)?)?;
        let bytes = self
            .store
            .read_content(&checkpoint.script_content)
            .map_err(CoreError::from)?;
        let script = String::from_utf8(bytes)
            .map_err(|_| invalid("book draft script checkpoint is not UTF-8"))?;
        let state = checkpoint.state(script);
        state.validate().map_err(invalid)?;
        Ok(PictureBookDraft {
            artifact_id: graph.context_artifact_id(),
            expected_head: graph.expected_revision_id(),
            source: graph
                .expected_revision_id()
                .map(|revision_id| WorkingInput {
                    artifact_id: graph.context_artifact_id(),
                    revision_id,
                }),
            draft_digest: digest(graph)?,
            state,
        })
    }
    fn book_draft_graph(
        &self,
        artifact_id: ArtifactId,
    ) -> Result<ArtifactWorkingGraph, PictureBookError> {
        let artifact = self
            .store
            .artifact(artifact_id)
            .map_err(CoreError::from)?
            .ok_or_else(|| invalid("book draft does not exist"))?;
        let graph = self
            .store
            .artifact_working_graph(artifact_id)
            .map_err(CoreError::from)?
            .ok_or_else(|| invalid("book draft is missing"))?;
        if artifact.kind != ArtifactKind::PictureBook
            || graph.expected_revision_id() != artifact.accepted_revision
        {
            return Err(invalid("book head changed; reopen the current version"));
        }
        book_node(&graph)?;
        Ok(graph)
    }
    fn book_checkpoint_configuration(
        &self,
        state: &PictureBookDraftState,
    ) -> Result<shape_domain::WorkingOperatorConfiguration, PictureBookError> {
        state.validate().map_err(invalid)?;
        let content = self
            .store
            .checkpoint_picture_book_script(state.script.as_bytes())
            .map_err(CoreError::from)?;
        state
            .checkpoint(content)
            .map_err(invalid)?
            .configuration()
            .map_err(invalid)
    }
    /// Reserves one book identity and checkpoints authoring state without accepting a script or book.
    /// # Errors
    /// Rejects invalid state and durable publication failures.
    pub fn create_picture_book_draft(
        &mut self,
        state: &PictureBookDraftState,
    ) -> Result<PictureBookDraft, PictureBookError> {
        let configuration = self.book_checkpoint_configuration(state)?;
        let name = if state.composition.name.trim().is_empty() || state.composition.name.len() > 120
        {
            "Picture book draft"
        } else {
            &state.composition.name
        };
        let artifact = Artifact::new(name, ArtifactKind::PictureBook).map_err(CoreError::from)?;
        let mut graph = ArtifactWorkingGraph::new_source(artifact.id);
        let node = graph
            .add_source_operator(
                OperatorTypeId::new("book.compose").map_err(CoreError::from)?,
                OperatorDataTypeId::new("book.picture").map_err(CoreError::from)?,
            )
            .map_err(CoreError::from)?;
        graph.set_operator_configuration(node.id(), Some(configuration));
        self.store
            .insert_source_artifact_with_working_graph(&artifact, &graph)
            .map_err(CoreError::from)?;
        self.book_draft_projection(&graph)
    }
    /// # Errors
    /// Rejects corrupt checkpoint bytes; current-head drafts are returned with full editor state.
    pub fn picture_book_drafts(&self) -> Result<Vec<PictureBookDraft>, PictureBookError> {
        self.store
            .artifact_working_graphs()
            .map_err(CoreError::from)?
            .iter()
            .filter(|g| {
                g.operators().iter().any(|n| {
                    n.operator_type().as_str() == "book.compose"
                        && n.configuration().is_some_and(|c| {
                            c.schema().as_str() == shape_domain::PICTURE_BOOK_DRAFT_SCHEMA
                        })
                })
            })
            .map(|g| self.book_draft_projection(g))
            .collect()
    }
    /// Opens an exact saved workspace or creates one from the current accepted book version.
    /// # Errors
    /// Rejects changed heads, unrelated drafts and corrupt accepted/checkpoint content.
    pub fn open_picture_book_draft(
        &self,
        artifact_id: ArtifactId,
    ) -> Result<PictureBookDraft, PictureBookError> {
        if self
            .store
            .artifact_working_graph(artifact_id)
            .map_err(CoreError::from)?
            .is_some()
        {
            return self.book_draft_projection(&self.book_draft_graph(artifact_id)?);
        }
        let artifact = self
            .store
            .artifact(artifact_id)
            .map_err(CoreError::from)?
            .ok_or_else(|| invalid("book does not exist"))?;
        let revision_id = artifact
            .accepted_revision
            .ok_or_else(|| invalid("unaccepted book has no recoverable workspace"))?;
        let composition = self.read_picture_book(WorkingInput {
            artifact_id,
            revision_id,
        })?;
        let source = composition
            .languages
            .get(&composition.default_language)
            .ok_or_else(|| invalid("accepted book default language is missing"))?;
        let script = String::from_utf8(self.read_revision_content(source.revision_id)?.bytes)
            .map_err(|_| invalid("accepted script is not UTF-8"))?;
        let state = PictureBookDraftState {
            active_language: composition.default_language.clone(),
            composition,
            page_index: 0,
            script,
            script_dirty: false,
            artwork_edit: None,
        };
        let mut graph = ArtifactWorkingGraph::new(artifact_id, revision_id);
        let node = graph
            .add_operator(
                OperatorTypeId::new("book.compose").map_err(CoreError::from)?,
                OperatorDataTypeId::new("book.picture").map_err(CoreError::from)?,
                OperatorDataTypeId::new("book.picture").map_err(CoreError::from)?,
            )
            .map_err(CoreError::from)?;
        graph.set_operator_configuration(
            node.id(),
            Some(self.book_checkpoint_configuration(&state)?),
        );
        self.store
            .replace_artifact_working_graph(artifact_id, None, Some(&graph))
            .map_err(CoreError::from)?;
        self.book_draft_projection(&graph)
    }
    /// # Errors
    /// Rejects stale saved graphs/heads. The complete authored state is checkpointed atomically.
    pub fn save_picture_book_draft(
        &self,
        artifact_id: ArtifactId,
        expected_digest: &str,
        state: &PictureBookDraftState,
    ) -> Result<PictureBookDraft, PictureBookError> {
        let graph = self.book_draft_graph(artifact_id)?;
        if digest(&graph)? != expected_digest {
            return Err(invalid("book draft changed; reload before editing"));
        }
        let configuration = self.book_checkpoint_configuration(state)?;
        let mut replacement = graph.clone();
        replacement.set_operator_configuration(book_node(&graph)?.id(), Some(configuration));
        self.store
            .replace_artifact_working_graph(artifact_id, Some(&graph), Some(&replacement))
            .map_err(CoreError::from)?;
        self.book_draft_projection(&replacement)
    }
    /// # Errors
    /// Removes only the exact saved draft and its unaccepted reserved output; accepted books stay.
    pub fn discard_picture_book_draft(
        &self,
        artifact_id: ArtifactId,
        expected_digest: &str,
    ) -> Result<(), PictureBookError> {
        let graph = self.book_draft_graph(artifact_id)?;
        if digest(&graph)? != expected_digest {
            return Err(invalid("book draft changed; reload before discarding"));
        }
        self.store
            .replace_artifact_working_graph(artifact_id, Some(&graph), None)
            .map_err(CoreError::from)?;
        Ok(())
    }
    /// # Errors
    /// Unadopted script edits cannot enter a proposal. Exact accepted material pins are revalidated.
    pub fn propose_picture_book_draft(
        &self,
        artifact_id: ArtifactId,
        expected_digest: &str,
    ) -> Result<PictureBookCandidate, PictureBookError> {
        let graph = self.book_draft_graph(artifact_id)?;
        if digest(&graph)? != expected_digest {
            return Err(invalid("book draft changed; reload before reviewing"));
        }
        let draft = self.book_draft_projection(&graph)?;
        if draft.state.script_dirty {
            return Err(invalid(
                "adopt or discard the script draft before reviewing the book",
            ));
        }
        Artifact::new(&draft.state.composition.name, ArtifactKind::PictureBook)
            .map_err(CoreError::from)?;
        let artifact = self
            .store
            .artifact(artifact_id)
            .map_err(CoreError::from)?
            .ok_or_else(|| invalid("book does not exist"))?;
        self.prepare_picture_book(
            artifact,
            draft.expected_head,
            true,
            Some(book_node(&graph)?.clone()),
            draft.state.composition,
        )
    }
    pub(super) fn current_book_candidate_node(
        &self,
        artifact_id: ArtifactId,
        expected: &WorkingOperatorDraft,
        composition: &shape_domain::PictureBookRequest,
    ) -> Result<WorkingOperatorDraft, PictureBookError> {
        let graph = self.book_draft_graph(artifact_id)?;
        let node = book_node(&graph)?;
        let actual = checkpoint(node)?;
        if node.id() != expected.id() || actual.composition != *composition || actual.script_dirty {
            return Err(invalid("book authoring changed after review"));
        }
        // Viewing another page/language may checkpoint UI state without changing the reviewed book.
        Ok(node.clone())
    }
}

#[cfg(test)]
mod tests;
