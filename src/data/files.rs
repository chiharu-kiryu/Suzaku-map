//! Bounded regular-file reads, private atomic writes and a cooperative maintenance gate.
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const SETTINGS_LIMIT: usize = 65_536;
static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

pub fn private_directory(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

pub fn read_optional(path: &Path, limit: usize) -> io::Result<Option<String>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Do not hang on a FIFO or follow an unexpected final-component symlink.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("expected a regular file"));
    }
    let mut contents = String::new();
    file.take(limit as u64 + 1).read_to_string(&mut contents)?;
    if contents.len() > limit {
        return Err(io::Error::other("file exceeds the size limit"));
    }
    Ok(Some(contents))
}

/// The temporary file is never shared with another writer and is removed on failure.
pub struct StagedFile {
    temporary: PathBuf,
    destination: PathBuf,
}

impl StagedFile {
    pub fn new(destination: &Path, bytes: &[u8]) -> io::Result<Self> {
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        private_directory(parent)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        for _ in 0..32 {
            let temporary = parent.join(format!(
                ".suzaku-{}-{}.tmp",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            let mut file = match options.open(&temporary) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            };
            let staged = Self {
                temporary,
                destination: destination.into(),
            };
            file.write_all(bytes)?;
            file.sync_all()?;
            return Ok(staged);
        }
        Err(io::Error::other("cannot reserve a temporary file"))
    }

    pub fn replace(self) -> io::Result<()> {
        fs::rename(&self.temporary, &self.destination)
    }

    pub fn create(self) -> io::Result<()> {
        // An atomic no-clobber publish, unlike an exists() check followed by rename().
        fs::hard_link(&self.temporary, &self.destination)?;
        #[cfg(unix)]
        File::open(
            self.destination
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )?
        .sync_all()?;
        Ok(())
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.temporary);
    }
}

pub fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    StagedFile::new(path, contents)?.replace()
}

/// New Linux panel/host processes hold shared leases for their entire lifetime.
/// Restore takes an exclusive lease, so a concurrent start or settings write fails safely.
pub struct DataLease(Vec<File>);

impl DataLease {
    pub fn current_shared() -> Result<Self, String> {
        // A standalone SUZAKU_IME_CONFIG must keep working even without HOME/data_home.
        let path = crate::ime::settings::settings_path().ok_or("无法定位输入法设置")?;
        Self::settings_shared(&path)
    }

    /// Hold every final-component alias and its read target. Atomic saves replace
    /// the named entry, not necessarily the final target, so retain both gates.
    pub fn settings_shared(settings: &Path) -> Result<Self, String> {
        Self::for_settings(
            settings,
            false,
            false,
            "Suzaku 正在运行或管理数据；请退出面板、停止输入法宿主后再恢复",
        )
    }

    pub fn acquire(path: &Path, exclusive: bool) -> Result<Self, String> {
        Self::acquire_with_message(
            path,
            exclusive,
            "Suzaku 正在运行或管理数据；请退出面板、停止输入法宿主后再恢复",
        )
    }

    /// Separate from the lifetime/restore lease: serialize cooperating settings
    /// writers without upgrading a live host's shared lease or blocking input.
    pub fn settings_writer(settings: &Path) -> Result<Self, String> {
        Self::for_settings(settings, true, true, "输入法设置正在保存，请稍后重试")
    }

    fn for_settings(
        settings: &Path,
        exclusive: bool,
        writer: bool,
        busy: &str,
    ) -> Result<Self, String> {
        let paths = settings_lock_paths(settings)?;
        let mut lease = Self(Vec::new());
        // Sorted, canonical directory paths avoid acquiring the same inode twice
        // through directory aliases. Never wait or retain a partially acquired set.
        for path in &paths {
            let path = if writer {
                path.with_file_name(".suzaku-settings-write.lock")
            } else {
                path.clone()
            };
            let mut next = Self::acquire_with_message(&path, exclusive, busy)?;
            lease.0.append(&mut next.0);
        }
        // A cooperative atomic replacement can shorten the chain. Keep its old
        // gates, but reject a newly observed target which this lease does not cover.
        if !settings_lock_paths(settings)?.is_subset(&paths) {
            return Err("配置链接目标已更改；请重试，运行中更换链接后请重启宿主和面板".into());
        }
        Ok(lease)
    }

    fn acquire_with_message(path: &Path, exclusive: bool, busy: &str) -> Result<Self, String> {
        let parent = path.parent().ok_or("invalid lock path")?;
        private_directory(parent).map_err(|error| error.to_string())?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = options.open(path).map_err(|error| error.to_string())?;
        if !file
            .metadata()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            return Err("invalid data lock file".into());
        }
        let result = if exclusive {
            file.try_lock()
        } else {
            file.try_lock_shared()
        };
        result.map_err(|_| busy.to_string())?;
        Ok(Self(vec![file]))
    }
}

impl Drop for DataLease {
    fn drop(&mut self) {
        for file in &self.0 {
            let _ = file.unlock();
        }
    }
}

fn settings_lock_paths(settings: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let first = super::paths::lock_for_settings(settings)?;
    // A fresh ordinary configuration may not have a directory yet. Only create
    // the explicitly selected parent; unresolved link target parents fail closed.
    private_directory(first.parent().ok_or("invalid lock path")?)
        .map_err(|error| error.to_string())?;
    let mut path = settings.to_path_buf();
    let mut visited = BTreeSet::new();
    let mut locks = BTreeSet::new();
    for _ in 0..=40 {
        let lock = super::paths::lock_for_settings(&path)?;
        let parent = fs::canonicalize(lock.parent().ok_or("invalid lock path")?)
            .map_err(|error| format!("无法保护输入法配置路径：{error}"))?;
        path = parent.join(path.file_name().ok_or("设置路径必须指向文件")?);
        if !visited.insert(path.clone()) {
            return Err("输入法配置包含循环链接；请使用真实配置路径".into());
        }
        locks.insert(parent.join(".suzaku-data.lock"));
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let target =
                    fs::read_link(&path).map_err(|error| format!("无法读取配置链接：{error}"))?;
                path = path.parent().ok_or("设置路径没有父目录")?.join(target);
            }
            Ok(_) => return Ok(locks),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(locks),
            Err(error) => return Err(format!("无法保护输入法配置路径：{error}")),
        }
    }
    Err("输入法配置链接层数过多；请使用真实配置路径".into())
}
