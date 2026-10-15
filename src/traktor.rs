//! Traktor `.nml` collections: read for import, write for export.
//!
//! Traktor addresses a file as VOLUME + DIR + FILE, where DIR uses `/:` as its
//! separator (`/:Users/:me/:Music/:`). On macOS VOLUME is the volume's name
//! ("Macintosh HD" for the boot disk); on Windows it is the drive (`C:`).

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::keys::Camelot;
use crate::library::{boot_volume_name, SourceTrack, TrackLocation, VolumePath};
use crate::store::TrackRow;

/// Used as the boot volume name when exporting root-filesystem paths on a
/// machine where it can't be detected (i.e. not macOS). It is the macOS
/// default; Traktor resolves files by volume name, so a renamed boot disk
/// needs `--traktor-volume`.
pub const DEFAULT_BOOT_VOLUME: &str = "Macintosh HD";

/// Traktor's `MUSICAL_KEY VALUE` (0–23) in Camelot order: 0–11 are the major
/// keys C, C#, D … B; 12–23 the minor keys Cm, C#m, Dm … Bm.
#[rustfmt::skip]
const TRAKTOR_KEYS: [(u8, bool); 24] = [
    (8, true), (3, true), (10, true), (5, true), (12, true), (7, true),
    (2, true), (9, true), (4, true), (11, true), (6, true), (1, true),
    (5, false), (12, false), (7, false), (2, false), (9, false), (4, false),
    (11, false), (6, false), (1, false), (8, false), (3, false), (10, false),
];

pub fn camelot_from_traktor(value: u8) -> Option<Camelot> {
    TRAKTOR_KEYS
        .get(value as usize)
        .map(|&(hour, major)| Camelot { hour, major })
}

pub fn traktor_from_camelot(c: &Camelot) -> u8 {
    TRAKTOR_KEYS
        .iter()
        .position(|&(h, m)| h == c.hour && m == c.major)
        .expect("every Camelot position has a Traktor key") as u8
}

#[derive(Debug, Deserialize)]
#[serde(rename = "NML")]
struct Nml {
    #[serde(rename = "COLLECTION", default)]
    collection: Option<NmlCollection>,
}

#[derive(Debug, Deserialize)]
struct NmlCollection {
    #[serde(rename = "ENTRY", default)]
    entries: Vec<NmlEntry>,
}

#[derive(Debug, Deserialize)]
struct NmlEntry {
    #[serde(rename = "@TITLE", default)]
    title: String,
    #[serde(rename = "@ARTIST", default)]
    artist: String,
    #[serde(rename = "LOCATION", default)]
    location: Option<NmlLocation>,
    #[serde(rename = "INFO", default)]
    info: Option<NmlInfo>,
    #[serde(rename = "TEMPO", default)]
    tempo: Option<NmlTempo>,
    #[serde(rename = "MUSICAL_KEY", default)]
    musical_key: Option<NmlMusicalKey>,
}

#[derive(Debug, Deserialize)]
struct NmlLocation {
    #[serde(rename = "@DIR", default)]
    dir: String,
    #[serde(rename = "@FILE", default)]
    file: String,
    #[serde(rename = "@VOLUME", default)]
    volume: String,
}

#[derive(Debug, Deserialize)]
struct NmlInfo {
    #[serde(rename = "@GENRE", default)]
    genre: String,
    #[serde(rename = "@LABEL", default)]
    label: String,
    #[serde(rename = "@KEY", default)]
    key: String,
    #[serde(rename = "@RANKING", default)]
    ranking: String,
    #[serde(rename = "@PLAYCOUNT", default)]
    play_count: String,
}

#[derive(Debug, Deserialize)]
struct NmlTempo {
    #[serde(rename = "@BPM", default)]
    bpm: String,
}

#[derive(Debug, Deserialize)]
struct NmlMusicalKey {
    #[serde(rename = "@VALUE", default)]
    value: String,
}

/// Read every track in a Traktor collection (or exported playlist) `.nml`.
pub fn read_collection(path: &Path) -> Result<Vec<SourceTrack>> {
    let xml =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_collection(&xml, boot_volume_name().as_deref())
}

pub fn parse_collection(xml: &str, boot_volume: Option<&str>) -> Result<Vec<SourceTrack>> {
    let nml: Nml = quick_xml::de::from_str(xml).context("parsing Traktor NML")?;
    let entries = nml
        .collection
        .ok_or_else(|| anyhow!("NML has no <COLLECTION> — is this a Traktor collection file?"))?
        .entries;
    Ok(entries
        .into_iter()
        .map(|e| entry_to_track(e, boot_volume))
        .collect())
}

fn entry_to_track(e: NmlEntry, boot_volume: Option<&str>) -> SourceTrack {
    let location = match &e.location {
        None => TrackLocation::Missing,
        Some(l) if l.file.trim().is_empty() => TrackLocation::Missing,
        Some(l) => match nml_location_to_path(&l.volume, &l.dir, &l.file, boot_volume) {
            Some(p) => TrackLocation::File(p),
            None => TrackLocation::Unparseable(format!("{}{}{}", l.volume, l.dir, l.file)),
        },
    };
    let info = e.info.as_ref();
    // Prefer Traktor's numeric key; its KEY text depends on the user's
    // display setting and may be empty.
    let tonality = e
        .musical_key
        .as_ref()
        .and_then(|k| k.value.trim().parse::<u8>().ok())
        .and_then(camelot_from_traktor)
        .map(|c| c.to_string())
        .or_else(|| {
            info.map(|i| i.key.trim().to_string())
                .filter(|k| !k.is_empty())
        })
        .unwrap_or_default();
    SourceTrack {
        native_id: None,
        name: e.title,
        artist: e.artist,
        genre: info.map(|i| i.genre.clone()).unwrap_or_default(),
        label: info.map(|i| i.label.clone()).unwrap_or_default(),
        bpm: e
            .tempo
            .as_ref()
            .and_then(|t| t.bpm.trim().parse::<f32>().ok())
            .filter(|b| *b > 0.0),
        tonality,
        rating: info.map(|i| ranking_to_stars(&i.ranking)).unwrap_or(0),
        play_count: info
            .and_then(|i| i.play_count.trim().parse().ok())
            .unwrap_or(0),
        location,
    }
}

/// Traktor RANKING uses the same 0/51/102/153/204/255 scale as Rekordbox.
fn ranking_to_stars(raw: &str) -> u8 {
    match raw.trim().parse::<u32>().unwrap_or(0) {
        0 => 0,
        1..=51 => 1,
        52..=102 => 2,
        103..=153 => 3,
        154..=204 => 4,
        _ => 5,
    }
}

fn nml_location_to_path(
    volume: &str,
    dir: &str,
    file: &str,
    boot_volume: Option<&str>,
) -> Option<PathBuf> {
    let mut rel: Vec<&str> = dir.split("/:").filter(|s| !s.is_empty()).collect();
    rel.push(file);
    let rel = rel.join("/");
    let volume = volume.trim();
    let vb = volume.as_bytes();
    if vb.len() == 2 && vb[1] == b':' && vb[0].is_ascii_alphabetic() {
        let v = VolumePath::Drive {
            drive: volume.to_ascii_uppercase(),
            rel: String::new(),
        };
        return Some(v.join_rel(&rel));
    }
    let root = VolumePath::Root { rel: String::new() }.join_rel(&rel);
    if volume.is_empty() || boot_volume == Some(volume) {
        return Some(root);
    }
    let mounted = VolumePath::Mounted {
        name: volume.to_string(),
        rel: String::new(),
    }
    .join_rel(&rel);
    // The NML may come from a Mac whose boot disk has a different name than
    // this one, with the files at the same place on our root filesystem.
    if !mounted.exists() && root.exists() {
        return Some(root);
    }
    Some(mounted)
}

/// Traktor's (VOLUME, DIR, FILE) for an absolute path.
pub fn path_to_nml_location(path: &str, boot_volume: &str) -> Option<(String, String, String)> {
    let vp = VolumePath::split(path, Some(boot_volume))?;
    let volume = match &vp {
        VolumePath::Root { .. } => boot_volume.to_string(),
        VolumePath::Mounted { name, .. } => name.clone(),
        VolumePath::Drive { drive, .. } => drive.clone(),
    };
    let mut parts: Vec<&str> = vp.rel().split('/').filter(|s| !s.is_empty()).collect();
    let file = parts.pop()?.to_string();
    let mut dir = String::from("/:");
    for p in parts {
        dir.push_str(p);
        dir.push_str("/:");
    }
    Some((volume, dir, file))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NmlWriteReport {
    pub written: usize,
    pub skipped_no_path: usize,
}

/// Write `rows` as a Traktor collection containing one playlist named
/// `playlist_name`. In Traktor: right-click Playlists → Import Playlist.
/// BPM, key, genre and label travel with each entry.
pub fn write_nml(
    out: &Path,
    playlist_name: &str,
    rows: &[TrackRow],
    boot_volume: Option<&str>,
) -> Result<NmlWriteReport> {
    let detected = boot_volume_name();
    let boot = boot_volume
        .map(str::to_string)
        .or(detected)
        .unwrap_or_else(|| DEFAULT_BOOT_VOLUME.to_string());
    let (body, report) = render_nml(playlist_name, rows, &boot);
    crate::export::ensure_parent_exists(out)?;
    std::fs::write(out, body).with_context(|| format!("writing {}", out.display()))?;
    Ok(report)
}

pub fn render_nml(
    playlist_name: &str,
    rows: &[TrackRow],
    boot_volume: &str,
) -> (String, NmlWriteReport) {
    let esc = |s: &str| quick_xml::escape::escape(&s.replace(['\r', '\n'], " ")).into_owned();
    let mut entries = String::new();
    let mut keys = Vec::new();
    let mut skipped = 0usize;
    for r in rows {
        let Some((volume, dir, file)) = r
            .file_path
            .as_deref()
            .and_then(|p| path_to_nml_location(p, boot_volume))
        else {
            skipped += 1;
            continue;
        };
        let camelot = Camelot::parse(&r.tonality);
        let _ = write!(
            entries,
            "<ENTRY TITLE=\"{}\" ARTIST=\"{}\"><LOCATION DIR=\"{}\" FILE=\"{}\" VOLUME=\"{}\"></LOCATION>",
            esc(&r.name),
            esc(&r.artist),
            esc(&dir),
            esc(&file),
            esc(&volume)
        );
        let key_text = camelot
            .map(|c| c.to_string())
            .unwrap_or_else(|| r.tonality.trim().to_string());
        let _ = write!(
            entries,
            "<INFO GENRE=\"{}\" KEY=\"{}\"></INFO>",
            esc(&r.genre),
            esc(&key_text)
        );
        if let Some(bpm) = r.bpm.filter(|b| *b > 0.0) {
            let _ = write!(
                entries,
                "<TEMPO BPM=\"{:.6}\" BPM_QUALITY=\"100.000000\"></TEMPO>",
                bpm
            );
        }
        if let Some(c) = camelot {
            let _ = write!(
                entries,
                "<MUSICAL_KEY VALUE=\"{}\"></MUSICAL_KEY>",
                traktor_from_camelot(&c)
            );
        }
        entries.push_str("</ENTRY>\n");
        keys.push(format!("{}{}{}", volume, dir, file));
    }

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\" ?>\n");
    out.push_str("<NML VERSION=\"19\"><HEAD COMPANY=\"www.native-instruments.com\" PROGRAM=\"Traktor\"></HEAD>\n");
    out.push_str("<MUSICFOLDERS></MUSICFOLDERS>\n");
    let _ = writeln!(out, "<COLLECTION ENTRIES=\"{}\">", keys.len());
    out.push_str(&entries);
    out.push_str("</COLLECTION>\n<SETS ENTRIES=\"0\"></SETS>\n");
    let _ = writeln!(
        out,
        "<PLAYLISTS><NODE TYPE=\"FOLDER\" NAME=\"$ROOT\"><SUBNODES COUNT=\"1\">\n\
         <NODE TYPE=\"PLAYLIST\" NAME=\"{}\"><PLAYLIST ENTRIES=\"{}\" TYPE=\"LIST\" UUID=\"{}\">",
        esc(playlist_name),
        keys.len(),
        playlist_uuid(playlist_name, &keys)
    );
    for k in &keys {
        let _ = writeln!(
            out,
            "<ENTRY><PRIMARYKEY TYPE=\"TRACK\" KEY=\"{}\"></PRIMARYKEY></ENTRY>",
            esc(k)
        );
    }
    out.push_str("</PLAYLIST></NODE></SUBNODES></NODE></PLAYLISTS>\n</NML>\n");
    (
        out,
        NmlWriteReport {
            written: keys.len(),
            skipped_no_path: skipped,
        },
    )
}

/// Traktor wants a 32-hex-digit playlist UUID. It only has to be unique per
/// playlist, so derive it from the name, contents and current time.
fn playlist_uuid(name: &str, keys: &[String]) -> String {
    use std::hash::{Hash, Hasher};
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut a = std::collections::hash_map::DefaultHasher::new();
    (name, keys, now).hash(&mut a);
    let mut b = std::collections::hash_map::DefaultHasher::new();
    (now, keys, name, 1u8).hash(&mut b);
    format!("{:016x}{:016x}", a.finish(), b.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traktor_key_table_matches_camelot_parser() {
        let names = [
            "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B", "Cm", "C#m", "Dm",
            "Ebm", "Em", "Fm", "F#m", "Gm", "G#m", "Am", "Bbm", "Bm",
        ];
        for (v, n) in names.iter().enumerate() {
            let c = camelot_from_traktor(v as u8).unwrap();
            assert_eq!(Some(c), Camelot::parse(n), "value {} ({})", v, n);
            assert_eq!(traktor_from_camelot(&c), v as u8);
        }
        assert_eq!(camelot_from_traktor(24), None);
    }

    #[test]
    fn location_round_trips() {
        let (v, d, f) =
            path_to_nml_location("/Users/me/Music/DnB/a b.mp3", "Macintosh HD").unwrap();
        assert_eq!(
            (v.as_str(), d.as_str(), f.as_str()),
            ("Macintosh HD", "/:Users/:me/:Music/:DnB/:", "a b.mp3")
        );
        assert_eq!(
            nml_location_to_path(&v, &d, &f, Some("Macintosh HD")),
            Some(PathBuf::from("/Users/me/Music/DnB/a b.mp3"))
        );

        let (v, d, f) = path_to_nml_location("/Volumes/USB/x.flac", "Macintosh HD").unwrap();
        assert_eq!(
            (v.as_str(), d.as_str(), f.as_str()),
            ("USB", "/:", "x.flac")
        );

        let (v, d, f) = path_to_nml_location(r"C:\Music\x.wav", "Macintosh HD").unwrap();
        assert_eq!(
            (v.as_str(), d.as_str(), f.as_str()),
            ("C:", "/:Music/:", "x.wav")
        );
        assert_eq!(
            nml_location_to_path(&v, &d, &f, None),
            Some(PathBuf::from(r"C:\Music\x.wav"))
        );
    }

    #[test]
    fn parses_collection_entries() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
<NML VERSION="19"><HEAD COMPANY="www.native-instruments.com" PROGRAM="Traktor"></HEAD>
<COLLECTION ENTRIES="2">
<ENTRY MODIFIED_DATE="2024/1/1" TITLE="Even If" ARTIST="Calibre">
<LOCATION DIR="/:Users/:me/:Music/:" FILE="even if.flac" VOLUME="Macintosh HD" VOLUMEID="abc"></LOCATION>
<ALBUM TITLE="Shelflife"></ALBUM>
<INFO BITRATE="1411000" GENRE="Liquid" LABEL="Signature" KEY="8A" PLAYCOUNT="7" RANKING="204"></INFO>
<TEMPO BPM="174.000000" BPM_QUALITY="100.000000"></TEMPO>
<MUSICAL_KEY VALUE="21"></MUSICAL_KEY>
<CUE_V2 NAME="AutoGrid" TYPE="4" START="10.0"></CUE_V2>
<CUE_V2 NAME="n.n." TYPE="0" START="20.0"></CUE_V2>
</ENTRY>
<ENTRY TITLE="Stream"><INFO KEY="Am"></INFO></ENTRY>
</COLLECTION>
<PLAYLISTS><NODE TYPE="FOLDER" NAME="$ROOT"></NODE></PLAYLISTS>
</NML>"#;
        let tracks = parse_collection(xml, Some("Macintosh HD")).unwrap();
        assert_eq!(tracks.len(), 2);
        let t = &tracks[0];
        assert_eq!(t.name, "Even If");
        assert_eq!(t.artist, "Calibre");
        assert_eq!(t.genre, "Liquid");
        assert_eq!(t.label, "Signature");
        assert_eq!(t.bpm, Some(174.0));
        assert_eq!(t.tonality, "8A");
        assert_eq!(t.rating, 4);
        assert_eq!(t.play_count, 7);
        assert_eq!(
            t.location,
            TrackLocation::File(PathBuf::from("/Users/me/Music/even if.flac"))
        );
        assert_eq!(tracks[1].location, TrackLocation::Missing);
        assert_eq!(tracks[1].tonality, "Am");
    }

    #[test]
    fn rendered_nml_reads_back() {
        let rows = vec![
            TrackRow {
                track_id: "1".into(),
                name: "A & B <live>".into(),
                artist: "Art\"ist".into(),
                genre: "DnB".into(),
                sub_genre: None,
                bpm: Some(174.0),
                tonality: "Am".into(),
                energy: Some(0.5),
                file_path: Some("/Users/me/Music/a.mp3".into()),
            },
            TrackRow {
                track_id: "2".into(),
                name: "No file".into(),
                artist: String::new(),
                genre: String::new(),
                sub_genre: None,
                bpm: None,
                tonality: String::new(),
                energy: None,
                file_path: None,
            },
        ];
        let (xml, report) = render_nml("My \"Set\"", &rows, "Macintosh HD");
        assert_eq!(
            report,
            NmlWriteReport {
                written: 1,
                skipped_no_path: 1
            }
        );
        assert!(xml.contains("KEY=\"Macintosh HD/:Users/:me/:Music/:a.mp3\""));
        assert!(xml.contains("NAME=\"My &quot;Set&quot;\""));
        let back = parse_collection(&xml, Some("Macintosh HD")).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].name, "A & B <live>");
        assert_eq!(back[0].artist, "Art\"ist");
        assert_eq!(back[0].tonality, "8A");
        assert_eq!(back[0].bpm, Some(174.0));
        assert_eq!(
            back[0].location,
            TrackLocation::File(PathBuf::from("/Users/me/Music/a.mp3"))
        );
    }
}
