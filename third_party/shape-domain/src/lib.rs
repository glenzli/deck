//! Platform-independent Creative Document contracts.
//!
//! This crate is the stable vocabulary shared by persistence, execution,
//! application services, and future desktop/media bridges:
//!
//! - [`ids`] owns strongly typed persistent identities;
//! - [`content`] owns immutable content references and media contracts;
//! - [`artifact`] owns creative identity and immutable accepted revisions;
//! - [`card_kit`] owns card material lists with independent text/artwork bindings;
//! - [`workflow`] routes creation templates to their concrete content consumers;
//! - [`audio`] owns portable audio values, family ports, and voice authorization;
//! - [`ai_image`] owns provider-neutral generative image families and authored parameters;
//! - [`image_blur`] owns deterministic alpha-aware blur parameters;
//! - [`image_crop`] owns the first stable image Operator contract;
//! - [`image_drop_shadow`] owns deterministic flattened shadow bounds and tint;
//! - [`image_resize`] owns deterministic resize dimensions and policies;
//! - [`image_transform`] owns lossless raster orientation transforms;
//! - [`image_unsharp_mask`] owns deterministic alpha-aware sharpening;
//! - [`operator_graph`] owns typed Source/Operator/Output scene structure;
//! - [`scene`] owns stable Scene identity, immutable accepted graph revisions,
//!   and named output publication;
//! - [`transformation`] owns creative meaning, constraints, and references;
//! - [`working_graph`] owns durable mutable Operator entries outside accepted history;
//! - [`project`] owns project identity and schema revision.
//!
//! Filesystem, `SQLite`, Qt, C++, provider, and network types do not belong here.

mod ai_image;
mod artifact;
mod card_kit;
pub use card_kit::{
    CARD_KIT_DATA_TYPE, CARD_KIT_DRAFT_SCHEMA, CARD_KIT_MAX_BYTES, CARD_KIT_MEDIA_TYPE,
    CARD_KIT_OPERATOR_TYPE, CARD_KIT_SCHEMA, CardImageFit, CardImagePosition, CardKitCard,
    CardKitDocument, CardKitRequest, CardLayoutConfig, CardLayoutMode, CardTextAlign,
    CardTextVerticalAlign,
};
mod workflow;
pub use workflow::{WorkflowTemplate, workflow_templates};
mod material_edit;
pub use material_edit::{
    MATERIAL_EDIT_SCHEMA, MaterialEditConfiguration, MaterialEditOperation, MaterialEditor,
    MaterialReturnNormalization,
};
mod audio;
mod sound_generation;
pub use sound_generation::{
    SOUND_GENERATION_REVISION, SoundGenerationKind, SoundGenerationOperation,
};
mod content;
mod error;
mod ids;
mod image_blur;
mod image_crop;
mod image_drop_shadow;
mod image_raster;
mod image_resize;
mod image_transform;
mod image_unsharp_mask;
mod operator_graph;
mod picture_book;
mod picture_book_draft;
pub use picture_book_draft::{
    PICTURE_BOOK_DRAFT_MAX_SCRIPT_BYTES, PICTURE_BOOK_DRAFT_SCHEMA,
    PICTURE_BOOK_DRAFT_SCRIPT_MEDIA_TYPE, PictureBookArtworkEditDraft, PictureBookDraftCheckpoint,
    PictureBookDraftState,
};
mod project;
pub use picture_book::{
    PICTURE_BOOK_MEDIA_TYPE, PICTURE_BOOK_SCHEMA, PictureBookDocument, PictureBookPage,
    PictureBookRequest,
};
mod scene;
pub mod speech_script;
mod text_document;
mod transformation;
mod working_graph;
pub use working_graph::WorkingInput;

pub use ai_image::{
    AI_IMAGE_GENERATE_FROM_MATERIALS_OPERATOR_TYPE, AI_IMAGE_GENERATE_OPERATOR_TYPE,
    AI_IMAGE_MAX_CANDIDATES, AI_IMAGE_MAX_DIMENSION, AI_IMAGE_MAX_PIXELS,
    AI_IMAGE_PARAMETERS_REVISION, AI_IMAGE_RASTER_DATA_TYPE, AiImageContractError,
    AiImageGenerateFromMaterialsParameters, AiImageGenerateParameters, AiImageMaterialReference,
    AiImageMaterialRole, AiImageOperation, AiImageOperatorContract, AiImageOperatorFamily,
    AiImageOutputCanvas, AiImagePortCardinality, AiImagePortContract, AiImagePortDataType,
};
pub use artifact::{Artifact, ArtifactContentContract, ArtifactKind, ArtifactRevision};
pub use audio::{
    AUDIO_CLIP_DATA_TYPE, AUDIO_VALUE_CONTRACT_REVISION, AUTHORIZED_VOICE_REFERENCE_DATA_TYPE,
    AudioContainer, AudioOperatorContract, AudioOperatorFamily, AudioOriginDisclosure,
    AudioPortCardinality, AudioPortContract, AudioPortDataType, AudioSampleFormat,
    AudioValueContract, AuthorizedVoiceReference, PresetVoiceAlias, PresetVoiceSelection,
    SpeechSynthesisOperation, SpeechVoiceSelection, VoiceAuthorizationScope,
};
pub use content::{ContentDigest, ContentDigestParseError, ContentRef};
pub use error::DomainError;
pub use ids::{ArtifactId, ProjectId, RevisionId, SceneId, SceneRevisionId, TransformationId};
pub use image_blur::{
    IMAGE_BLUR_DATA_TYPE, IMAGE_BLUR_MAX_PIXELS, IMAGE_BLUR_MAX_RADIUS, IMAGE_BLUR_OPERATOR_TYPE,
    IMAGE_BLUR_PARAMETERS_REVISION, ImageBlurContractError, ImageBlurOperator, PreparedImageBlur,
    RasterGaussianBlur,
};
pub use image_crop::{
    IMAGE_CROP_DATA_TYPE, IMAGE_CROP_OPERATOR_TYPE, IMAGE_CROP_PARAMETERS_REVISION,
    ImageCropContractError, ImageCropOperator, PreparedImageCrop, RasterCrop,
};
pub use image_drop_shadow::{
    IMAGE_DROP_SHADOW_DATA_TYPE, IMAGE_DROP_SHADOW_MAX_BLUR_RADIUS,
    IMAGE_DROP_SHADOW_MAX_DIMENSION, IMAGE_DROP_SHADOW_MAX_OFFSET, IMAGE_DROP_SHADOW_MAX_PIXELS,
    IMAGE_DROP_SHADOW_OPERATOR_TYPE, IMAGE_DROP_SHADOW_PARAMETERS_REVISION,
    ImageDropShadowContractError, ImageDropShadowOperator, PreparedImageDropShadow,
    RasterDropShadow, RasterShadowColor,
};
pub use image_raster::{
    IMAGE_RASTER_CONTRACT_REVISION, ImageAlphaMode, ImageColorPrimaries, ImageColorProfile,
    ImageLightReference, ImageOrientation, ImagePixelFormat, ImageRasterContract,
    ImageTransferFunction,
};
pub use image_resize::{
    IMAGE_RESIZE_DATA_TYPE, IMAGE_RESIZE_MAX_DIMENSION, IMAGE_RESIZE_MAX_PIXELS,
    IMAGE_RESIZE_OPERATOR_TYPE, IMAGE_RESIZE_PARAMETERS_REVISION, ImageResizeContractError,
    ImageResizeOperator, PreparedImageResize, RasterResize, RasterResizeAspectPolicy,
    RasterResizeDimensions, RasterResizeResampling,
};
pub use image_transform::{
    IMAGE_TRANSFORM_DATA_TYPE, IMAGE_TRANSFORM_OPERATOR_TYPE, IMAGE_TRANSFORM_PARAMETERS_REVISION,
    ImageTransformContractError, ImageTransformOperator, PreparedImageTransform, RasterTransform,
};
pub use image_unsharp_mask::{
    IMAGE_UNSHARP_MASK_DATA_TYPE, IMAGE_UNSHARP_MASK_MAX_AMOUNT_MILLI,
    IMAGE_UNSHARP_MASK_MAX_PIXELS, IMAGE_UNSHARP_MASK_MAX_RADIUS, IMAGE_UNSHARP_MASK_OPERATOR_TYPE,
    IMAGE_UNSHARP_MASK_PARAMETERS_REVISION, ImageUnsharpMaskContractError,
    ImageUnsharpMaskOperator, PreparedImageUnsharpMask, RasterUnsharpMask,
};
pub use operator_graph::{
    OperatorDataTypeId, OperatorGraph, OperatorGraphEdge, OperatorGraphNode, OperatorNodeBinding,
    OperatorNodeId, OperatorNodeRole, OperatorPort, OperatorPortId, OperatorTypeId,
};
pub use project::{ProjectMetadata, SHAPE_PROJECT_SCHEMA_REVISION};
pub use scene::{NamedSceneOutput, Scene, SceneOutputName, SceneRevision};
pub use text_document::{SPEECH_SCRIPT_FORMAT_REVISION, TextDocumentContract};
pub use transformation::{
    Constraint, ConstraintKind, ConstraintStrength, IntentSpec, ReferenceBinding, ReferenceRole,
    Transformation, TransformationKind, TransformationOperation,
};
pub use working_graph::{
    ArtifactWorkingGraph, OperatorConfigurationSchemaId, WorkingOperatorConfiguration,
    WorkingOperatorDraft,
};
