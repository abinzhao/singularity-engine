use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ArtifactKind, ArtifactKindParseError, ArtifactName, ArtifactNameError};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TargetRef {
    kind: ArtifactKind,
    name: ArtifactName,
}

impl TargetRef {
    pub fn new(kind: ArtifactKind, name: ArtifactName) -> Self {
        Self { kind, name }
    }

    pub fn kind(&self) -> ArtifactKind {
        self.kind
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl fmt::Display for TargetRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.name)
    }
}

impl FromStr for TargetRef {
    type Err = TargetRefParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (kind, name) = input
            .split_once(':')
            .ok_or(TargetRefParseError::MissingSeparator)?;

        Ok(Self {
            kind: kind.parse()?,
            name: name.parse()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TargetRefParseError {
    #[error("target ref must use `<kind>:<name>`")]
    MissingSeparator,
    #[error(transparent)]
    Kind(#[from] ArtifactKindParseError),
    #[error(transparent)]
    Name(#[from] ArtifactNameError),
}
