use std::fs;

use serde_json::json;
use sge_store::{
    AppendOnlyJournal, JournalEntry, JournalState, RecoveryAction, RecoveryClassifier,
};
use tempfile::TempDir;

#[test]
fn journal_appends_newline_delimited_json_with_monotonic_sequences() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("journal.ndjson");
    let journal = AppendOnlyJournal::open(&path).unwrap();

    let first = journal
        .append(JournalState::Prepared, json!({ "revision": "base" }))
        .unwrap();
    let second = journal
        .append(JournalState::Mutating, json!({ "candidate": "next" }))
        .unwrap();

    assert_eq!(first.sequence, 1);
    assert_eq!(second.sequence, 2);

    let contents = fs::read_to_string(&path).unwrap();
    let lines = contents.lines().collect::<Vec<_>>();

    assert_eq!(lines.len(), 2);

    let first_line: JournalEntry = serde_json::from_str(lines[0]).unwrap();
    let second_line: JournalEntry = serde_json::from_str(lines[1]).unwrap();

    assert_eq!(first_line.sequence, 1);
    assert_eq!(first_line.state, JournalState::Prepared);
    assert_eq!(second_line.sequence, 2);
    assert_eq!(second_line.state, JournalState::Mutating);
    assert!(contents.ends_with('\n'));
}

#[test]
fn journal_reopens_at_next_sequence() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("journal.ndjson");

    AppendOnlyJournal::open(&path)
        .unwrap()
        .append(JournalState::Prepared, json!({}))
        .unwrap();

    let entry = AppendOnlyJournal::open(&path)
        .unwrap()
        .append(JournalState::Evaluating, json!({}))
        .unwrap();

    assert_eq!(entry.sequence, 2);
}

#[test]
fn recovery_classifies_terminal_and_non_terminal_states() {
    for state in [JournalState::Completed, JournalState::Aborted] {
        let action = RecoveryClassifier::classify(state);

        assert_eq!(action, RecoveryAction::Terminal(state));
    }

    for state in [
        JournalState::Prepared,
        JournalState::Baseline,
        JournalState::Diagnosed,
        JournalState::Approved,
        JournalState::Mutating,
        JournalState::Evaluating,
        JournalState::ReviewPending,
        JournalState::Applying,
    ] {
        let action = RecoveryClassifier::classify(state);

        assert_ne!(action, RecoveryAction::Terminal(JournalState::Completed));
        assert!(matches!(
            action,
            RecoveryAction::Resumable(_) | RecoveryAction::Abortable(_)
        ));
    }
}
