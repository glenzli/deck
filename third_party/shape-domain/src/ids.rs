//! Strongly typed persistent identities.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! persistent_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Creates a fresh time-ordered identity.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Wraps an identity loaded from durable storage.
            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Exposes the underlying UUID.
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::from_str(value).map(Self)
            }
        }
    };
}

persistent_id!(ProjectId, "Identifies one portable Shape project.");
persistent_id!(SceneId, "Identifies one stable creative scene.");
persistent_id!(
    SceneRevisionId,
    "Identifies one immutable accepted scene graph state."
);
persistent_id!(ArtifactId, "Identifies one stable creative artifact.");
persistent_id!(
    RevisionId,
    "Identifies one immutable accepted artifact state."
);
persistent_id!(
    TransformationId,
    "Identifies the creative meaning that produced a revision."
);

#[cfg(test)]
mod tests;
