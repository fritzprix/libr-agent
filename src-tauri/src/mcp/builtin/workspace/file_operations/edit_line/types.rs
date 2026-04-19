#[derive(Debug, Clone)]
pub(super) struct LineEdit {
    pub(super) start_line: usize,
    pub(super) end_line: usize,
    pub(super) new_value: String,
    pub(super) start_anchor: Option<String>,
    pub(super) end_anchor: Option<String>,
    pub(super) action: EditAction,
}

impl LineEdit {
    pub(super) fn requires_existing_line_anchor(&self) -> bool {
        !(self.action == EditAction::InsertAfter && self.start_line == 0)
    }

    pub(super) fn requires_end_hash(&self) -> bool {
        matches!(self.action, EditAction::Replace | EditAction::Delete)
            && self.end_line > self.start_line
    }

    pub(super) fn replacement_line_count(&self) -> usize {
        if self.new_value.is_empty() {
            0
        } else {
            self.new_value.lines().count()
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ParsedEdit {
    pub(super) path: String,
    pub(super) edit: LineEdit,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedFileEdit {
    pub(super) path: String,
    pub(super) edits: Vec<LineEdit>,
    pub(super) original_content: String,
    pub(super) new_content: String,
    pub(super) original_line_count: usize,
    pub(super) new_hash_sections: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditAction {
    Replace,
    InsertAfter,
    Delete,
}
