use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sge_app::import::import_artifact;
use sge_app::init;

fn temp_workspace(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("sge-app-import-{name}-{nonce}"))
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("evolution")
        .join("basic-skill")
}

fn init_workspace(root: &Path) {
    init::initialize(root).expect("workspace initialization should succeed");
}

#[test]
fn import_copies_declared_files_and_snapshots_revision() {
    let root = temp_workspace("basic");
    init_workspace(&root);
    let fixture = fixture_path();

    let imported = import_artifact(&root, &fixture).expect("import should succeed");

    let skill_dir = root.join("skills").join("code-review");
    assert!(
        skill_dir.join("skill.yaml").is_file(),
        "skill.yaml should be copied into skills/code-review"
    );

    let expected_instructions = fs::read_to_string(fixture.join("instructions.md"))
        .expect("fixture instructions should be readable");
    let actual_instructions = fs::read_to_string(skill_dir.join("instructions.md"))
        .expect("copied instructions should be readable");
    assert_eq!(actual_instructions, expected_instructions);

    assert_eq!(imported.target, "skill:code-review");
    assert!(
        !imported.revision.is_empty(),
        "revision should be a non-empty git oid"
    );
    assert!(
        imported.warnings.len() <= 2,
        "expected few or no warnings, got: {:?}",
        imported.warnings
    );

    fs::remove_dir_all(root).expect("failed to clean up temp workspace");
}

#[test]
fn import_rejects_symlink_escaping_source_root() {
    let root = temp_workspace("symlink");
    init_workspace(&root);

    let source_dir = std::env::temp_dir().join(format!(
        "sge-symlink-source-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock issue")
            .as_nanos()
    ));
    fs::create_dir_all(&source_dir).expect("create symlink source dir");

    let skill_yaml = r#"
schema: sge.dev/artifact/v1
id: evil-skill-v1
kind: skill
name: evil-skill
title: Evil Skill
summary: Tries to escape source root.
body: see escape.md
files:
  - path: escape.md
    required: true
"#;
    fs::write(source_dir.join("skill.yaml"), skill_yaml).expect("write skill.yaml");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let _ = symlink("/etc/hosts", source_dir.join("escape.md"));
    }
    #[cfg(not(unix))]
    {
        fs::write(source_dir.join("escape.md"), "placeholder").unwrap();
    }

    let err = import_artifact(&root, &source_dir).expect_err("symlink escape must be rejected");
    assert!(
        err.code().starts_with("SGE-IMPORT-"),
        "expected SGE-IMPORT- error code, got {}",
        err.code()
    );

    let _ = fs::remove_dir_all(&source_dir);
    fs::remove_dir_all(root).expect("failed to clean up temp workspace");
}

#[test]
fn import_rejects_duplicate_name() {
    let root = temp_workspace("duplicate");
    init_workspace(&root);
    let fixture = fixture_path();

    import_artifact(&root, &fixture).expect("first import should succeed");

    let err = import_artifact(&root, &fixture).expect_err("second import must fail");
    assert_eq!(err.code(), "SGE-IMPORT-002");

    fs::remove_dir_all(root).expect("failed to clean up temp workspace");
}
