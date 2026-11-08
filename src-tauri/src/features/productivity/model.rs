use std::collections::HashSet;

use serde::{Deserialize, Serialize};

pub const MAX_NOTES: usize = 1_000;
pub const MAX_BOOKMARKS: usize = 500;
pub const MAX_CATEGORIES: usize = 50;
pub const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
pub const CATEGORY_COLORS: [&str; 6] = ["violet", "blue", "mint", "amber", "rose", "gray"];

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Todo {
    pub id: String,
    pub text: String,
    pub done: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub todos: Vec<Todo>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NoteInput {
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub todos: Vec<Todo>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BookmarkKind {
    File,
    Folder,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bookmark {
    pub id: String,
    pub label: String,
    pub path: String,
    pub kind: BookmarkKind,
    pub category_id: Option<String>,
    pub created_at: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub notes: Vec<Note>,
    pub bookmarks: Vec<Bookmark>,
    pub categories: Vec<Category>,
}

pub fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 100
        || !id
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || value == b'-' || value == b'_')
    {
        return Err("Mã mục không hợp lệ.".into());
    }
    Ok(())
}

fn validate_text(value: &str, limit: usize, label: &str) -> Result<(), String> {
    if value.chars().count() > limit || value.contains('\0') {
        return Err(format!(
            "{label} không được quá {limit} ký tự hoặc chứa ký tự rỗng."
        ));
    }
    Ok(())
}

fn validate_todos(todos: &[Todo]) -> Result<(), String> {
    if todos.len() > 1_000 {
        return Err("Một ghi chú chỉ được có tối đa 1.000 việc cần làm.".into());
    }
    let mut ids = HashSet::new();
    for todo in todos {
        validate_id(&todo.id)?;
        if !ids.insert(&todo.id) {
            return Err("Danh sách việc có mã bị trùng.".into());
        }
        if todo.text.trim().is_empty() {
            return Err("Nội dung việc cần làm không được để trống.".into());
        }
        validate_text(&todo.text, 1_024, "Nội dung việc cần làm")?;
    }
    Ok(())
}

impl NoteInput {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(id) = &self.id {
            validate_id(id)?;
        }
        validate_text(&self.title, 160, "Tiêu đề")?;
        validate_text(&self.body, 32_000, "Nội dung ghi chú")?;
        validate_todos(&self.todos)?;
        if self.title.trim().is_empty() && self.body.trim().is_empty() && self.todos.is_empty() {
            return Err("Hãy nhập nội dung hoặc thêm việc cần làm trước khi lưu.".into());
        }
        Ok(())
    }
}

impl Snapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.notes.len() > MAX_NOTES
            || self.bookmarks.len() > MAX_BOOKMARKS
            || self.categories.len() > MAX_CATEGORIES
        {
            return Err("Dữ liệu vượt giới hạn số ghi chú, dấu trang hoặc nhóm.".into());
        }
        let mut ids = HashSet::new();
        for note in &self.notes {
            validate_id(&note.id)?;
            if !ids.insert(&note.id) {
                return Err("Dữ liệu có mã ghi chú bị trùng.".into());
            }
            NoteInput {
                id: Some(note.id.clone()),
                title: note.title.clone(),
                body: note.body.clone(),
                pinned: note.pinned,
                todos: note.todos.clone(),
            }
            .validate()?;
            if note.updated_at < note.created_at {
                return Err("Thời gian ghi chú không hợp lệ.".into());
            }
        }
        let mut category_ids = HashSet::new();
        let mut category_names = HashSet::new();
        for category in &self.categories {
            validate_id(&category.id)?;
            validate_category(&category.name, &category.color)?;
            if !category_ids.insert(category.id.as_str())
                || !category_names.insert(category.name.to_lowercase())
            {
                return Err("Dữ liệu có nhóm bị trùng.".into());
            }
        }
        ids.clear();
        for bookmark in &self.bookmarks {
            validate_id(&bookmark.id)?;
            validate_label(&bookmark.label)?;
            validate_path_input(&bookmark.path)?;
            if !ids.insert(&bookmark.id) {
                return Err("Dữ liệu có mã dấu trang bị trùng.".into());
            }
            if let Some(id) = &bookmark.category_id {
                if !category_ids.contains(id.as_str()) {
                    return Err("Dấu trang thuộc nhóm không còn tồn tại.".into());
                }
            }
        }
        let bytes = serde_json::to_vec(self)
            .map_err(|error| format!("Không thể kiểm tra dữ liệu: {error}"))?;
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err("Tổng dữ liệu ghi chú và dấu trang không được quá 8 MB.".into());
        }
        Ok(())
    }

    pub fn save_note(&mut self, input: NoteInput, id: String, now: u64) -> Result<Note, String> {
        input.validate()?;
        let previous = match input.id.as_deref() {
            Some(existing) => Some(
                self.notes
                    .iter()
                    .find(|note| note.id == existing)
                    .ok_or("Ghi chú không còn tồn tại. Hãy tải lại danh sách.")?
                    .clone(),
            ),
            None => None,
        };
        if previous.is_none() && self.notes.len() >= MAX_NOTES {
            return Err("Bạn đã đạt giới hạn 1.000 ghi chú.".into());
        }
        let note = Note {
            id: previous.as_ref().map_or(id, |note| note.id.clone()),
            title: if input.title.trim().is_empty() {
                "Ghi chú mới".into()
            } else {
                input.title.trim().into()
            },
            body: input.body,
            pinned: input.pinned,
            todos: input
                .todos
                .into_iter()
                .map(|todo| Todo {
                    text: todo.text.trim().into(),
                    ..todo
                })
                .collect(),
            created_at: previous.as_ref().map_or(now, |note| note.created_at),
            updated_at: previous
                .as_ref()
                .map_or(now, |note| now.max(note.updated_at.saturating_add(1))),
        };
        if let Some(index) = self.notes.iter().position(|entry| entry.id == note.id) {
            self.notes[index] = note.clone();
        } else {
            self.notes.insert(0, note.clone());
        }
        Ok(note)
    }

    pub fn save_category(
        &mut self,
        id: Option<String>,
        generated_id: String,
        name: String,
        color: String,
    ) -> Result<Category, String> {
        let name = name.trim().to_string();
        validate_category(&name, &color)?;
        if self.categories.iter().any(|category| {
            category.name.to_lowercase() == name.to_lowercase()
                && Some(category.id.as_str()) != id.as_deref()
        }) {
            return Err("Tên nhóm này đã được dùng.".into());
        }
        let index = match &id {
            Some(id) => Some(
                self.categories
                    .iter()
                    .position(|entry| entry.id == *id)
                    .ok_or("Nhóm không còn tồn tại.")?,
            ),
            None => None,
        };
        if index.is_none() && self.categories.len() >= MAX_CATEGORIES {
            return Err("Bạn đã đạt giới hạn 50 nhóm dấu trang.".into());
        }
        let category = Category {
            id: id.unwrap_or(generated_id),
            name,
            color,
        };
        if let Some(index) = index {
            self.categories[index] = category.clone();
        } else {
            self.categories.push(category.clone());
        }
        Ok(category)
    }

    pub fn delete_category(&mut self, id: &str) -> Result<(), String> {
        let index = self
            .categories
            .iter()
            .position(|entry| entry.id == id)
            .ok_or("Nhóm không còn tồn tại.")?;
        self.categories.remove(index);
        for bookmark in &mut self.bookmarks {
            if bookmark.category_id.as_deref() == Some(id) {
                bookmark.category_id = None;
            }
        }
        Ok(())
    }

    pub fn validate_category_id(&self, id: Option<&str>) -> Result<(), String> {
        if let Some(id) = id {
            validate_id(id)?;
            if !self.categories.iter().any(|category| category.id == id) {
                return Err("Nhóm dấu trang không còn tồn tại.".into());
            }
        }
        Ok(())
    }
}

pub fn validate_label(label: &str) -> Result<(), String> {
    if label.trim().is_empty() {
        return Err("Tên dấu trang không được để trống.".into());
    }
    validate_text(label, 240, "Tên dấu trang")
}

pub fn validate_category(name: &str, color: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Tên nhóm không được để trống.".into());
    }
    validate_text(name, 60, "Tên nhóm")?;
    if !CATEGORY_COLORS.contains(&color) {
        return Err("Màu nhóm không được hỗ trợ.".into());
    }
    Ok(())
}

pub fn validate_path_input(path: &str) -> Result<(), String> {
    if path.is_empty() || path.len() > 32_767 || path.chars().any(char::is_control) {
        return Err("Đường dẫn không hợp lệ.".into());
    }
    let lower = path.to_ascii_lowercase();
    if lower.starts_with("\\\\.\\")
        || lower.starts_with("\\\\?\\")
        || lower.starts_with("\\??\\")
        || lower.contains("://")
    {
        return Err("Chỉ chấp nhận đường dẫn tệp hoặc thư mục thông thường.".into());
    }
    if !std::path::Path::new(path).is_absolute() {
        return Err("Dấu trang cần một đường dẫn đầy đủ.".into());
    }
    // On Windows, an extra colon can refer to a device or NTFS alternate stream.
    #[cfg(windows)]
    if path
        .char_indices()
        .any(|(index, character)| character == ':' && index != 1)
    {
        return Err("Không hỗ trợ thiết bị hoặc luồng dữ liệu đặc biệt.".into());
    }
    Ok(())
}

pub fn commit_change<T>(
    data: &mut Snapshot,
    change: impl FnOnce(&mut Snapshot) -> Result<T, String>,
    persist: impl FnOnce(&Snapshot) -> Result<(), String>,
) -> Result<(T, bool), String> {
    let mut candidate = data.clone();
    let value = change(&mut candidate)?;
    candidate.validate()?;
    let changed = candidate != *data;
    if changed {
        persist(&candidate)?;
        *data = candidate;
    }
    Ok((value, changed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> NoteInput {
        NoteInput {
            id: None,
            title: "Ghi chú".into(),
            body: "Nội dung".into(),
            pinned: false,
            todos: vec![Todo {
                id: "todo-1".into(),
                text: "Làm việc".into(),
                done: false,
            }],
        }
    }

    #[test]
    fn failed_persistence_cannot_publish_a_note_change() {
        let mut snapshot = Snapshot::default();
        let result = commit_change(
            &mut snapshot,
            |data| data.save_note(input(), "note-1".into(), 10),
            |_| Err("Ổ đĩa đầy".into()),
        );
        assert!(result.is_err());
        assert!(snapshot.notes.is_empty());
    }

    #[test]
    fn editing_a_note_preserves_identity_and_creation_time() {
        let mut snapshot = Snapshot::default();
        snapshot.save_note(input(), "note-1".into(), 100).unwrap();
        let mut edited = input();
        edited.id = Some("note-1".into());
        edited.pinned = true;
        edited.todos[0].done = true;
        let note = snapshot.save_note(edited, "ignored-id".into(), 90).unwrap();
        assert_eq!(note.id, "note-1");
        assert_eq!(note.created_at, 100);
        assert_eq!(note.updated_at, 101);
        assert!(note.pinned && note.todos[0].done);
        assert_eq!(snapshot.notes.len(), 1);
    }

    #[test]
    fn stale_edits_and_duplicate_todo_ids_are_rejected() {
        let mut snapshot = Snapshot::default();
        let mut stale = input();
        stale.id = Some("deleted-note".into());
        assert!(snapshot.save_note(stale, "unused".into(), 1).is_err());
        let mut duplicate = input();
        duplicate.todos.push(duplicate.todos[0].clone());
        assert!(duplicate.validate().is_err());
    }

    #[test]
    fn deleting_a_category_keeps_bookmarks_and_their_files() {
        let mut data = Snapshot::default();
        data.save_category(None, "cat-1".into(), "Công việc".into(), "blue".into())
            .unwrap();
        data.bookmarks.push(Bookmark {
            id: "bookmark-1".into(),
            label: "Tài liệu".into(),
            path: "not-opened-by-test".into(),
            kind: BookmarkKind::File,
            category_id: Some("cat-1".into()),
            created_at: 1,
        });
        data.delete_category("cat-1").unwrap();
        assert_eq!(data.bookmarks.len(), 1);
        assert!(data.bookmarks[0].category_id.is_none());
    }

    #[test]
    fn paths_cannot_be_urls_devices_or_relative_commands() {
        for path in [
            "https://example.com",
            "file:///C:/notes",
            "cmd.exe",
            "../secrets",
            "\\\\.\\PhysicalDrive0",
            "\\\\?\\GLOBALROOT\\Device\\test",
            "C:\\bad\nname",
        ] {
            assert!(validate_path_input(path).is_err(), "accepted {path:?}");
        }
    }

    #[test]
    fn invalid_category_and_empty_notes_do_not_mutate_data() {
        let mut data = Snapshot::default();
        assert!(data
            .save_category(None, "cat-1".into(), " ".into(), "blue".into())
            .is_err());
        let empty = NoteInput {
            id: None,
            title: String::new(),
            body: String::new(),
            pinned: false,
            todos: vec![],
        };
        assert!(data.save_note(empty, "note-1".into(), 1).is_err());
        assert!(data.categories.is_empty() && data.notes.is_empty());
    }
}
