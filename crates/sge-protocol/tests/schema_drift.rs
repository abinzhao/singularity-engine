use std::fs;
use std::path::PathBuf;

use sge_protocol::schemas::v1_schema_files;

#[test]
fn committed_v1_json_schemas_match_generated_output() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    for schema_file in v1_schema_files().unwrap() {
        let path = workspace_root
            .join("schemas/v1")
            .join(schema_file.file_name());
        let committed = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));

        assert_eq!(
            committed,
            schema_file.contents(),
            "{} is out of date; run `cargo run -p xtask -- schemas`",
            path.display()
        );
    }
}
