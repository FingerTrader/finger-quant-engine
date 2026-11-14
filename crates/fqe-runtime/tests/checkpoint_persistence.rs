use fqe_ir::{
    checkpoint::{JournalEntry, SnapshotCommand, SnapshotDocument},
    model::ContentId,
};
use fqe_runtime::checkpoint_file;
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NONCE: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fqe-checkpoint-test-{}-{}",
            std::process::id(),
            NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn images() -> (Vec<u8>, Vec<u8>) {
    let mut doc = SnapshotDocument::from_bytes(include_bytes!(
        "../../fqe-ir/tests/fixtures/checkpoint-independent.json"
    ))
    .unwrap();
    let old = doc.to_bytes().unwrap();
    doc.entries.push(JournalEntry {
        command: SnapshotCommand::Drive,
        outcome_id: ContentId::from_bytes([3; 32]),
        output_cursor: 1,
    });
    (old, doc.to_bytes().unwrap())
}
#[test]
fn prepare_drop_and_atomic_commit_preserve_complete_valid_files() {
    let dir = Temp::new();
    let path = dir.0.join("state.json");
    let (old, new) = images();
    checkpoint_file::save(&path, &old).unwrap();
    let pending = checkpoint_file::prepare(&path, &new).unwrap();
    assert_eq!(checkpoint_file::read(&path).unwrap(), old);
    drop(pending);
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
    let receipt = checkpoint_file::save(&path, &new).unwrap();
    assert_eq!(receipt.bytes, new.len());
    assert_eq!(checkpoint_file::read(&path).unwrap(), new);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o077,
            0
        );
    }
    let e = checkpoint_file::save(&path, b"truncated").unwrap_err();
    assert!(!e.committed);
    assert_eq!(checkpoint_file::read(&path).unwrap(), new);
    assert!(!checkpoint_file::save(&dir.0, &new).unwrap_err().committed);
}
#[test]
fn checkpoint_crash_worker() {
    let Some(path) = std::env::var_os("FQE_CHECKPOINT_CRASH_TARGET") else {
        return;
    };
    let (_, new) = images();
    let _pending = checkpoint_file::prepare(Path::new(&path), &new).unwrap();
    std::process::exit(73);
}
#[test]
fn process_exit_after_temp_sync_before_rename_retains_previous_committed_checkpoint() {
    let dir = Temp::new();
    let path = dir.0.join("state.json");
    let (old, new) = images();
    checkpoint_file::save(&path, &old).unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "checkpoint_crash_worker", "--nocapture"])
        .env("FQE_CHECKPOINT_CRASH_TARGET", &path)
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(73),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(checkpoint_file::read(&path).unwrap(), old);
    assert_eq!(
        std::fs::read_dir(&dir.0).unwrap().count(),
        2,
        "abrupt child exit leaves only uncommitted sibling temp"
    );
    checkpoint_file::save(&path, &new).unwrap();
    assert_eq!(checkpoint_file::read(&path).unwrap(), new);
}
#[cfg(unix)]
#[test]
fn symlink_and_oversized_inputs_are_rejected() {
    let dir = Temp::new();
    let path = dir.0.join("state.json");
    let alias = dir.0.join("alias.json");
    let (old, new) = images();
    checkpoint_file::save(&path, &old).unwrap();
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    assert!(checkpoint_file::read(&alias).is_err());
    assert!(!checkpoint_file::save(&alias, &new).unwrap_err().committed);
    assert_eq!(checkpoint_file::read(&path).unwrap(), old);
    let huge = dir.0.join("huge.json");
    std::fs::write(&huge, vec![b' '; 262145]).unwrap();
    assert!(checkpoint_file::read(&huge).is_err());
}
