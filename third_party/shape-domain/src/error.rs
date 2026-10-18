//! Creative contract validation failures.

use thiserror::Error;

/// A violation of a platform-independent Shape domain invariant.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    #[error(
        "sound generation requires a bounded prompt, supported kind, and duration from 1 to 30 seconds"
    )]
    InvalidSoundGenerationOperation,
    /// A project name is empty or exceeds the portable limit.
    #[error("project name must contain 1..={max_bytes} UTF-8 bytes")]
    InvalidProjectName { max_bytes: usize },
    /// A scene name is empty or exceeds the portable limit.
    #[error("scene name must contain 1..={max_bytes} UTF-8 bytes")]
    InvalidSceneName { max_bytes: usize },
    /// A named Scene output is not a bounded portable token.
    #[error("scene output name must be a portable ASCII token within {max_bytes} bytes")]
    InvalidSceneOutputName { max_bytes: usize },
    /// An accepted Scene revision has no named output or an inconsistent output mapping.
    #[error("accepted scene revisions must name every output node exactly once")]
    InvalidSceneOutputs,
    /// An artifact name is empty or exceeds the portable limit.
    #[error("artifact name must contain 1..={max_bytes} UTF-8 bytes")]
    InvalidArtifactName { max_bytes: usize },
    /// A media type is empty or is too large to persist portably.
    #[error("media type must contain 1..={max_bytes} ASCII bytes")]
    InvalidMediaType { max_bytes: usize },
    /// Intent text is empty or exceeds the bounded creative contract.
    #[error("intent summary must contain 1..={max_bytes} UTF-8 bytes")]
    InvalidIntent { max_bytes: usize },
    /// A constraint description is empty or too large.
    #[error("constraint value must contain 1..={max_bytes} UTF-8 bytes")]
    InvalidConstraint { max_bytes: usize },
    /// A component target is empty or too large.
    #[error("component target must contain 1..={max_bytes} UTF-8 bytes")]
    InvalidComponentTarget { max_bytes: usize },
    /// A transformation has too many inputs, constraints, or references.
    #[error("{collection} contains {actual} items; maximum is {maximum}")]
    CollectionTooLarge {
        collection: &'static str,
        actual: usize,
        maximum: usize,
    },
    /// A transformation lists the same input revision more than once.
    #[error("transformation input revisions must be unique")]
    DuplicateTransformationInput,
    /// A reference label is empty or too large.
    #[error("reference label must contain 1..={max_bytes} UTF-8 bytes")]
    InvalidReferenceLabel { max_bytes: usize },
    /// A raster contract contained zero or non-portable dimensions.
    #[error("raster dimensions must be within 1..={maximum}; received {width}x{height}")]
    InvalidRasterDimensions {
        width: u32,
        height: u32,
        maximum: u32,
    },
    /// A crop rectangle was empty or exceeded its source raster.
    #[error(
        "crop rectangle ({x}, {y}, {width}, {height}) exceeds source {source_width}x{source_height}"
    )]
    InvalidRasterCrop {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        source_width: u32,
        source_height: u32,
    },
    /// Resize dimensions exceeded a portable axis or total-pixel bound.
    #[error(
        "resize dimensions must be within 1..={maximum_dimension} and at most {maximum_pixels} pixels; received {width}x{height}"
    )]
    InvalidRasterResizeDimensions {
        width: u32,
        height: u32,
        maximum_dimension: u32,
        maximum_pixels: u64,
    },
    /// A blur radius was an identity or exceeded the portable support bound.
    #[error("blur radius must be within 1..={maximum}; received {radius}")]
    InvalidRasterBlurRadius { radius: u16, maximum: u16 },
    /// Unsharp-mask radius or amount was an identity or exceeded portable bounds.
    #[error(
        "unsharp mask requires radius within 1..={maximum_radius} and amount within 1..={maximum_amount_milli}; received radius={radius}, amount={amount_milli}"
    )]
    InvalidRasterUnsharpMask {
        radius: u16,
        amount_milli: u16,
        maximum_radius: u16,
        maximum_amount_milli: u16,
    },
    /// Drop-shadow parameters were invisible or exceeded portable bounds.
    #[error(
        "drop shadow requires visible color, offsets within +-{maximum_offset}, and blur radius within 0..={maximum_radius}"
    )]
    InvalidRasterDropShadow {
        maximum_offset: u32,
        maximum_radius: u16,
    },
    /// A typed operation was attached to an incompatible transformation family.
    #[error("typed operation is incompatible with its transformation family")]
    InvalidTransformationOperation,
    /// An Operator Graph identifier is not a bounded namespaced ASCII token.
    #[error("{field} id must be a namespaced ASCII value within {max_bytes} bytes")]
    InvalidOperatorIdentifier {
        field: &'static str,
        max_bytes: usize,
    },
    /// A node role, durable binding, or port direction is inconsistent.
    #[error("operator node role, binding, and port directions are inconsistent")]
    InvalidOperatorNode,
    /// Two nodes share one identity.
    #[error("operator graph node identities must be unique")]
    DuplicateOperatorNode,
    /// Two ports in one direction share one identity.
    #[error("{collection} must have unique identities")]
    DuplicateOperatorPort { collection: &'static str },
    /// An edge endpoint or port is missing, duplicated, or self-referential.
    #[error("operator graph edge is invalid")]
    InvalidOperatorEdge,
    /// Connected ports declare different data contracts.
    #[error("operator graph ports must have exactly matching data types")]
    OperatorPortTypeMismatch,
    /// One input port has more than one upstream binding.
    #[error("operator input port is already bound")]
    OperatorInputAlreadyBound,
    /// The first graph contract permits only acyclic creative dependencies.
    #[error("operator graph must be acyclic")]
    OperatorGraphCycle,
    /// Mutable Operator configuration is not a bounded versioned JSON object.
    #[error(
        "working Operator configuration must be a versioned JSON object within {max_bytes} bytes"
    )]
    InvalidWorkingOperatorConfiguration { max_bytes: usize },
    /// A mutable graph mixes an accepted input anchor with a zero-input Source Operator.
    #[error("working graph anchor and Operator input presence are inconsistent")]
    InvalidWorkingGraphAnchor,
    /// An accepted audio value has an impossible or empty sample contract.
    #[error("audio value contract must declare bounded non-empty PCM audio")]
    InvalidAudioValueContract,
    /// A runtime-owned preset voice alias is malformed.
    #[error("preset voice alias must be lowercase ASCII within {max_bytes} bytes")]
    InvalidPresetVoiceAlias { max_bytes: usize },
    /// A preset voice catalog revision is missing or not portable.
    #[error("preset voice catalog revision must contain 1..={max_bytes} ASCII bytes")]
    InvalidVoiceCatalogRevision { max_bytes: usize },
    /// A real-voice reference lacks explicit local-only consent or disclosure.
    #[error("voice authorization must be bounded, local-only, and require synthetic disclosure")]
    InvalidVoiceAuthorization { max_bytes: usize },
    /// Speech synthesis parameters are outside the portable creative contract.
    #[error(
        "speech synthesis requires a bounded language tag, 0.25x-4.0x speed, and disclosure (language limit {max_language_bytes} bytes)"
    )]
    InvalidSpeechSynthesisOperation { max_language_bytes: usize },
}
