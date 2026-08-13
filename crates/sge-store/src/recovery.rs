use crate::JournalState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    Terminal(JournalState),
    Resumable(JournalState),
    Abortable(JournalState),
}

pub struct RecoveryClassifier;

impl RecoveryClassifier {
    pub fn classify(state: JournalState) -> RecoveryAction {
        match state {
            JournalState::Completed | JournalState::Aborted => RecoveryAction::Terminal(state),
            JournalState::Prepared | JournalState::ReviewPending => {
                RecoveryAction::Resumable(state)
            }
            JournalState::Mutating | JournalState::Evaluating | JournalState::Applying => {
                RecoveryAction::Abortable(state)
            }
        }
    }
}
