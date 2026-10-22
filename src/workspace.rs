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
        if !resolved.starts_with(self.root.as_ref()) {
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
                if ancestor
                    .canonicalize()
                    .is_ok_and(|p| p.starts_with(self.root.as_ref()))
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
