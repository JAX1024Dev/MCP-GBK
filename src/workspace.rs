use crate::error::{AppError, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// Refuse special files and final-component links without blocking on FIFOs.
pub fn open_file(path: &Path) -> Result<(fs::File, fs::Metadata)> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || is_link(&metadata) {
        return Err(AppError::new(
            "not_a_file",
            "Only regular, non-linked files are supported",
        ));
    }
    Ok((file, metadata))
}

#[derive(Clone)]
pub struct Workspace {
    pub root: Arc<PathBuf>,
    pub writes: Arc<Mutex<()>>,
}

impl Workspace {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().canonicalize()?;
        if !root.is_dir() {
            return Err(AppError::new(
                "invalid_root",
                "Workspace root must be a directory",
            ));
        }
        Ok(Self {
            root: Arc::new(root),
            writes: Arc::new(Mutex::new(())),
        })
    }

    pub fn resolve(&self, path: &str, new_file: bool) -> Result<PathBuf> {
        let candidate = self.root.join(path);
        let exists = candidate.try_exists()?;
        let resolved = if exists {
            candidate.canonicalize()?
        } else if new_file {
            let mut ancestor = candidate.as_path();
            let mut missing = Vec::new();
            while !ancestor.try_exists()? {
                missing.push(
                    ancestor
                        .file_name()
                        .ok_or_else(|| {
                            AppError::new(
                                "invalid_path",
                                "Missing path components must be normal names",
                            )
                        })?
                        .to_owned(),
                );
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| AppError::new("invalid_path", "Parent directory is missing"))?;
            }
            let mut resolved = ancestor.canonicalize()?;
            for part in missing.into_iter().rev() {
                resolved.push(part);
            }
            resolved
        } else {
            return Err(AppError::new(
                "not_found",
                format!("Path not found: {path}"),
            ));
        };
        if !path_is_within(&resolved, self.root.as_ref())? {
            return Err(AppError::new(
                "outside_root",
                "Path escapes the configured workspace root",
            ));
        }
        // Inspect the original path as canonicalize would hide links and junctions.
        for ancestor in candidate.ancestors() {
            if let Ok(metadata) = fs::symlink_metadata(ancestor)
                && is_link(&metadata)
            {
                // Root ancestors may be OS aliases (/var -> /private/var on macOS).
                if let Ok(canonical) = ancestor.canonicalize()
                    && path_is_within(&canonical, self.root.as_ref())?
                {
                    return Err(AppError::new(
                        "symlink_not_allowed",
                        "Symbolic links and reparse points inside the workspace are not followed",
                    ));
                }
            }
        }
        Ok(resolved)
    }

    pub fn relative(&self, path: &Path) -> String {
        let relative = path
            .strip_prefix(self.root.as_ref())
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        if cfg!(windows) {
            relative.replace('\\', "/")
        } else {
            relative
        }
    }
}

/// Compare canonical path components using the platform's case semantics.
pub fn path_is_within(path: &Path, root: &Path) -> Result<bool> {
    if path.starts_with(root) {
        return Ok(true);
    }
    #[cfg(not(windows))]
    {
        Ok(false)
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Globalization::{CSTR_EQUAL, CompareStringOrdinal};
        let mut parts = path.components();
        let same_spelling = root.components().all(|expected| {
            let Some(actual) = parts.next() else {
                return false;
            };
            let a: Vec<u16> = actual.as_os_str().encode_wide().collect();
            let b: Vec<u16> = expected.as_os_str().encode_wide().collect();
            // Canonical Windows paths contain at most 32767 UTF-16 code units.
            unsafe {
                CompareStringOrdinal(a.as_ptr(), a.len() as i32, b.as_ptr(), b.len() as i32, 1)
                    == CSTR_EQUAL
            }
        });
        if !same_spelling {
            return Ok(false);
        }
        // NTFS can enable case sensitivity per directory. Case folding alone
        // must not expand the root into a distinct, similarly named directory.
        let prefix: PathBuf = path.components().take(root.components().count()).collect();
        Ok(directory_identity(&prefix)? == directory_identity(root)?)
    }
}

#[cfg(windows)]
fn directory_identity(path: &Path) -> Result<(u32, u32, u32)> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES,
        GetFileInformationByHandle,
    };
    let file = fs::OpenOptions::new()
        .read(true)
        .access_mode(FILE_READ_ATTRIBUTES)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok((
        info.dwVolumeSerialNumber,
        info.nFileIndexHigh,
        info.nFileIndexLow,
    ))
}

pub fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
