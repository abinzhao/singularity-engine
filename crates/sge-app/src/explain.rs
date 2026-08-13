use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Explanation {
    pub run_id: String,
    pub markdown: String,
}

pub fn explain_run(workspace: impl AsRef<Path>, run_id: &str) -> Result<Explanation> {
    validate_run_id(run_id)?;
    let path = workspace
        .as_ref()
        .join(".singularity/runs")
        .join(run_id)
        .join("decision.md");
    let markdown = fs::read_to_string(&path).map_err(|source| AppError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(Explanation {
        run_id: run_id.to_string(),
        markdown,
    })
}

pub(crate) fn validate_run_id(run_id: &str) -> Result<()> {
    if run_id.is_empty()
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(AppError::Evolution {
            path: Path::new(run_id).to_path_buf(),
            message: "run id contains unsupported characters".to_string(),
        });
    }
    Ok(())
}
