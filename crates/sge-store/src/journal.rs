use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::StoreError;

#[derive(Debug, Clone)]
pub struct AppendOnlyJournal {
    path: PathBuf,
}

impl AppendOnlyJournal {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        Ok(Self { path })
    }

    pub fn append(
        &self,
        state: JournalState,
        payload: serde_json::Value,
    ) -> Result<JournalEntry, StoreError> {
        let entry = JournalEntry {
            sequence: self.next_sequence()?,
            state,
            payload,
        };
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, &entry)?;
        file.write_all(b"\n")?;
        file.sync_all()?;

        Ok(entry)
    }

    fn next_sequence(&self) -> Result<u64, StoreError> {
        if !self.path.exists() {
            return Ok(1);
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut last_sequence = 0;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: JournalEntry = serde_json::from_str(&line)?;
            if entry.sequence <= last_sequence {
                return Err(StoreError::JournalInvariant {
                    message: "journal sequence numbers must increase monotonically".to_owned(),
                });
            }
            last_sequence = entry.sequence;
        }

        Ok(last_sequence + 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JournalState {
    Prepared,
    Mutating,
    Evaluating,
    ReviewPending,
    Applying,
    Completed,
    Aborted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub sequence: u64,
    pub state: JournalState,
    pub payload: serde_json::Value,
}
