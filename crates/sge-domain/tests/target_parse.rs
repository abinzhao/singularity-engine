use sge_domain::{ArtifactKind, TargetRef};

#[test]
fn parses_skill_target_ref() {
    let target = "skill:code-review".parse::<TargetRef>().unwrap();

    assert_eq!(target.kind(), ArtifactKind::Skill);
    assert_eq!(target.name(), "code-review");
}

#[test]
fn rejects_invalid_target_names() {
    for input in [
        "skill:",
        "skill:Code",
        "skill:code_review",
        "skill:code/review",
        "skill:code..review",
        "skill:code--review",
        "skill:-code",
        "skill:code-",
        "skill:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        assert!(input.parse::<TargetRef>().is_err(), "{input} should fail");
    }
}

#[test]
fn rejects_unknown_target_kind() {
    assert!("tool:code-review".parse::<TargetRef>().is_err());
}
