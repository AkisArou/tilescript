use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(target_family = "unix")]
mod unix {
    use super::*;
    use inotify::{Inotify, WatchMask};
    use std::os::fd::{AsRawFd, RawFd};

    #[derive(Debug)]
    pub(crate) struct SourceWatcher {
        inotify: Inotify,
        watched_roots: BTreeSet<PathBuf>,
    }

    impl SourceWatcher {
        pub(crate) fn new(watched_files: &BTreeSet<PathBuf>) -> std::io::Result<Self> {
            let mut watcher = Self {
                inotify: Inotify::init()?,
                watched_roots: watched_root_directories(watched_files),
            };
            watcher.register_roots()?;
            Ok(watcher)
        }

        pub(crate) fn signal_fd(&self) -> RawFd {
            self.inotify.as_raw_fd()
        }

        pub(crate) fn drain(&mut self) -> std::io::Result<bool> {
            let mut buffer = [0u8; 8192];
            let mut had_event = false;
            loop {
                match self.inotify.read_events(&mut buffer) {
                    Ok(mut events) => {
                        if events.next().is_none() {
                            return Ok(had_event);
                        }
                        had_event = true;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        return Ok(had_event);
                    }
                    Err(error) => return Err(error),
                }
            }
        }

        fn register_roots(&mut self) -> std::io::Result<()> {
            for root in &self.watched_roots {
                self.inotify.watches().add(
                    root,
                    WatchMask::CREATE
                        | WatchMask::MODIFY
                        | WatchMask::ATTRIB
                        | WatchMask::MOVED_FROM
                        | WatchMask::MOVED_TO
                        | WatchMask::DELETE
                        | WatchMask::DELETE_SELF
                        | WatchMask::MOVE_SELF
                        | WatchMask::CLOSE_WRITE,
                )?;
            }
            Ok(())
        }
    }
}

#[cfg(target_family = "unix")]
pub(crate) use unix::SourceWatcher;

#[cfg(not(target_family = "unix"))]
pub(crate) struct SourceWatcher;

#[cfg(not(target_family = "unix"))]
impl SourceWatcher {
    pub(crate) fn new(_watched_files: &BTreeSet<PathBuf>) -> std::io::Result<Self> {
        Ok(Self)
    }

    pub(crate) fn signal_fd(&self) -> i32 {
        -1
    }

    pub(crate) fn drain(&mut self) -> std::io::Result<bool> {
        Ok(false)
    }
}

pub(crate) fn watched_root_directories(watched_files: &BTreeSet<PathBuf>) -> BTreeSet<PathBuf> {
    let mut roots = BTreeSet::new();
    for path in watched_files {
        if let Some(parent) = path.parent() {
            roots.insert(parent.to_path_buf());
            collect_watch_directories(parent, &mut roots);
        }
    }
    roots
}

fn collect_watch_directories(root: &Path, watched: &mut BTreeSet<PathBuf>) {
    if !watched.insert(root.to_path_buf()) {
        return;
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ".hypreact-build" || name == ".sdk" {
                continue;
            }
            collect_watch_directories(&path, watched);
        }
    }
}
