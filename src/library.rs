//! Source-neutral import model.
//!
//! Every library we can read — a Rekordbox XML export, a Traktor NML
//! collection, a Serato database or crate, or a plain folder of audio files —
//! is turned into a list of [`SourceTrack`]s. The analysis pipeline only ever
//! sees this type, so adding a new source never touches decoding or storage.

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::{rekordbox, serato, traktor};

/// Audio file extensions the decoder (Symphonia with `all` features) handles.
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "wave", "aif", "aiff", "m4a", "mp4", "aac", "alac", "ogg", "oga",
];

/// Where a library entry's audio lives, as far as the source could tell us.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackLocation {
    /// A filesystem path. It has not been checked for existence yet.
    File(PathBuf),
    /// The source entry carried no location at all.
    Missing,
    /// The source entry had a location we could not turn into a path
    /// (e.g. a streaming URI). The raw value is kept for reporting.
    Unparseable(String),
}

/// One library entry, independent of which app it came from.
#[derive(Debug, Clone)]
pub struct SourceTrack {
    /// The source's own stable ID, if it has one (Rekordbox `TrackID`).
    /// Sources without one are keyed by file path when stored.
    pub native_id: Option<String>,
    pub name: String,
    pub artist: String,
    pub genre: String,
    pub label: String,
    pub bpm: Option<f32>,
    pub tonality: String,
    /// 0–5 stars.
    pub rating: u8,
    pub play_count: i64,
    pub location: TrackLocation,
}

impl SourceTrack {
    pub fn from_path(path: PathBuf) -> Self {
        Self {
            native_id: None,
            name: String::new(),
            artist: String::new(),
            genre: String::new(),
            label: String::new(),
            bpm: None,
            tonality: String::new(),
            rating: 0,
            play_count: 0,
            location: TrackLocation::File(path),
        }
    }

    /// Fill any field the source left blank from the file's embedded tags,
    /// then from the file name (`Artist - Title.ext`). Source metadata always
    /// wins: a DJ's edits in their library app are more deliberate than tags.
    pub fn fill_missing(&mut self, tags: &FileTags, path: &Path) {
        fill(&mut self.name, &tags.title);
        fill(&mut self.artist, &tags.artist);
        fill(&mut self.genre, &tags.genre);
        fill(&mut self.label, &tags.label);
        fill(&mut self.tonality, &tags.key);
        if self.bpm.is_none_or(|b| b <= 0.0) {
            self.bpm = tags.bpm.filter(|b| *b > 0.0);
        }
        if self.name.trim().is_empty() || self.artist.trim().is_empty() {
            let (artist, title) = split_file_stem(path);
            if self.name.trim().is_empty() {
                self.name = title;
            }
            if self.artist.trim().is_empty() {
                if let Some(a) = artist {
                    self.artist = a;
                }
            }
        }
    }
}

fn fill(dst: &mut String, src: &Option<String>) {
    if dst.trim().is_empty() {
        if let Some(v) = src.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            *dst = v.to_string();
        }
    }
}

/// `Artist - Title.mp3` → (Some("Artist"), "Title"); anything else → (None, stem).
fn split_file_stem(path: &Path) -> (Option<String>, String) {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    match stem.split_once(" - ") {
        Some((a, t)) if !a.trim().is_empty() && !t.trim().is_empty() => {
            (Some(a.trim().to_string()), t.trim().to_string())
        }
        _ => (None, stem.trim().to_string()),
    }
}

/// Tags embedded in an audio file. Every field is optional.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub genre: Option<String>,
    pub label: Option<String>,
    pub key: Option<String>,
    pub bpm: Option<f32>,
}

/// Which library to import.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Rekordbox: File → Export Collection in xml format.
    Rekordbox(PathBuf),
    /// Traktor: the `collection.nml` file, or any exported `.nml` playlist.
    Traktor(PathBuf),
    /// Serato: a `_Serato_` folder, its `database V2` file, or a single `.crate`.
    Serato(PathBuf),
    /// Any folder; scanned recursively for audio files.
    Directory(PathBuf),
}

impl Source {
    pub fn path(&self) -> &Path {
        match self {
            Source::Rekordbox(p)
            | Source::Traktor(p)
            | Source::Serato(p)
            | Source::Directory(p) => p,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Source::Rekordbox(_) => "rekordbox",
            Source::Traktor(_) => "traktor",
            Source::Serato(_) => "serato",
            Source::Directory(_) => "directory",
        }
    }

    /// Build a source from a kind name (`rekordbox | traktor | serato | directory`).
    pub fn from_kind(kind: &str, path: PathBuf) -> Result<Self> {
        Ok(match kind {
            "rekordbox" => Source::Rekordbox(path),
            "traktor" => Source::Traktor(path),
            "serato" => Source::Serato(path),
            "directory" => Source::Directory(path),
            other => {
                return Err(anyhow!(
                    "unknown source kind `{}`. try: rekordbox | traktor | serato | directory",
                    other
                ))
            }
        })
    }
}

/// The result of reading a source: its tracks, plus anything that could not
/// be read at all (unreadable sub-folders during a scan).
#[derive(Debug, Clone, Default)]
pub struct Loaded {
    pub tracks: Vec<SourceTrack>,
    pub scan_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanError {
    pub path: String,
    pub error: String,
}

pub fn load(source: &Source) -> Result<Loaded> {
    match source {
        Source::Rekordbox(p) => Ok(Loaded {
            tracks: rekordbox::parse(p)?
                .collection
                .tracks
                .into_iter()
                .map(rekordbox::Track::into_source_track)
                .collect(),
            scan_errors: Vec::new(),
        }),
        Source::Traktor(p) => Ok(Loaded {
            tracks: traktor::read_collection(p)?,
            scan_errors: Vec::new(),
        }),
        Source::Serato(p) => Ok(Loaded {
            tracks: serato::read_library(p)?,
            scan_errors: Vec::new(),
        }),
        Source::Directory(p) => {
            let scan = scan_directory(p)?;
            Ok(Loaded {
                tracks: scan.files.into_iter().map(SourceTrack::from_path).collect(),
                scan_errors: scan
                    .errors
                    .into_iter()
                    .map(|e| format!("{}: {}", e.path, e.error))
                    .collect(),
            })
        }
    }
}

#[derive(Debug, Default)]
pub struct DirectoryScan {
    /// Audio files found, sorted by path.
    pub files: Vec<PathBuf>,
    /// Sub-folders that could not be read (permissions, broken mounts).
    pub errors: Vec<ScanError>,
}

/// Recursively collect audio files under `root`.
///
/// Hidden entries (dot-files, including macOS `._` AppleDouble companions
/// that look like audio but aren't) are skipped. Symlinked directories are
/// not followed, so a link loop can't hang the scan. A `root` that can't be
/// read is an error; an unreadable sub-folder is recorded and the scan
/// carries on so one bad folder doesn't sink a 20k-track library.
pub fn scan_directory(root: &Path) -> Result<DirectoryScan> {
    let meta = std::fs::metadata(root).with_context(|| format!("reading {}", root.display()))?;
    if !meta.is_dir() {
        return Err(anyhow!("not a directory: {}", root.display()));
    }
    let mut scan = DirectoryScan::default();
    let mut stack = vec![root.to_path_buf()];
    let mut first = true;
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if first => {
                return Err(anyhow!(e).context(format!("reading {}", dir.display())));
            }
            Err(e) => {
                scan.errors.push(ScanError {
                    path: dir.display().to_string(),
                    error: e.to_string(),
                });
                continue;
            }
        };
        first = false;
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    scan.errors.push(ScanError {
                        path: dir.display().to_string(),
                        error: e.to_string(),
                    });
                    continue;
                }
            };
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(e) => {
                    scan.errors.push(ScanError {
                        path: path.display().to_string(),
                        error: e.to_string(),
                    });
                    continue;
                }
            };
            if ft.is_dir() {
                stack.push(path);
            } else if (ft.is_file() || ft.is_symlink()) && is_audio_file(&path) {
                scan.files.push(path);
            }
        }
    }
    scan.files.sort();
    Ok(scan)
}

pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// The macOS boot volume's display name (e.g. "Macintosh HD"), found as the
/// entry in `/Volumes` that links back to `/`. `None` on other platforms.
pub fn boot_volume_name() -> Option<String> {
    let root = std::fs::canonicalize("/").ok()?;
    for entry in std::fs::read_dir("/Volumes").ok()?.flatten() {
        if std::fs::canonicalize(entry.path()).ok().as_deref() == Some(root.as_path()) {
            return Some(entry.file_name().to_string_lossy().into_owned());
        }
    }
    None
}

/// A path split into the volume it lives on and the part below that volume's
/// root. This is how both Traktor and Serato address files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VolumePath {
    /// The boot / root filesystem (`/…`). `rel` has no leading separator.
    Root { rel: String },
    /// A mounted macOS volume (`/Volumes/<name>/…`).
    Mounted { name: String, rel: String },
    /// A Windows drive (`C:\…`). `drive` is like `C:`.
    Drive { drive: String, rel: String },
}

impl VolumePath {
    /// Split an absolute path. `/Volumes/<boot>` is folded back into `Root`
    /// so the same file always maps to the same volume.
    pub fn split(path: &str, boot_volume: Option<&str>) -> Option<VolumePath> {
        let bytes = path.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
            let rest = path[2..].trim_start_matches(['\\', '/']);
            return Some(VolumePath::Drive {
                drive: path[..2].to_ascii_uppercase(),
                rel: rest.replace('\\', "/"),
            });
        }
        let rest = path.strip_prefix('/')?;
        if let Some(after) = rest.strip_prefix("Volumes/") {
            let (name, rel) = after.split_once('/').unwrap_or((after, ""));
            if !name.is_empty() {
                if boot_volume == Some(name) {
                    return Some(VolumePath::Root {
                        rel: rel.to_string(),
                    });
                }
                return Some(VolumePath::Mounted {
                    name: name.to_string(),
                    rel: rel.to_string(),
                });
            }
        }
        Some(VolumePath::Root {
            rel: rest.to_string(),
        })
    }

    pub fn rel(&self) -> &str {
        match self {
            VolumePath::Root { rel }
            | VolumePath::Mounted { rel, .. }
            | VolumePath::Drive { rel, .. } => rel,
        }
    }

    /// Same volume, ignoring the relative part.
    pub fn same_volume(&self, other: &VolumePath) -> bool {
        match (self, other) {
            (VolumePath::Root { .. }, VolumePath::Root { .. }) => true,
            (VolumePath::Mounted { name: a, .. }, VolumePath::Mounted { name: b, .. }) => a == b,
            (VolumePath::Drive { drive: a, .. }, VolumePath::Drive { drive: b, .. }) => {
                a.eq_ignore_ascii_case(b)
            }
            _ => false,
        }
    }

    /// Rebuild a filesystem path for a relative path on this volume.
    pub fn join_rel(&self, rel: &str) -> PathBuf {
        let rel = rel.trim_start_matches(['/', '\\']);
        match self {
            VolumePath::Root { .. } => PathBuf::from(format!("/{}", rel)),
            VolumePath::Mounted { name, .. } => PathBuf::from(format!("/Volumes/{}/{}", name, rel)),
            VolumePath::Drive { drive, .. } => {
                PathBuf::from(format!("{}\\{}", drive, rel.replace('/', "\\")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_volume_paths() {
        assert_eq!(
            VolumePath::split("/Users/me/a.mp3", Some("Macintosh HD")),
            Some(VolumePath::Root {
                rel: "Users/me/a.mp3".into()
            })
        );
        assert_eq!(
            VolumePath::split("/Volumes/Macintosh HD/Users/me/a.mp3", Some("Macintosh HD")),
            Some(VolumePath::Root {
                rel: "Users/me/a.mp3".into()
            })
        );
        assert_eq!(
            VolumePath::split("/Volumes/USB/Music/a.mp3", Some("Macintosh HD")),
            Some(VolumePath::Mounted {
                name: "USB".into(),
                rel: "Music/a.mp3".into()
            })
        );
        assert_eq!(
            VolumePath::split(r"d:\Music\a.mp3", None),
            Some(VolumePath::Drive {
                drive: "D:".into(),
                rel: "Music/a.mp3".into()
            })
        );
        assert_eq!(VolumePath::split("relative/a.mp3", None), None);
    }

    #[test]
    fn fill_missing_prefers_source_then_tags_then_filename() {
        let path = PathBuf::from("/m/Calibre - Even If.flac");
        let mut t = SourceTrack::from_path(path.clone());
        t.genre = "Liquid".into();
        let tags = FileTags {
            title: None,
            artist: None,
            genre: Some("Drum & Bass".into()),
            label: Some("Signature".into()),
            key: Some("8A".into()),
            bpm: Some(174.0),
        };
        t.fill_missing(&tags, &path);
        assert_eq!(t.name, "Even If");
        assert_eq!(t.artist, "Calibre");
        assert_eq!(t.genre, "Liquid");
        assert_eq!(t.label, "Signature");
        assert_eq!(t.tonality, "8A");
        assert_eq!(t.bpm, Some(174.0));
    }

    #[test]
    fn scan_skips_hidden_and_non_audio_and_recurses() {
        let root = std::env::temp_dir().join(format!("t2m-scan-{}", std::process::id()));
        std::fs::create_dir_all(root.join("sub/deeper")).unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();
        for f in [
            "a.mp3",
            "sub/b.FLAC",
            "sub/deeper/c.aiff",
            "sub/._b.FLAC",
            "notes.txt",
            ".hidden/d.mp3",
        ] {
            std::fs::write(root.join(f), b"x").unwrap();
        }
        let scan = scan_directory(&root).unwrap();
        let rel: Vec<String> = scan
            .files
            .iter()
            .map(|p| {
                p.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(rel, vec!["a.mp3", "sub/b.FLAC", "sub/deeper/c.aiff"]);
        assert!(scan.errors.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn scan_of_missing_root_is_an_error() {
        assert!(scan_directory(Path::new("/definitely/not/here")).is_err());
    }
}
