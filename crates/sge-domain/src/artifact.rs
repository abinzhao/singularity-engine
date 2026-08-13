use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    Agent,
    Skill,
    Rule,
}

impl ArtifactKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Skill => "skill",
            Self::Rule => "rule",
        }
    }
}

impl fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ArtifactKind {
    type Err = ArtifactKindParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "agent" => Ok(Self::Agent),
            "skill" => Ok(Self::Skill),
            "rule" => Ok(Self::Rule),
            _ => Err(ArtifactKindParseError {
                value: input.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactName(String);

impl ArtifactName {
    pub fn new(input: impl Into<String>) -> Result<Self, ArtifactNameError> {
        let input = input.into();
        validate_artifact_name(&input)?;
        Ok(Self(input))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ArtifactName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<ArtifactName> for String {
    fn from(name: ArtifactName) -> Self {
        name.0
    }
}

impl TryFrom<String> for ArtifactName {
    type Error = ArtifactNameError;

    fn try_from(input: String) -> Result<Self, Self::Error> {
        Self::new(input)
    }
}

impl TryFrom<&str> for ArtifactName {
    type Error = ArtifactNameError;

    fn try_from(input: &str) -> Result<Self, Self::Error> {
        Self::new(input)
    }
}

impl FromStr for ArtifactName {
    type Err = ArtifactNameError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

fn validate_artifact_name(input: &str) -> Result<(), ArtifactNameError> {
    if input.is_empty() {
        return Err(ArtifactNameError::Empty);
    }

    if input.len() > 64 {
        return Err(ArtifactNameError::TooLong { bytes: input.len() });
    }

    if input.starts_with('-') || input.ends_with('-') {
        return Err(ArtifactNameError::EdgeHyphen);
    }

    let mut previous_hyphen = false;
    for byte in input.bytes() {
        let valid = byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-';
        if !valid {
            return Err(ArtifactNameError::InvalidCharacter);
        }

        if byte == b'-' {
            if previous_hyphen {
                return Err(ArtifactNameError::ConsecutiveHyphen);
            }
            previous_hyphen = true;
        } else {
            previous_hyphen = false;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown artifact kind `{value}`")]
pub struct ArtifactKindParseError {
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ArtifactNameError {
    #[error("artifact name must not be empty")]
    Empty,
    #[error("artifact name must be at most 64 bytes, got {bytes}")]
    TooLong { bytes: usize },
    #[error("artifact name must not start or end with hyphen")]
    EdgeHyphen,
    #[error("artifact name must not contain consecutive hyphens")]
    ConsecutiveHyphen,
    #[error("artifact name must contain only lowercase ASCII letters, digits, or hyphens")]
    InvalidCharacter,
}
