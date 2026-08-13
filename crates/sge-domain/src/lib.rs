pub mod artifact;
pub mod target;

pub use artifact::{ArtifactKind, ArtifactKindParseError, ArtifactName, ArtifactNameError};
pub use target::{TargetRef, TargetRefParseError};

pub const PRODUCT_NAME: &str = "SINGULARITY ENGINE";
