use pdf_engine::EditCommand;

/// Edit stack with undo/redo. Every mutation of queued edits goes through
/// here so the redo branch is dropped exactly when a new edit is pushed.
#[derive(Default)]
pub struct EditHistory {
    edits: Vec<EditCommand>,
    undone: Vec<EditCommand>,
}

impl EditHistory {
    pub fn edits(&self) -> &[EditCommand] {
        &self.edits
    }

    pub fn len(&self) -> usize {
        self.edits.len()
    }

    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    pub fn can_undo(&self) -> bool {
        !self.edits.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    pub fn push(&mut self, edit: EditCommand) {
        self.undone.clear();
        self.edits.push(edit);
    }

    pub fn undo(&mut self) -> Option<EditCommand> {
        let edit = self.edits.pop()?;
        self.undone.push(edit.clone());
        Some(edit)
    }

    pub fn redo(&mut self) -> Option<EditCommand> {
        let edit = self.undone.pop()?;
        self.edits.push(edit.clone());
        Some(edit)
    }

    pub fn remove(&mut self, index: usize) -> Option<EditCommand> {
        (index < self.edits.len()).then(|| {
            self.undone.clear();
            self.edits.remove(index)
        })
    }

    pub fn retain(&mut self, keep: impl FnMut(&EditCommand) -> bool) {
        self.edits.retain(keep);
    }

    pub fn clear(&mut self) {
        self.edits.clear();
        self.undone.clear();
    }

    pub fn iter(&self) -> std::slice::Iter<'_, EditCommand> {
        self.edits.iter()
    }

    pub fn to_vec(&self) -> Vec<EditCommand> {
        self.edits.clone()
    }
}

#[cfg(test)]
mod tests {
    use document_core::PdfRect;
    use pdf_engine::EditCommand;

    use super::EditHistory;

    fn redaction(page_index: usize) -> EditCommand {
        EditCommand::Redact {
            page_index,
            rect: PdfRect::new(0.0, 0.0, 10.0, 10.0).unwrap(),
        }
    }

    #[test]
    fn undo_and_redo_walk_the_same_stack() {
        let mut history = EditHistory::default();
        history.push(redaction(0));
        history.push(redaction(1));

        assert!(history.undo().is_some());
        assert_eq!(history.len(), 1);
        assert!(history.can_redo());

        assert!(history.redo().is_some());
        assert_eq!(history.len(), 2);
        assert!(!history.can_redo());
    }

    #[test]
    fn new_edit_discards_the_redo_branch() {
        let mut history = EditHistory::default();
        history.push(redaction(0));
        history.undo();
        history.push(redaction(1));

        assert!(!history.can_redo());
        assert_eq!(history.len(), 1);
    }
}
