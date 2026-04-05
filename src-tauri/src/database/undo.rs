use crate::error::{AppError, StorageErrorCode};
use std::collections::VecDeque;
use std::sync::Mutex;

const MAX_UNDO_STACK: usize = 50;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum UndoOperation {
    UpdateText {
        segment_id: String,
        old_text: String,
        new_text: String,
    },
    DeleteSegment {
        segment_id: String,
        transcript_id: String,
        index_num: i64,
        start_ms: i64,
        end_ms: i64,
        text: String,
        speaker_id: Option<String>,
        confidence: Option<f64>,
    },
    MergeSegments {
        kept_id: String,
        removed_id: String,
        old_kept_text: String,
        old_removed_text: String,
        merged_text: String,
        old_kept_end_ms: i64,
        new_kept_end_ms: i64,
    },
    SplitSegment {
        original_id: String,
        new_id: String,
        old_text: String,
        first_text: String,
        second_text: String,
        split_ms: i64,
    },
}

pub struct UndoManager {
    undo_stack: Mutex<VecDeque<UndoOperation>>,
    redo_stack: Mutex<VecDeque<UndoOperation>>,
}

impl UndoManager {
    pub fn new() -> Self {
        Self {
            undo_stack: Mutex::new(VecDeque::with_capacity(MAX_UNDO_STACK)),
            redo_stack: Mutex::new(VecDeque::with_capacity(MAX_UNDO_STACK)),
        }
    }

    pub fn push(&self, op: UndoOperation) -> Result<(), AppError> {
        let mut stack = self.undo_stack.lock().map_err(|_| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: "Failed to lock undo stack".into(),
        })?;
        if stack.len() >= MAX_UNDO_STACK {
            stack.pop_front();
        }
        stack.push_back(op);
        // Clear redo stack on new action
        if let Ok(mut redo) = self.redo_stack.lock() {
            redo.clear();
        }
        Ok(())
    }

    pub fn pop_undo(&self) -> Result<Option<UndoOperation>, AppError> {
        let mut stack = self.undo_stack.lock().map_err(|_| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: "Failed to lock undo stack".into(),
        })?;
        Ok(stack.pop_back())
    }

    pub fn pop_redo(&self) -> Result<Option<UndoOperation>, AppError> {
        let mut stack = self.redo_stack.lock().map_err(|_| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: "Failed to lock redo stack".into(),
        })?;
        Ok(stack.pop_back())
    }

    pub fn push_redo(&self, op: UndoOperation) -> Result<(), AppError> {
        let mut stack = self.redo_stack.lock().map_err(|_| AppError::StorageError {
            code: StorageErrorCode::DatabaseError,
            message: "Failed to lock redo stack".into(),
        })?;
        if stack.len() >= MAX_UNDO_STACK {
            stack.pop_front();
        }
        stack.push_back(op);
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        self.undo_stack
            .lock()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }

    pub fn can_redo(&self) -> bool {
        self.redo_stack
            .lock()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_pop_undo() {
        let mgr = UndoManager::new();
        mgr.push(UndoOperation::UpdateText {
            segment_id: "s1".into(),
            old_text: "old".into(),
            new_text: "new".into(),
        })
        .unwrap();
        assert!(mgr.can_undo());
        let op = mgr.pop_undo().unwrap();
        assert!(op.is_some());
        assert!(!mgr.can_undo());
    }

    #[test]
    fn test_redo_cleared_on_new_push() {
        let mgr = UndoManager::new();
        mgr.push(UndoOperation::UpdateText {
            segment_id: "s1".into(),
            old_text: "a".into(),
            new_text: "b".into(),
        })
        .unwrap();
        let op = mgr.pop_undo().unwrap().unwrap();
        mgr.push_redo(op).unwrap();
        assert!(mgr.can_redo());

        mgr.push(UndoOperation::UpdateText {
            segment_id: "s2".into(),
            old_text: "c".into(),
            new_text: "d".into(),
        })
        .unwrap();
        assert!(!mgr.can_redo());
    }

    #[test]
    fn test_max_stack_size() {
        let mgr = UndoManager::new();
        for i in 0..60 {
            mgr.push(UndoOperation::UpdateText {
                segment_id: format!("s{}", i),
                old_text: "old".into(),
                new_text: "new".into(),
            })
            .unwrap();
        }
        let stack = mgr.undo_stack.lock().unwrap();
        assert!(stack.len() <= MAX_UNDO_STACK);
    }
}
