use std::path::Path;

use crate::{AppError, Result, init};

pub fn validate_workspace(path: impl AsRef<Path>) -> Result<()> {
    let root = path.as_ref();
    require_file(root.join("singularity.yaml").as_path())?;

    for relative in init::required_workspace_dirs() {
        require_dir(root.join(relative).as_path())?;
    }

    require_file(root.join(".singularity/repo.git/HEAD").as_path())?;
    Ok(())
}

fn require_file(path: &Path) -> Result<()> {
    if path.is_file() {
        return Ok(());
    }

    Err(AppError::InvalidWorkspace {
        path: path.to_path_buf(),
        message: "expected file to exist".to_owned(),
    })
}

fn require_dir(path: &Path) -> Result<()> {
    if path.is_dir() {
        return Ok(());
    }

    Err(AppError::InvalidWorkspace {
        path: path.to_path_buf(),
        message: "expected directory to exist".to_owned(),
    })
}
