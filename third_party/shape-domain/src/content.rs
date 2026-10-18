//! Immutable content identity and portable media contracts.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::DomainError;

const MAX_MEDIA_TYPE_BYTES: usize = 127;

/// BLAKE3-256 identity over exact content bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentDigest([u8; 32]);

impl ContentDigest {
    /// Hashes exact payload bytes.
    #[must_use]
    pub fn from_bytes(payload: &[u8]) -> Self {
        Self(*blake3::hash(payload).as_bytes())
    }

    /// Constructs an identity from a verified digest.
    #[must_use]
    pub const fn from_digest_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for ContentDigest {
    type Err = ContentDigestParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64 || !value.is_ascii() {
            return Err(ContentDigestParseError);
        }
        let mut bytes = [0u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            let pair = std::str::from_utf8(pair).map_err(|_| ContentDigestParseError)?;
            bytes[index] = u8::from_str_radix(pair, 16).map_err(|_| ContentDigestParseError)?;
        }
        Ok(Self(bytes))
    }
}

/// A malformed hexadecimal content digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentDigestParseError;

impl fmt::Display for ContentDigestParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("content digest must be 64 hexadecimal characters")
    }
}

impl std::error::Error for ContentDigestParseError {}

/// Durable reference to one immutable content object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentRef {
    /// Content-addressed object identity.
    pub digest: ContentDigest,
    /// IANA-style media type describing the payload.
    pub media_type: String,
    /// Exact payload length.
    pub byte_length: u64,
}

impl ContentRef {
    /// Creates a validated content reference.
    ///
    /// # Errors
    ///
    /// Returns an error when the media type is not a bounded ASCII token.
    pub fn new(
        digest: ContentDigest,
        media_type: impl Into<String>,
        byte_length: u64,
    ) -> Result<Self, DomainError> {
        let media_type = media_type.into();
        if media_type.is_empty()
            || media_type.len() > MAX_MEDIA_TYPE_BYTES
            || !media_type.is_ascii()
            || !media_type.contains('/')
        {
            return Err(DomainError::InvalidMediaType {
                max_bytes: MAX_MEDIA_TYPE_BYTES,
            });
        }
        Ok(Self {
            digest,
            media_type,
            byte_length,
        })
    }
}

#[cfg(test)]
mod tests;
