//! Repository contracts exercised against owned temporary files, without Tauri,
//! user AppData, Windows clipboard access, or background listener threads.

use super::*;

struct Fixture {
    root: PathBuf,
    resolved_root: PathBuf,
    temporary_parent: PathBuf,
    directory: PathBuf,
    cache: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temporary_parent = fs::canonicalize(std::env::temp_dir()).expect("temporary directory");
        let root = temporary_parent.join(format!(
            "zone-clipboard-repository-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create uniquely owned fixture");
        let resolved_root = fs::canonicalize(&root).expect("resolved fixture");
        Self {
            directory: root.join("data"),
            cache: root.join("cache"),
            root,
            resolved_root,
            temporary_parent,
        }
    }

    fn open(&self) -> (Repository, Data) {
        Repository::open_directory(&self.directory, &self.cache).expect("open fixture repository")
    }

    fn document(&self) -> Vec<u8> {
        fs::read(self.directory.join("clipboard.json")).expect("read fixture history")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Verify the exact resolved target before recursive removal. This test
        // owns only a direct, uniquely named child of the temporary directory.
        let Ok(metadata) = fs::symlink_metadata(&self.root) else {
            return;
        };
        let Ok(resolved) = fs::canonicalize(&self.root) else {
            return;
        };
        let prefix = format!("zone-clipboard-repository-{}-", std::process::id());
        if !metadata.file_type().is_symlink()
            && resolved == self.resolved_root
            && resolved.parent() == Some(self.temporary_parent.as_path())
            && resolved
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
        {
            let _ = fs::remove_dir_all(resolved);
        }
    }
}

fn text_entry(id: &str, pinned: bool, text: &str) -> StoredItem {
    StoredItem {
        item: ClipboardItem {
            id: id.into(),
            kind: "text".into(),
            text: Some(text.into()),
            preview: text.chars().take(240).collect(),
            image_path: None,
            image_width: None,
            image_height: None,
            files: vec![],
            pinned,
            created_at: 42,
        },
        fingerprint: fingerprint(text),
        encoded_bytes: 0,
    }
}

fn image_entry(repository: &Repository, id: &str, pinned: bool) -> StoredItem {
    let image = image_data::ClipboardImage {
        width: 1,
        height: 1,
        rgba: vec![30, 60, 90, 255],
    };
    let original = image_data::encode_png(&image).expect("encode original fixture");
    let preview = image_data::encode_thumbnail(&image).expect("encode preview fixture");
    let original_path = repository
        .expected_image_path(id)
        .expect("owned original path");
    let preview_path = repository
        .expected_thumbnail_path(id)
        .expect("owned preview path");
    atomic_write(&original_path, &original).expect("write original fixture");
    atomic_write(&preview_path, &preview).expect("write preview fixture");
    StoredItem {
        item: ClipboardItem {
            id: id.into(),
            kind: "image".into(),
            text: None,
            preview: "Ảnh 1 × 1".into(),
            image_path: Some(original_path.to_string_lossy().into_owned()),
            image_width: Some(1),
            image_height: Some(1),
            files: vec![],
            pinned,
            created_at: 42,
        },
        fingerprint: fingerprint(&original),
        encoded_bytes: (original.len() + preview.len()) as u64,
    }
}

fn ids(data: &Data) -> Vec<&str> {
    data.items
        .iter()
        .map(|entry| entry.item.id.as_str())
        .collect()
}

#[test]
fn default_disk_projection_restores_unicode_pins_but_not_session_text() {
    let fixture = Fixture::new();
    let (repository, mut data) = fixture.open();
    data.items = vec![
        text_entry("clip-10-1", false, "session-only private fixture"),
        text_entry("clip-10-2", true, "Ghi nhớ tiếng Việt 🏝"),
        image_entry(&repository, "clip-10-3", true),
    ];
    repository.persist(&data).expect("persist pins projection");
    assert!(!String::from_utf8(fixture.document())
        .unwrap()
        .contains("session-only"));
    drop(repository);
    let (reopened, restored) = fixture.open();
    assert_eq!(ids(&restored), ["clip-10-2", "clip-10-3"]);
    assert_eq!(
        restored.items[0].item.text.as_deref(),
        Some("Ghi nhớ tiếng Việt 🏝")
    );
    assert_eq!(restored.settings, ClipboardSettings::default());
    assert!(reopened.open_image("clip-10-3").is_ok());
    assert!(reopened.read_thumbnail("clip-10-3").is_ok());
}

#[test]
fn opting_out_of_full_history_removes_session_data_from_the_next_restart() {
    let fixture = Fixture::new();
    let (repository, mut data) = fixture.open();
    data.settings.persist_history = true;
    data.items = vec![
        text_entry("clip-20-1", true, "pin"),
        image_entry(&repository, "clip-20-2", false),
    ];
    repository.persist(&data).expect("persist opted-in history");
    drop(repository);
    let (repository, mut restored) = fixture.open();
    assert_eq!(ids(&restored), ["clip-20-1", "clip-20-2"]);
    restored.settings.persist_history = false;
    repository
        .persist(&restored)
        .expect("persist opt-out projection");
    // Same primitive used by normal shutdown; the session item still exists in
    // memory until then, but must not be recovered from disk on next startup.
    repository
        .prune_orphans(&restored.persisted())
        .expect("shutdown cache cleanup");
    assert!(!repository
        .expected_image_path("clip-20-2")
        .unwrap()
        .exists());
    assert!(!repository
        .expected_thumbnail_path("clip-20-2")
        .unwrap()
        .exists());
    drop(repository);
    let (_, restored) = fixture.open();
    assert_eq!(ids(&restored), ["clip-20-1"]);
    assert!(!restored.settings.persist_history);
}

#[test]
fn unpinning_an_image_keeps_it_in_session_but_releases_its_persistent_cache() {
    let fixture = Fixture::new();
    let (repository, mut data) = fixture.open();
    data.items.push(image_entry(&repository, "clip-30-1", true));
    repository.persist(&data).expect("persist pinned image");
    data.items[0].item.pinned = false;
    repository.persist(&data).expect("persist unpin");
    repository
        .prune_orphans(&data)
        .expect("retain session image");
    assert!(repository.open_image("clip-30-1").is_ok());
    repository
        .prune_orphans(&data.persisted())
        .expect("release session cache on shutdown");
    assert!(!repository
        .expected_image_path("clip-30-1")
        .unwrap()
        .exists());
    drop(repository);
    let (_, restored) = fixture.open();
    assert!(restored.items.is_empty());
}

#[test]
fn retained_history_controls_individual_and_clear_cache_cleanup() {
    let fixture = Fixture::new();
    let (repository, mut data) = fixture.open();
    data.settings.persist_history = true;
    data.items = vec![
        image_entry(&repository, "clip-40-1", true),
        image_entry(&repository, "clip-40-2", false),
        text_entry("clip-40-3", true, "keep pin"),
    ];
    repository.persist(&data).expect("persist full fixture");
    data.items.remove(0);
    repository.persist(&data).expect("persist removed item");
    repository
        .prune_orphans(&data)
        .expect("prune removed item's cache");
    assert!(!repository
        .expected_image_path("clip-40-1")
        .unwrap()
        .exists());
    assert!(!repository
        .expected_thumbnail_path("clip-40-1")
        .unwrap()
        .exists());
    assert!(repository.open_image("clip-40-2").is_ok());
    data.items.remove(0);
    repository
        .persist(&data)
        .expect("persist history keeping remaining pin");
    repository
        .prune_orphans(&data)
        .expect("prune remaining unpinned image");
    assert!(!repository
        .expected_image_path("clip-40-2")
        .unwrap()
        .exists());
    assert!(!repository
        .expected_thumbnail_path("clip-40-2")
        .unwrap()
        .exists());
    drop(repository);
    let (repository, mut restored) = fixture.open();
    assert_eq!(ids(&restored), ["clip-40-3"]);
    restored.items.clear();
    repository
        .persist(&restored)
        .expect("persist explicit empty history");
    repository
        .prune_orphans(&restored)
        .expect("prune empty history");
    drop(repository);
    let (_, restored) = fixture.open();
    assert!(restored.items.is_empty());
}

#[test]
fn missing_image_files_do_not_destroy_pinned_metadata_on_reopen() {
    let fixture = Fixture::new();
    let (repository, mut data) = fixture.open();
    data.items.push(image_entry(&repository, "clip-50-1", true));
    repository.persist(&data).expect("persist pinned fixture");
    fs::remove_file(repository.expected_image_path("clip-50-1").unwrap()).unwrap();
    fs::remove_file(repository.expected_thumbnail_path("clip-50-1").unwrap()).unwrap();
    drop(repository);
    let (repository, restored) = fixture.open();
    assert_eq!(ids(&restored), ["clip-50-1"]);
    assert!(restored.items[0].item.pinned);
    assert!(repository.open_image("clip-50-1").is_err());
    assert!(repository.read_thumbnail("clip-50-1").is_err());
    assert!(String::from_utf8(fixture.document())
        .unwrap()
        .contains("clip-50-1"));
}

#[test]
fn thumbnail_read_rejects_path_components_and_bounds_png_data() {
    let fixture = Fixture::new();
    let (repository, _) = fixture.open();
    assert!(repository.read_thumbnail("../clip-51-1").is_err());
    let path = repository.expected_thumbnail_path("clip-51-1").unwrap();
    let too_wide = image_data::ClipboardImage {
        width: image_data::THUMBNAIL_SIDE + 1,
        height: 1,
        rgba: vec![255; (image_data::THUMBNAIL_SIDE as usize + 1) * 4],
    };
    let bytes = image_data::encode_png(&too_wide).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(repository.read_thumbnail("clip-51-1").is_err());
    OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_len(image_data::MAX_THUMBNAIL_BYTES as u64 + 1)
        .unwrap();
    assert!(repository.read_thumbnail("clip-51-1").is_err());
}

#[test]
fn reopening_prunes_owned_crash_files_without_touching_foreign_files_or_directories() {
    let fixture = Fixture::new();
    let (repository, _) = fixture.open();
    let stale = [
        "clip-60-1.png",
        "clip-60-1.thumb.png",
        "clip-60-2.22.33.tmp",
        "clip-60-2.thumb.22.33.tmp",
    ];
    let foreign = ["notes.tmp", "photo.png", "clip-60-2.invalid.33.tmp"];
    for name in stale.iter().chain(foreign.iter()) {
        fs::write(fixture.cache.join(name), b"owned fixture").unwrap();
    }
    let directory = fixture.cache.join("clip-60-3.png");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("keep.txt"), b"do not recurse").unwrap();
    drop(repository);
    let (_, restored) = fixture.open();
    assert!(restored.items.is_empty());
    for name in stale {
        assert!(!fixture.cache.join(name).exists());
    }
    for name in foreign {
        assert_eq!(
            fs::read(fixture.cache.join(name)).unwrap(),
            b"owned fixture"
        );
    }
    assert_eq!(
        fs::read(directory.join("keep.txt")).unwrap(),
        b"do not recurse"
    );
}

#[test]
fn invalid_documents_are_reported_without_replacing_the_original_or_pruning_cache() {
    let fixture = Fixture::new();
    let (repository, _) = fixture.open();
    drop(repository);
    fs::write(
        fixture.cache.join("clip-70-1.png"),
        b"preserve cache until document is repaired",
    )
    .unwrap();
    for bytes in [
        b"{broken json".as_slice(),
        br#"{"data":{"settings":{"historyLimit":19},"items":[]}}"#.as_slice(),
    ] {
        fs::write(fixture.directory.join("clipboard.json"), bytes).unwrap();
        assert!(Repository::open_directory(&fixture.directory, &fixture.cache).is_err());
        assert_eq!(fixture.document(), bytes);
        assert!(fixture.cache.join("clip-70-1.png").exists());
    }
}

#[test]
fn repository_lease_rejects_a_second_writer_and_is_released_on_drop() {
    let fixture = Fixture::new();
    let (repository, mut data) = fixture.open();
    data.items
        .push(text_entry("clip-80-1", true, "first writer"));
    repository.persist(&data).unwrap();
    assert!(Repository::open_directory(&fixture.directory, &fixture.cache).is_err());
    drop(repository);
    let (_, restored) = fixture.open();
    assert_eq!(restored.items[0].item.text.as_deref(), Some("first writer"));
}

#[cfg(windows)]
#[test]
fn failed_atomic_replace_preserves_the_old_file_and_removes_temporary_output() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    let (repository, mut data) = fixture.open();
    data.items.push(text_entry("clip-90-1", true, "original"));
    repository.persist(&data).unwrap();
    let original = fixture.document();
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&repository.path)
        .expect("exclusive fixture file handle");
    data.items[0].item.text = Some("replacement must not publish".into());
    assert!(repository.persist(&data).is_err());
    drop(locked);
    assert_eq!(fixture.document(), original);
    assert!(fs::read_dir(&fixture.directory).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".tmp")));
    drop(repository);
    let (_, restored) = fixture.open();
    assert_eq!(restored.items[0].item.text.as_deref(), Some("original"));
}
