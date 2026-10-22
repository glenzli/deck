//! Product use cases over the domain, execution, and persistence owners.
//!
//! This crate demonstrates the critical foundation invariant: execution and
//! Scene graph editing create transient candidates, while explicit acceptance
//! alone advances a durable Artifact or Scene head.

mod error;
mod project;

pub use error::CoreError;
pub use project::{
    AcceptedArtifactContent, AgentTextCandidate, AiImageCandidate, AudioCandidate, ImageCandidate,
    ImageEditCandidate, ImageResizeCandidate, MATERIAL_EDIT_TICKET_SCHEMA, MaterialEditCandidate,
    MaterialEditHandoff, MaterialEditTicket, SceneGraphCandidate, ShapeProject, SoundCandidate,
    TEXT_DOCUMENT_DATA_TYPE, TEXT_EDIT_OPERATOR_TYPE, TEXT_TRANSFORM_OPERATOR_TYPE, TextCandidate,
    TextEditParameters, TextTransformMode, TextTransformParameters,
};
pub use project::{
    CARD_BACK_RESOURCE_ID, CardKitAction, CardKitCandidate, CardKitDraft, CardKitError,
    CardKitExport, CardPrintCutLineStyle, CardPrintFlipEdge, CardPrintOptions, CardPrintPaper,
    CardPrintPlan, CardPrintRequestOptions, CardPrintSide, CardPrintSides, CardPrintSidesOptions,
};
pub use project::{
    PictureBookCandidate, PictureBookDraft, PictureBookDraftAction, PictureBookError,
    PictureBookExport, PictureBookPage, PictureBookRequest,
};
