//! Same-directory atomic checkpoint publication; a post-rename sync failure is explicitly committed.
use fqe_ir::{checkpoint::SnapshotDocument, model::MAX_MODEL_BYTES};
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
#[derive(Debug)]
pub struct PersistenceError {
    pub stage: &'static str,
    pub committed: bool,
    pub detail: String,
}
impl fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "checkpoint {} (committed={}): {}",
            self.stage, self.committed, self.detail
        )
    }
}
impl std::error::Error for PersistenceError {}
fn error(stage: &'static str, committed: bool, e: impl fmt::Display) -> PersistenceError {
    PersistenceError {
        stage,
        committed,
        detail: e.to_string(),
    }
}
static NONCE: AtomicU64 = AtomicU64::new(0);
pub struct PreparedCheckpoint {
    temporary: PathBuf,
    target: PathBuf,
    bytes: usize,
    committed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveReceipt {
    pub target: PathBuf,
    pub bytes: usize,
}
fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}
fn destination(path: &Path) -> Result<(), PersistenceError> {
    if path.file_name().is_none() {
        return Err(error("destination", false, "regular filename required"));
    }
    match fs::symlink_metadata(path) {
        Ok(m) if !m.is_file() || m.file_type().is_symlink() => Err(error(
            "destination",
            false,
            "target must be a regular file or absent",
        )),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(error("destination", false, e)),
    }
}
pub fn prepare(
    path: impl AsRef<Path>,
    bytes: &[u8],
) -> Result<PreparedCheckpoint, PersistenceError> {
    SnapshotDocument::from_bytes(bytes).map_err(|e| error("validate", false, e))?;
    let path = path.as_ref();
    destination(path)?;
    let mut created = None;
    for _ in 0..16 {
        let temporary = parent(path).join(format!(
            ".fqe-checkpoint-{}-{}.tmp",
            std::process::id(),
            NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temporary) {
            Ok(file) => {
                created = Some((temporary, file));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(error("create", false, e)),
        }
    }
    let (temporary, mut file) =
        created.ok_or_else(|| error("create", false, "bounded temp-name attempts exhausted"))?;
    let pending = PreparedCheckpoint {
        temporary,
        target: path.to_path_buf(),
        bytes: bytes.len(),
        committed: false,
    };
    file.write_all(bytes)
        .map_err(|e| error("write", false, e))?;
    file.sync_all().map_err(|e| error("file-sync", false, e))?;
    drop(file);
    Ok(pending)
}
impl PreparedCheckpoint {
    pub fn commit(mut self) -> Result<SaveReceipt, PersistenceError> {
        destination(&self.target)?;
        fs::rename(&self.temporary, &self.target).map_err(|e| error("rename", false, e))?;
        self.committed = true;
        File::open(parent(&self.target))
            .and_then(|directory| directory.sync_all())
            .map_err(|e| error("directory-sync", true, e))?;
        Ok(SaveReceipt {
            target: self.target.clone(),
            bytes: self.bytes,
        })
    }
}
impl Drop for PreparedCheckpoint {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.temporary);
        }
    }
}
pub fn save(path: impl AsRef<Path>, bytes: &[u8]) -> Result<SaveReceipt, PersistenceError> {
    prepare(path, bytes)?.commit()
}
pub fn read(path: impl AsRef<Path>) -> Result<Vec<u8>, PersistenceError> {
    let path = path.as_ref();
    let metadata = fs::symlink_metadata(path).map_err(|e| error("read", false, e))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(error("read", false, "regular checkpoint file required"));
    }
    let file = File::open(path).map_err(|e| error("read", false, e))?;
    let mut bytes = vec![];
    file.take(MAX_MODEL_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| error("read", false, e))?;
    SnapshotDocument::from_bytes(&bytes).map_err(|e| error("validate", false, e))?;
    Ok(bytes)
}
