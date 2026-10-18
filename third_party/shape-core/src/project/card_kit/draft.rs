//! Mutable card composition uses the existing Working Graph checkpoint and exact-graph CAS.
use serde::Serialize;
use shape_domain::{
    Artifact, ArtifactId, ArtifactKind, ArtifactWorkingGraph, CARD_KIT_DATA_TYPE,
    CARD_KIT_OPERATOR_TYPE, CardKitRequest, ContentDigest, OperatorDataTypeId, OperatorTypeId,
    RevisionId, WorkingInput, WorkingOperatorDraft,
};

use super::{CardKitError, ShapeProject, invalid};
use crate::CoreError;

#[derive(Debug, Clone, Serialize)]
pub struct CardKitDraft {
    pub artifact_id: ArtifactId,
    pub expected_head: Option<RevisionId>,
    pub draft_digest: String,
    pub composition: CardKitRequest,
}

pub(super) fn node(graph: &ArtifactWorkingGraph) -> Result<&WorkingOperatorDraft, CardKitError> {
    if graph.operators().len() != 1 {
        return Err(invalid("card kit needs one composition draft"));
    }
    let node = &graph.operators()[0];
    if node.operator_type().as_str() != CARD_KIT_OPERATOR_TYPE
        || node.output_data_type().as_str() != CARD_KIT_DATA_TYPE
    {
        return Err(invalid("artifact does not contain a card kit draft"));
    }
    Ok(node)
}

fn projection(graph: &ArtifactWorkingGraph) -> Result<CardKitDraft, CardKitError> {
    let configuration = node(graph)?
        .configuration()
        .ok_or_else(|| invalid("card kit draft has no composition"))?;
    let composition = CardKitRequest::from_configuration(configuration).map_err(invalid)?;
    Ok(CardKitDraft {
        artifact_id: graph.context_artifact_id(),
        expected_head: graph.expected_revision_id(),
        draft_digest: ContentDigest::from_bytes(&serde_json::to_vec(graph)?).to_string(),
        composition,
    })
}

impl ShapeProject {
    /// Reserves an unaccepted card kit and its complete authored state atomically.
    /// # Errors
    /// Rejects invalid direction, cards, dimensions and storage failures.
    pub fn create_card_kit_draft(
        &mut self,
        composition: &CardKitRequest,
    ) -> Result<CardKitDraft, CardKitError> {
        composition.validate().map_err(invalid)?;
        let artifact =
            Artifact::new(&composition.name, ArtifactKind::CardKit).map_err(CoreError::from)?;
        let mut graph = ArtifactWorkingGraph::new_source(artifact.id);
        let draft = graph
            .add_source_operator(
                OperatorTypeId::new(CARD_KIT_OPERATOR_TYPE).map_err(CoreError::from)?,
                OperatorDataTypeId::new(CARD_KIT_DATA_TYPE).map_err(CoreError::from)?,
            )
            .map_err(CoreError::from)?;
        graph.set_operator_configuration(
            draft.id(),
            Some(composition.configuration().map_err(invalid)?),
        );
        self.store
            .insert_source_artifact_with_working_graph(&artifact, &graph)
            .map_err(CoreError::from)?;
        projection(&graph)
    }

    /// Lists complete current-head drafts in stable artifact identity order, including accepted kits.
    /// # Errors
    /// Rejects corrupt persisted draft metadata rather than inventing replacement values.
    pub fn card_kit_drafts(&self) -> Result<Vec<CardKitDraft>, CardKitError> {
        self.store
            .artifact_working_graphs()
            .map_err(CoreError::from)?
            .iter()
            .filter(|g| {
                g.operators()
                    .iter()
                    .any(|n| n.operator_type().as_str() == CARD_KIT_OPERATOR_TYPE)
            })
            .map(projection)
            .collect()
    }

    /// Reopens an existing exact draft or explicitly starts editing the current accepted kit.
    /// # Errors
    /// Rejects changed heads and unrelated existing drafts; no other writer's draft is replaced.
    pub fn open_card_kit_draft(
        &mut self,
        artifact_id: ArtifactId,
    ) -> Result<CardKitDraft, CardKitError> {
        let artifact = self
            .store
            .artifact(artifact_id)
            .map_err(CoreError::from)?
            .ok_or_else(|| invalid("card kit does not exist"))?;
        if artifact.kind != ArtifactKind::CardKit {
            return Err(invalid("artifact is not a card kit"));
        }
        if let Some(graph) = self
            .store
            .artifact_working_graph(artifact_id)
            .map_err(CoreError::from)?
        {
            if graph.expected_revision_id() != artifact.accepted_revision {
                return Err(invalid("card kit head changed; reopen the current version"));
            }
            return projection(&graph);
        }
        let revision_id = artifact
            .accepted_revision
            .ok_or_else(|| invalid("unaccepted card kit has no recoverable draft"))?;
        let composition = self.read_card_kit(WorkingInput {
            artifact_id,
            revision_id,
        })?;
        let mut graph = ArtifactWorkingGraph::new(artifact_id, revision_id);
        let draft = graph
            .add_operator(
                OperatorTypeId::new(CARD_KIT_OPERATOR_TYPE).map_err(CoreError::from)?,
                OperatorDataTypeId::new(CARD_KIT_DATA_TYPE).map_err(CoreError::from)?,
                OperatorDataTypeId::new(CARD_KIT_DATA_TYPE).map_err(CoreError::from)?,
            )
            .map_err(CoreError::from)?;
        graph.set_operator_configuration(
            draft.id(),
            Some(composition.configuration().map_err(invalid)?),
        );
        self.store
            .replace_artifact_working_graph(artifact_id, None, Some(&graph))
            .map_err(CoreError::from)?;
        projection(&graph)
    }

    pub(super) fn checked_card_kit_draft(
        &self,
        artifact_id: ArtifactId,
        digest: &str,
    ) -> Result<CardKitDraft, CardKitError> {
        let artifact = self
            .store
            .artifact(artifact_id)
            .map_err(CoreError::from)?
            .ok_or_else(|| invalid("card kit does not exist"))?;
        let graph = self
            .store
            .artifact_working_graph(artifact_id)
            .map_err(CoreError::from)?
            .ok_or_else(|| invalid("card kit draft is missing"))?;
        if artifact.kind != ArtifactKind::CardKit
            || graph.expected_revision_id() != artifact.accepted_revision
        {
            return Err(invalid("card kit head changed; reopen the current version"));
        }
        let draft = projection(&graph)?;
        if draft.draft_digest != digest {
            return Err(invalid("card kit draft changed; reload before editing"));
        }
        Ok(draft)
    }

    /// Saves only against the exact last-read graph, preserving stable card and producer ids.
    /// # Errors
    /// Rejects stale edits and invalid content. Repeated unchanged saves retain the same digest.
    pub fn save_card_kit_draft(
        &self,
        artifact_id: ArtifactId,
        digest: &str,
        composition: &CardKitRequest,
    ) -> Result<CardKitDraft, CardKitError> {
        self.checked_card_kit_draft(artifact_id, digest)?;
        composition.validate().map_err(invalid)?;
        let graph = self
            .store
            .artifact_working_graph(artifact_id)
            .map_err(CoreError::from)?
            .ok_or_else(|| invalid("card kit draft is missing"))?;
        // This second comparison binds the graph read for replacement to the caller's digest.
        if projection(&graph)?.draft_digest != digest {
            return Err(invalid("card kit draft changed; reload before editing"));
        }
        let mut replacement = graph.clone();
        replacement.set_operator_configuration(
            node(&graph)?.id(),
            Some(composition.configuration().map_err(invalid)?),
        );
        self.store
            .replace_artifact_working_graph(artifact_id, Some(&graph), Some(&replacement))
            .map_err(CoreError::from)?;
        projection(&replacement)
    }

    /// Removes only the exact saved draft; an accepted kit remains readable.
    /// # Errors
    /// Rejects stale drafts or heads and leaves them untouched.
    pub fn discard_card_kit_draft(
        &self,
        artifact_id: ArtifactId,
        digest: &str,
    ) -> Result<(), CardKitError> {
        self.checked_card_kit_draft(artifact_id, digest)?;
        let graph = self
            .store
            .artifact_working_graph(artifact_id)
            .map_err(CoreError::from)?
            .ok_or_else(|| invalid("card kit draft is missing"))?;
        if projection(&graph)?.draft_digest != digest {
            return Err(invalid("card kit draft changed; reload before discarding"));
        }
        self.store
            .replace_artifact_working_graph(artifact_id, Some(&graph), None)
            .map_err(CoreError::from)?;
        Ok(())
    }
}
