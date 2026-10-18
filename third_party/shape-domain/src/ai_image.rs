//! Provider-neutral contracts for comprehensive AI Image Operators.
//!
//! Shape exposes two distinct creative identities: prompt-led generation with
//! no material inputs, and material-conditioned generation whose accepted
//! image revisions are explicit authored references. Both produce one logical
//! `image.raster` output. Physical models, providers, samplers, endpoints, and
//! transient Candidate payloads do not belong in this contract.

use std::collections::HashSet;

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use crate::{Constraint, ConstraintKind, ConstraintStrength, DomainError, RevisionId};

/// Prompt-led generation without a primary material input.
pub const AI_IMAGE_GENERATE_OPERATOR_TYPE: &str = "image.generate";
/// Generation conditioned by one or more explicit accepted image materials.
pub const AI_IMAGE_GENERATE_FROM_MATERIALS_OPERATOR_TYPE: &str = "image.generate_from_materials";
/// First exact authored-parameter schema shared by both logical identities.
pub const AI_IMAGE_PARAMETERS_REVISION: &str = "20260811.1";
/// Scene-graph value produced and consumed by AI Image Operators.
pub const AI_IMAGE_RASTER_DATA_TYPE: &str = "image.raster";
/// Largest portable output dimension accepted by the authored contract.
pub const AI_IMAGE_MAX_DIMENSION: u32 = 32_768;
/// Largest portable output raster accepted by the authored contract.
pub const AI_IMAGE_MAX_PIXELS: u64 = 64 * 1024 * 1024;
/// Largest number of transient Candidates requested by one authored operation.
pub const AI_IMAGE_MAX_CANDIDATES: u8 = 8;

const MAX_INSTRUCTION_BYTES: usize = 16 * 1024;
const MAX_MATERIAL_LABEL_BYTES: usize = 160;
const MAX_MATERIALS: usize = 16;
const MAX_CONSTRAINTS: usize = 32;

/// A violation of the provider-neutral AI Image contract.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AiImageContractError {
    #[error("AI Image parameters must use revision {expected}")]
    UnsupportedParametersRevision { expected: &'static str },
    #[error("AI Image instruction must contain 1..={max_bytes} UTF-8 bytes")]
    InvalidInstruction { max_bytes: usize },
    #[error(
        "AI Image output must be within 1..={max_dimension} per axis and at most {max_pixels} pixels; received {width}x{height}"
    )]
    InvalidOutputDimensions {
        width: u32,
        height: u32,
        max_dimension: u32,
        max_pixels: u64,
    },
    #[error("AI Image candidate count must be within 1..={maximum}; received {actual}")]
    InvalidCandidateCount { actual: u8, maximum: u8 },
    #[error("{collection} contains {actual} items; maximum is {maximum}")]
    CollectionTooLarge {
        collection: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("image.generate cannot declare material-preservation constraints")]
    SourceLessPreservationConstraint,
    #[error("AI Image constraints cannot preserve timing")]
    InvalidImageConstraint,
    #[error("image.generate_from_materials requires at least one explicit image material")]
    MaterialRequired,
    #[error("AI Image material label must contain 1..={max_bytes} UTF-8 bytes")]
    InvalidMaterialLabel { max_bytes: usize },
    #[error("AI Image materials must bind each accepted revision only once")]
    DuplicateMaterialRevision,
    #[error(transparent)]
    Domain(#[from] DomainError),
}

/// The two user-visible comprehensive AI Image Operator families.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiImageOperatorFamily {
    /// Create an image from authored intent without a primary material.
    Generate,
    /// Create an image conditioned by explicit accepted image materials.
    GenerateFromMaterials,
}

/// Portable data type allowed at an AI Image family port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiImagePortDataType {
    ImageRaster,
}

impl AiImagePortDataType {
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::ImageRaster => AI_IMAGE_RASTER_DATA_TYPE,
        }
    }
}

/// Whether an instantiated AI Image Operator must expose one port binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiImagePortCardinality {
    Required,
    OneOrMore,
}

/// One stable port in an AI Image family contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiImagePortContract {
    pub id: &'static str,
    pub data_type: AiImagePortDataType,
    pub cardinality: AiImagePortCardinality,
}

/// Version-independent creative ports for one AI Image family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiImageOperatorContract {
    pub operator_type: &'static str,
    pub inputs: &'static [AiImagePortContract],
    pub outputs: &'static [AiImagePortContract],
}

const GENERATE_INPUTS: [AiImagePortContract; 0] = [];
const GENERATE_FROM_MATERIALS_INPUTS: [AiImagePortContract; 1] = [AiImagePortContract {
    id: "input.materials",
    data_type: AiImagePortDataType::ImageRaster,
    cardinality: AiImagePortCardinality::OneOrMore,
}];
const IMAGE_OUTPUTS: [AiImagePortContract; 1] = [AiImagePortContract {
    id: "output.image",
    data_type: AiImagePortDataType::ImageRaster,
    cardinality: AiImagePortCardinality::Required,
}];

impl AiImageOperatorFamily {
    /// Returns the creative port contract without claiming runtime executability.
    #[must_use]
    pub const fn contract(self) -> AiImageOperatorContract {
        match self {
            Self::Generate => AiImageOperatorContract {
                operator_type: AI_IMAGE_GENERATE_OPERATOR_TYPE,
                inputs: &GENERATE_INPUTS,
                outputs: &IMAGE_OUTPUTS,
            },
            Self::GenerateFromMaterials => AiImageOperatorContract {
                operator_type: AI_IMAGE_GENERATE_FROM_MATERIALS_OPERATOR_TYPE,
                inputs: &GENERATE_FROM_MATERIALS_INPUTS,
                outputs: &IMAGE_OUTPUTS,
            },
        }
    }
}

/// Authored output canvas shared by the two generation families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiImageOutputCanvas {
    width: u32,
    height: u32,
}

impl<'de> Deserialize<'de> for AiImageOutputCanvas {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireCanvas {
            width: u32,
            height: u32,
        }

        let wire = WireCanvas::deserialize(deserializer)?;
        Self::new(wire.width, wire.height).map_err(D::Error::custom)
    }
}

impl AiImageOutputCanvas {
    /// Creates a bounded flattened-raster output request.
    ///
    /// # Errors
    ///
    /// Rejects zero dimensions, dimensions over 32768, or more than 64 Mi pixels.
    pub fn new(width: u32, height: u32) -> Result<Self, AiImageContractError> {
        let pixel_count = u64::from(width) * u64::from(height);
        if width == 0
            || height == 0
            || width > AI_IMAGE_MAX_DIMENSION
            || height > AI_IMAGE_MAX_DIMENSION
            || pixel_count > AI_IMAGE_MAX_PIXELS
        {
            return Err(AiImageContractError::InvalidOutputDimensions {
                width,
                height,
                max_dimension: AI_IMAGE_MAX_DIMENSION,
                max_pixels: AI_IMAGE_MAX_PIXELS,
            });
        }
        Ok(Self { width, height })
    }

    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }

    #[must_use]
    pub const fn pixel_count(self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

/// Creative meaning assigned to one material-conditioned image input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiImageMaterialRole {
    /// Source content that the operation is expected to transform.
    Source,
    Identity,
    Style,
    Composition,
    Palette,
    Lighting,
    /// A visual counterexample describing what the result should avoid.
    Negative,
}

/// One accepted image revision explicitly bound as generation material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiImageMaterialReference {
    revision_id: RevisionId,
    role: AiImageMaterialRole,
    label: String,
}

impl<'de> Deserialize<'de> for AiImageMaterialReference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireReference {
            revision_id: RevisionId,
            role: AiImageMaterialRole,
            label: String,
        }

        let wire = WireReference::deserialize(deserializer)?;
        Self::new(wire.revision_id, wire.role, wire.label).map_err(D::Error::custom)
    }
}

impl AiImageMaterialReference {
    /// Creates a labeled, typed reference to one accepted image revision.
    ///
    /// # Errors
    ///
    /// Rejects empty or oversized labels.
    pub fn new(
        revision_id: RevisionId,
        role: AiImageMaterialRole,
        label: impl Into<String>,
    ) -> Result<Self, AiImageContractError> {
        let label = label.into();
        if label.trim().is_empty() || label.len() > MAX_MATERIAL_LABEL_BYTES {
            return Err(AiImageContractError::InvalidMaterialLabel {
                max_bytes: MAX_MATERIAL_LABEL_BYTES,
            });
        }
        Ok(Self {
            revision_id,
            role,
            label,
        })
    }

    #[must_use]
    pub const fn revision_id(&self) -> RevisionId {
        self.revision_id
    }

    #[must_use]
    pub const fn role(&self) -> AiImageMaterialRole {
        self.role
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Exact authored request for prompt-led, source-less generation.
/// The requested output canvas guides generation. The returned raster contract
/// records native dimensions; deterministic resizing is a separate operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiImageGenerateParameters {
    schema_revision: String,
    instruction: String,
    output: AiImageOutputCanvas,
    candidate_count: u8,
    constraints: Vec<Constraint>,
}

impl<'de> Deserialize<'de> for AiImageGenerateParameters {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireParameters {
            schema_revision: String,
            instruction: String,
            output: AiImageOutputCanvas,
            candidate_count: u8,
            constraints: Vec<WireConstraint>,
        }

        let wire = WireParameters::deserialize(deserializer)?;
        let constraints = decode_constraints(wire.constraints).map_err(D::Error::custom)?;
        Self::from_parts(
            wire.schema_revision,
            wire.instruction,
            wire.output,
            wire.candidate_count,
            constraints,
        )
        .map_err(D::Error::custom)
    }
}

impl AiImageGenerateParameters {
    /// Creates source-less generation parameters.
    ///
    /// # Errors
    ///
    /// Rejects invalid instructions, output bounds, candidate counts, and
    /// preservation constraints that have no material anchor.
    pub fn new(
        instruction: impl Into<String>,
        output: AiImageOutputCanvas,
        candidate_count: u8,
        constraints: Vec<Constraint>,
    ) -> Result<Self, AiImageContractError> {
        Self::from_parts(
            AI_IMAGE_PARAMETERS_REVISION.to_owned(),
            instruction.into(),
            output,
            candidate_count,
            constraints,
        )
    }

    fn from_parts(
        schema_revision: String,
        instruction: String,
        output: AiImageOutputCanvas,
        candidate_count: u8,
        constraints: Vec<Constraint>,
    ) -> Result<Self, AiImageContractError> {
        validate_common(
            &schema_revision,
            &instruction,
            candidate_count,
            &constraints,
        )?;
        if constraints.iter().any(|constraint| {
            matches!(
                constraint.kind,
                ConstraintKind::PreserveIdentity
                    | ConstraintKind::PreserveGeometry
                    | ConstraintKind::PreserveContent
                    | ConstraintKind::RegionScope
            )
        }) {
            return Err(AiImageContractError::SourceLessPreservationConstraint);
        }
        Ok(Self {
            schema_revision,
            instruction,
            output,
            candidate_count,
            constraints,
        })
    }

    #[must_use]
    pub fn schema_revision(&self) -> &str {
        &self.schema_revision
    }

    #[must_use]
    pub fn instruction(&self) -> &str {
        &self.instruction
    }

    #[must_use]
    pub const fn output(&self) -> AiImageOutputCanvas {
        self.output
    }

    #[must_use]
    pub const fn candidate_count(&self) -> u8 {
        self.candidate_count
    }

    #[must_use]
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }
}

/// Exact provider-neutral parameters for generation from explicit materials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiImageGenerateFromMaterialsParameters {
    schema_revision: String,
    instruction: String,
    materials: Vec<AiImageMaterialReference>,
    output: AiImageOutputCanvas,
    candidate_count: u8,
    constraints: Vec<Constraint>,
}

impl<'de> Deserialize<'de> for AiImageGenerateFromMaterialsParameters {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireParameters {
            schema_revision: String,
            instruction: String,
            materials: Vec<AiImageMaterialReference>,
            output: AiImageOutputCanvas,
            candidate_count: u8,
            constraints: Vec<WireConstraint>,
        }

        let wire = WireParameters::deserialize(deserializer)?;
        let constraints = decode_constraints(wire.constraints).map_err(D::Error::custom)?;
        Self::from_parts(
            wire.schema_revision,
            wire.instruction,
            wire.materials,
            wire.output,
            wire.candidate_count,
            constraints,
        )
        .map_err(D::Error::custom)
    }
}

impl AiImageGenerateFromMaterialsParameters {
    /// Creates generation parameters bound to accepted image materials.
    ///
    /// # Errors
    ///
    /// Rejects missing or duplicate materials and all invalid common fields.
    pub fn new(
        instruction: impl Into<String>,
        materials: Vec<AiImageMaterialReference>,
        output: AiImageOutputCanvas,
        candidate_count: u8,
        constraints: Vec<Constraint>,
    ) -> Result<Self, AiImageContractError> {
        Self::from_parts(
            AI_IMAGE_PARAMETERS_REVISION.to_owned(),
            instruction.into(),
            materials,
            output,
            candidate_count,
            constraints,
        )
    }

    fn from_parts(
        schema_revision: String,
        instruction: String,
        materials: Vec<AiImageMaterialReference>,
        output: AiImageOutputCanvas,
        candidate_count: u8,
        constraints: Vec<Constraint>,
    ) -> Result<Self, AiImageContractError> {
        validate_common(
            &schema_revision,
            &instruction,
            candidate_count,
            &constraints,
        )?;
        if materials.is_empty() {
            return Err(AiImageContractError::MaterialRequired);
        }
        if materials.len() > MAX_MATERIALS {
            return Err(AiImageContractError::CollectionTooLarge {
                collection: "AI Image materials",
                actual: materials.len(),
                maximum: MAX_MATERIALS,
            });
        }
        let unique_revisions = materials
            .iter()
            .map(AiImageMaterialReference::revision_id)
            .collect::<HashSet<_>>();
        if unique_revisions.len() != materials.len() {
            return Err(AiImageContractError::DuplicateMaterialRevision);
        }
        Ok(Self {
            schema_revision,
            instruction,
            materials,
            output,
            candidate_count,
            constraints,
        })
    }

    #[must_use]
    pub fn schema_revision(&self) -> &str {
        &self.schema_revision
    }

    #[must_use]
    pub fn instruction(&self) -> &str {
        &self.instruction
    }

    #[must_use]
    pub fn materials(&self) -> &[AiImageMaterialReference] {
        &self.materials
    }

    #[must_use]
    pub const fn output(&self) -> AiImageOutputCanvas {
        self.output
    }

    #[must_use]
    pub const fn candidate_count(&self) -> u8 {
        self.candidate_count
    }

    #[must_use]
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }
}

/// One exact authored AI Image operation, independent of its future executor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operator_type", content = "parameters", deny_unknown_fields)]
pub enum AiImageOperation {
    #[serde(rename = "image.generate")]
    Generate(AiImageGenerateParameters),
    #[serde(rename = "image.generate_from_materials")]
    GenerateFromMaterials(AiImageGenerateFromMaterialsParameters),
}

impl AiImageOperation {
    #[must_use]
    pub const fn family(&self) -> AiImageOperatorFamily {
        match self {
            Self::Generate(_) => AiImageOperatorFamily::Generate,
            Self::GenerateFromMaterials(_) => AiImageOperatorFamily::GenerateFromMaterials,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireConstraint {
    kind: ConstraintKind,
    strength: ConstraintStrength,
    value: String,
    target_component: Option<String>,
}

fn decode_constraints(
    constraints: Vec<WireConstraint>,
) -> Result<Vec<Constraint>, AiImageContractError> {
    constraints
        .into_iter()
        .map(|constraint| {
            Constraint::new(
                constraint.kind,
                constraint.strength,
                constraint.value,
                constraint.target_component,
            )
            .map_err(AiImageContractError::from)
        })
        .collect()
}

fn validate_common(
    schema_revision: &str,
    instruction: &str,
    candidate_count: u8,
    constraints: &[Constraint],
) -> Result<(), AiImageContractError> {
    if schema_revision != AI_IMAGE_PARAMETERS_REVISION {
        return Err(AiImageContractError::UnsupportedParametersRevision {
            expected: AI_IMAGE_PARAMETERS_REVISION,
        });
    }
    if instruction.trim().is_empty() || instruction.len() > MAX_INSTRUCTION_BYTES {
        return Err(AiImageContractError::InvalidInstruction {
            max_bytes: MAX_INSTRUCTION_BYTES,
        });
    }
    if !(1..=AI_IMAGE_MAX_CANDIDATES).contains(&candidate_count) {
        return Err(AiImageContractError::InvalidCandidateCount {
            actual: candidate_count,
            maximum: AI_IMAGE_MAX_CANDIDATES,
        });
    }
    if constraints.len() > MAX_CONSTRAINTS {
        return Err(AiImageContractError::CollectionTooLarge {
            collection: "AI Image constraints",
            actual: constraints.len(),
            maximum: MAX_CONSTRAINTS,
        });
    }
    for constraint in constraints {
        if constraint.kind == ConstraintKind::PreserveTiming {
            return Err(AiImageContractError::InvalidImageConstraint);
        }
        Constraint::new(
            constraint.kind,
            constraint.strength,
            constraint.value.clone(),
            constraint.target_component.clone(),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
