use sge_protocol::{Document, ProtocolError, parse_document};

const FIXTURES: [(&str, &str); 5] = [
    (
        "artifact",
        include_str!("../../../fixtures/protocol/v1/artifact.yaml"),
    ),
    (
        "contract",
        include_str!("../../../fixtures/protocol/v1/contract.yaml"),
    ),
    (
        "evidence",
        include_str!("../../../fixtures/protocol/v1/evidence.yaml"),
    ),
    (
        "memory",
        include_str!("../../../fixtures/protocol/v1/memory.yaml"),
    ),
    (
        "adapter",
        include_str!("../../../fixtures/protocol/v1/adapter.yaml"),
    ),
];

#[test]
fn round_trips_v1_fixtures_without_losing_extension_fields() {
    for (kind, raw) in FIXTURES {
        let document = parse_document(raw).unwrap();

        assert_matches_kind(kind, &document);

        let rendered = serde_yaml::to_string(&document).unwrap();
        assert!(
            rendered.contains("x-extra"),
            "{kind} fixture lost extension fields"
        );

        let reparsed = parse_document(&rendered).unwrap();
        assert_eq!(reparsed, document, "{kind} fixture should round-trip");
    }
}

#[test]
fn rejects_unsupported_schema_without_fallback() {
    let err = parse_document(
        r#"
schema: sge.dev/artifact/v2
id: skill:future-artifact
kind: skill
name: future-artifact
"#,
    )
    .unwrap_err();

    assert_eq!(
        err,
        ProtocolError::UnsupportedSchema {
            schema: "sge.dev/artifact/v2".to_owned(),
        }
    );
}

fn assert_matches_kind(kind: &str, document: &Document) {
    match (kind, document) {
        ("artifact", Document::Artifact(_)) => {}
        ("contract", Document::Contract(_)) => {}
        ("evidence", Document::Evidence(_)) => {}
        ("memory", Document::Memory(_)) => {}
        ("adapter", Document::Adapter(_)) => {}
        _ => panic!("{kind} parsed into the wrong document variant: {document:?}"),
    }
}
