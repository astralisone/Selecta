//! Serato library files: `database V2` and `.crate` files under `_Serato_`.
//!
//! Both are a flat stream of tag–length–value records: a 4-byte ASCII tag,
//! a big-endian u32 length, then the value. Tags starting with `o` hold a
//! nested record stream; `t` (text) and `p` (path) values are UTF-16BE.
//!
//! Track paths are stored relative to the root of the volume that holds the
//! `_Serato_` folder (`Users/me/Music/a.mp3` for `~/Music/_Serato_`), which
//! is why a crate can only reference files on its own drive.

use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::library::{boot_volume_name, SourceTrack, TrackLocation, VolumePath};
use crate::store::TrackRow;

pub const DATABASE_FILE: &str = "database V2";
const CRATE_VERSION: &str = "1.0/Serato ScratchLive Crate";
/// Columns shown when Serato opens an exported crate.
const CRATE_COLUMNS: &[&str] = &["song", "artist", "bpm", "key", "genre"];

type Record<'a> = ([u8; 4], &'a [u8]);

fn parse_records(data: &[u8]) -> Result<Vec<Record<'_>>> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < data.len() {
        if data.len() - pos < 8 {
            bail!("truncated record header at byte {}", pos);
        }
        let tag: [u8; 4] = data[pos..pos + 4].try_into().unwrap();
        let len = u32::from_be_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let start = pos + 8;
        let end = start
            .checked_add(len)
            .filter(|e| *e <= data.len())
            .ok_or_else(|| {
                anyhow!(
                    "record `{}` at byte {} claims {} bytes but only {} remain",
                    String::from_utf8_lossy(&tag),
                    pos,
                    len,
                    data.len() - start
                )
            })?;
        out.push((tag, &data[start..end]));
        pos = end;
    }
    Ok(out)
}

fn utf16be(data: &[u8]) -> String {
    let units: Vec<u16> = data
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
        .trim_end_matches('\0')
        .to_string()
}

fn push_record(buf: &mut Vec<u8>, tag: &[u8; 4], value: &[u8]) {
    buf.extend_from_slice(tag);
    buf.extend_from_slice(&(value.len() as u32).to_be_bytes());
    buf.extend_from_slice(value);
}

fn encode_utf16be(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect()
}

/// Metadata for one `otrk` record in `database V2`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DbTrack {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub genre: String,
    pub label: String,
    pub bpm: String,
    pub key: String,
}

pub fn parse_database(data: &[u8]) -> Result<Vec<DbTrack>> {
    let mut tracks = Vec::new();
    for (tag, value) in parse_records(data).context("parsing Serato database")? {
        if &tag != b"otrk" {
            continue;
        }
        let mut t = DbTrack::default();
        for (field, v) in parse_records(value).context("parsing Serato database track")? {
            match &field {
                b"pfil" => t.path = utf16be(v),
                b"tsng" => t.title = utf16be(v),
                b"tart" => t.artist = utf16be(v),
                b"tgen" => t.genre = utf16be(v),
                b"tlbl" => t.label = utf16be(v),
                b"tbpm" => t.bpm = utf16be(v),
                b"tkey" => t.key = utf16be(v),
                _ => {}
            }
        }
        if !t.path.is_empty() {
            tracks.push(t);
        }
    }
    Ok(tracks)
}

/// The `ptrk` paths in a crate, in crate order.
pub fn parse_crate(data: &[u8]) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    for (tag, value) in parse_records(data).context("parsing Serato crate")? {
        if &tag != b"otrk" {
            continue;
        }
        for (field, v) in parse_records(value).context("parsing Serato crate track")? {
            if &field == b"ptrk" {
                paths.push(utf16be(v));
            }
        }
    }
    Ok(paths)
}

pub fn render_crate(rel_paths: &[String]) -> Vec<u8> {
    let mut buf = Vec::new();
    push_record(&mut buf, b"vrsn", &encode_utf16be(CRATE_VERSION));
    let mut sort = Vec::new();
    push_record(&mut sort, b"tvcn", &encode_utf16be("song"));
    push_record(&mut sort, b"brev", &[0]);
    push_record(&mut buf, b"osrt", &sort);
    for col in CRATE_COLUMNS {
        let mut c = Vec::new();
        push_record(&mut c, b"tvcn", &encode_utf16be(col));
        push_record(&mut c, b"tvcw", &encode_utf16be("0"));
        push_record(&mut buf, b"ovct", &c);
    }
    for p in rel_paths {
        let mut t = Vec::new();
        push_record(&mut t, b"ptrk", &encode_utf16be(p));
        push_record(&mut buf, b"otrk", &t);
    }
    buf
}

/// The volume a `_Serato_` folder lives on; its tracks are relative to it.
fn serato_volume(serato_dir: &Path, boot: Option<&str>) -> Result<VolumePath> {
    let abs = std::path::absolute(serato_dir)
        .with_context(|| format!("resolving {}", serato_dir.display()))?;
    VolumePath::split(&abs.to_string_lossy(), boot)
        .ok_or_else(|| anyhow!("can't determine the volume of {}", abs.display()))
}

/// Turn a Serato-stored path into a filesystem path.
fn resolve_serato_path(stored: &str, volume: &VolumePath) -> PathBuf {
    let b = stored.as_bytes();
    if stored.starts_with('/') || (b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic()) {
        return PathBuf::from(stored);
    }
    volume.join_rel(stored)
}

/// Read a Serato library. `path` may be a `_Serato_` folder, its
/// `database V2` file, or a `.crate` file (only that crate's tracks, with
/// metadata from the neighbouring `database V2` when it exists).
pub fn read_library(path: &Path) -> Result<Vec<SourceTrack>> {
    read_library_with_boot(path, boot_volume_name().as_deref())
}

pub fn read_library_with_boot(path: &Path, boot: Option<&str>) -> Result<Vec<SourceTrack>> {
    let is_crate = path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("crate"))
        .unwrap_or(false);
    if is_crate {
        let serato_dir = path.parent().and_then(Path::parent).ok_or_else(|| {
            anyhow!(
                "{} is not inside a _Serato_/Subcrates folder",
                path.display()
            )
        })?;
        let volume = serato_volume(serato_dir, boot)?;
        let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let crate_paths = parse_crate(&data).with_context(|| format!("in {}", path.display()))?;
        let db_path = serato_dir.join(DATABASE_FILE);
        let meta: HashMap<String, DbTrack> = if db_path.is_file() {
            let db = std::fs::read(&db_path)
                .with_context(|| format!("reading {}", db_path.display()))?;
            parse_database(&db)
                .with_context(|| format!("in {}", db_path.display()))?
                .into_iter()
                .map(|t| (t.path.clone(), t))
                .collect()
        } else {
            HashMap::new()
        };
        return Ok(crate_paths
            .into_iter()
            .map(|p| {
                let db = meta.get(&p).cloned().unwrap_or_else(|| DbTrack {
                    path: p.clone(),
                    ..Default::default()
                });
                db_track_to_source(db, &volume)
            })
            .collect());
    }

    let (db_path, serato_dir) = if path.is_dir() {
        (path.join(DATABASE_FILE), path.to_path_buf())
    } else {
        let parent = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        (path.to_path_buf(), parent)
    };
    if !db_path.is_file() {
        bail!(
            "no Serato `{}` at {} — pick your _Serato_ folder (usually ~/Music/_Serato_)",
            DATABASE_FILE,
            db_path.display()
        );
    }
    let volume = serato_volume(&serato_dir, boot)?;
    let data = std::fs::read(&db_path).with_context(|| format!("reading {}", db_path.display()))?;
    Ok(parse_database(&data)
        .with_context(|| format!("in {}", db_path.display()))?
        .into_iter()
        .map(|t| db_track_to_source(t, &volume))
        .collect())
}

fn db_track_to_source(t: DbTrack, volume: &VolumePath) -> SourceTrack {
    let location = if t.path.trim().is_empty() {
        TrackLocation::Missing
    } else {
        TrackLocation::File(resolve_serato_path(&t.path, volume))
    };
    SourceTrack {
        native_id: None,
        name: t.title,
        artist: t.artist,
        genre: t.genre,
        label: t.label,
        bpm: t.bpm.trim().parse::<f32>().ok().filter(|b| *b > 0.0),
        tonality: t.key,
        rating: 0,
        play_count: 0,
        location,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrateWriteReport {
    pub written: usize,
    pub skipped_no_path: usize,
    /// Tracks on a different drive than the crate. Serato only lets a crate
    /// reference files on the drive its `_Serato_` folder lives on.
    pub skipped_other_volume: usize,
    pub other_volume_examples: Vec<String>,
}

/// Write `rows` as a Serato crate. `out` must be
/// `<volume>/…/_Serato_/Subcrates/<Crate name>.crate`; Serato shows it as a
/// crate named after the file the next time it starts.
pub fn write_crate(out: &Path, rows: &[TrackRow]) -> Result<CrateWriteReport> {
    write_crate_with_boot(out, rows, boot_volume_name().as_deref())
}

pub fn write_crate_with_boot(
    out: &Path,
    rows: &[TrackRow],
    boot: Option<&str>,
) -> Result<CrateWriteReport> {
    let in_subcrates = out
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n == "Subcrates")
        .unwrap_or(false);
    let serato_dir = out.parent().and_then(Path::parent);
    let in_serato = serato_dir
        .and_then(|p| p.file_name())
        .map(|n| n == "_Serato_")
        .unwrap_or(false);
    if !in_subcrates || !in_serato {
        bail!(
            "Serato crates must be saved inside a `_Serato_/Subcrates` folder \
             (e.g. ~/Music/_Serato_/Subcrates/My Set.crate); got {}",
            out.display()
        );
    }
    let volume = serato_volume(serato_dir.unwrap(), boot)?;

    let mut report = CrateWriteReport::default();
    let mut rel_paths = Vec::new();
    for r in rows {
        let Some(path) = r.file_path.as_deref() else {
            report.skipped_no_path += 1;
            continue;
        };
        match VolumePath::split(path, boot) {
            Some(vp) if vp.same_volume(&volume) => rel_paths.push(vp.rel().to_string()),
            _ => {
                report.skipped_other_volume += 1;
                if report.other_volume_examples.len() < 5 {
                    report.other_volume_examples.push(path.to_string());
                }
            }
        }
    }
    report.written = rel_paths.len();
    crate::export::ensure_parent_exists(out)?;
    std::fs::write(out, render_crate(&rel_paths))
        .with_context(|| format!("writing {}", out.display()))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(tag: &[u8; 4], s: &str) -> Vec<u8> {
        let mut b = Vec::new();
        push_record(&mut b, tag, &encode_utf16be(s));
        b
    }

    #[test]
    fn crate_round_trips() {
        let paths = vec![
            "Users/me/Music/a.mp3".to_string(),
            "Users/me/Music/Ünïcode ✓.flac".to_string(),
        ];
        let bytes = render_crate(&paths);
        assert_eq!(&bytes[..4], b"vrsn");
        assert_eq!(parse_crate(&bytes).unwrap(), paths);
    }

    #[test]
    fn parses_database_tracks_and_skips_unknown_fields() {
        let mut otrk = Vec::new();
        otrk.extend(text(b"ttyp", "mp3"));
        otrk.extend(text(b"pfil", "Users/me/Music/a.mp3"));
        otrk.extend(text(b"tsng", "Even If"));
        otrk.extend(text(b"tart", "Calibre"));
        otrk.extend(text(b"tgen", "Liquid"));
        otrk.extend(text(b"tbpm", "174.00"));
        otrk.extend(text(b"tkey", "Am"));
        push_record(&mut otrk, b"uadd", &1_700_000_000u32.to_be_bytes());
        push_record(&mut otrk, b"bmis", &[0]);
        let mut db = text(b"vrsn", "2.0/Serato Scratch LIVE Database");
        push_record(&mut db, b"otrk", &otrk);

        let tracks = parse_database(&db).unwrap();
        assert_eq!(
            tracks,
            vec![DbTrack {
                path: "Users/me/Music/a.mp3".into(),
                title: "Even If".into(),
                artist: "Calibre".into(),
                genre: "Liquid".into(),
                label: String::new(),
                bpm: "174.00".into(),
                key: "Am".into(),
            }]
        );
    }

    #[test]
    fn truncated_data_is_an_error() {
        let mut db = Vec::new();
        push_record(&mut db, b"otrk", &[1, 2, 3, 4]);
        db.truncate(db.len() - 1);
        assert!(parse_database(&db).is_err());
    }

    #[test]
    fn resolves_paths_against_the_serato_volume() {
        let root = VolumePath::Root { rel: String::new() };
        assert_eq!(
            resolve_serato_path("Users/me/a.mp3", &root),
            PathBuf::from("/Users/me/a.mp3")
        );
        let usb = VolumePath::Mounted {
            name: "USB".into(),
            rel: String::new(),
        };
        assert_eq!(
            resolve_serato_path("Music/a.mp3", &usb),
            PathBuf::from("/Volumes/USB/Music/a.mp3")
        );
        assert_eq!(
            resolve_serato_path("/abs/a.mp3", &usb),
            PathBuf::from("/abs/a.mp3")
        );
    }

    fn row(path: Option<&str>) -> TrackRow {
        TrackRow {
            track_id: path.unwrap_or("none").into(),
            name: "n".into(),
            artist: "a".into(),
            genre: String::new(),
            sub_genre: None,
            bpm: None,
            tonality: String::new(),
            energy: None,
            file_path: path.map(str::to_string),
        }
    }

    #[test]
    fn writes_crate_and_reads_it_back_with_database_metadata() {
        let base = std::env::temp_dir().join(format!("t2m-serato-{}", std::process::id()));
        let subcrates = base.join("_Serato_/Subcrates");
        std::fs::create_dir_all(&subcrates).unwrap();
        let track = base.join("music/a.mp3");
        let track_str = track.to_string_lossy().to_string();
        let rel = VolumePath::split(&track_str, None)
            .unwrap()
            .rel()
            .to_string();

        let mut otrk = text(b"pfil", &rel);
        otrk.extend(text(b"tsng", "Title From Serato"));
        otrk.extend(text(b"tbpm", "128"));
        let mut db = text(b"vrsn", "2.0/Serato Scratch LIVE Database");
        push_record(&mut db, b"otrk", &otrk);
        std::fs::write(base.join("_Serato_").join(DATABASE_FILE), db).unwrap();

        let out = subcrates.join("My Set.crate");
        let report = write_crate_with_boot(
            &out,
            &[
                row(Some(&track_str)),
                row(None),
                row(Some("/Volumes/Other/x.mp3")),
            ],
            Some("Macintosh HD"),
        )
        .unwrap();
        assert_eq!(report.written, 1);
        assert_eq!(report.skipped_no_path, 1);
        assert_eq!(report.skipped_other_volume, 1);
        assert_eq!(
            report.other_volume_examples,
            vec!["/Volumes/Other/x.mp3".to_string()]
        );

        let tracks = read_library_with_boot(&out, Some("Macintosh HD")).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].name, "Title From Serato");
        assert_eq!(tracks[0].bpm, Some(128.0));
        assert_eq!(tracks[0].location, TrackLocation::File(track.clone()));

        let all = read_library_with_boot(&base.join("_Serato_"), Some("Macintosh HD")).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].location, TrackLocation::File(track));

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn refuses_crates_outside_subcrates() {
        let out = std::env::temp_dir().join("t2m-not-serato.crate");
        let err = write_crate_with_boot(&out, &[row(Some("/a.mp3"))], None).unwrap_err();
        assert!(err.to_string().contains("_Serato_/Subcrates"));
    }
}
