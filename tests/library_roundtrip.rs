//! End-to-end: scan a folder of real (synthesised) audio, then export the
//! result to Traktor and Serato and import those files back.
//!
//! The WAVs carry RIFF INFO tags, so this also checks that tags flow from
//! the decoder into the database, and that importing the same files from a
//! second source updates the existing rows instead of duplicating them.

use std::path::{Path, PathBuf};
use track2mix_core::analyze::{self, AnalyzeOptions};
use track2mix_core::export::{write_playlist, ExportFormat};
use track2mix_core::library::Source;
use track2mix_core::store::Store;

fn tmp_dir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("track2mix-it-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn info_entry(id: &[u8; 4], value: &str) -> Vec<u8> {
    let mut v = value.as_bytes().to_vec();
    v.push(0);
    if v.len() % 2 == 1 {
        v.push(0);
    }
    let mut out = id.to_vec();
    out.extend_from_slice(&((value.len() + 1) as u32).to_le_bytes());
    out.extend_from_slice(&v);
    out
}

/// Two seconds of a 22.05 kHz mono click track with optional INFO tags.
fn write_wav(path: &Path, title: Option<&str>, artist: Option<&str>, genre: Option<&str>) {
    let sr = 22050u32;
    let mut pcm = Vec::new();
    for i in 0..(sr * 2) {
        let t = i as f32 / sr as f32;
        let click = if i % (sr / 3) < 200 { 0.8 } else { 0.0 };
        let s = (t * 440.0 * std::f32::consts::TAU).sin() * 0.2 + click;
        pcm.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    let mut info = b"INFO".to_vec();
    if let Some(t) = title {
        info.extend(info_entry(b"INAM", t));
    }
    if let Some(a) = artist {
        info.extend(info_entry(b"IART", a));
    }
    if let Some(g) = genre {
        info.extend(info_entry(b"IGNR", g));
    }

    let mut body = b"WAVE".to_vec();
    body.extend_from_slice(b"fmt ");
    body.extend_from_slice(&16u32.to_le_bytes());
    body.extend_from_slice(&1u16.to_le_bytes()); // PCM
    body.extend_from_slice(&1u16.to_le_bytes()); // mono
    body.extend_from_slice(&sr.to_le_bytes());
    body.extend_from_slice(&(sr * 2).to_le_bytes());
    body.extend_from_slice(&2u16.to_le_bytes());
    body.extend_from_slice(&16u16.to_le_bytes());
    if info.len() > 4 {
        body.extend_from_slice(b"LIST");
        body.extend_from_slice(&(info.len() as u32).to_le_bytes());
        body.extend_from_slice(&info);
    }
    body.extend_from_slice(b"data");
    body.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    body.extend_from_slice(&pcm);

    let mut file = b"RIFF".to_vec();
    file.extend_from_slice(&(body.len() as u32).to_le_bytes());
    file.extend_from_slice(&body);
    std::fs::write(path, file).unwrap();
}

fn analyze(source: Source, db: &Path) -> analyze::AnalyzeSummary {
    analyze::run(
        AnalyzeOptions {
            source,
            db_path: db.to_path_buf(),
            limit: None,
            skip_analyzed: false,
            dry_run: false,
            path_map: Vec::new(),
        },
        |_| {},
    )
    .expect("analysis should succeed")
}

#[test]
fn scan_export_and_reimport() {
    let root = tmp_dir("roundtrip");
    let music = root.join("music/Liquid");
    std::fs::create_dir_all(&music).unwrap();
    write_wav(
        &music.join("one.wav"),
        Some("Tagged Title"),
        Some("Tagged Artist"),
        Some("Drum & Bass"),
    );
    write_wav(
        &music.join("Name Artist - Name Title.wav"),
        None,
        None,
        None,
    );
    std::fs::write(music.join("cover.jpg"), b"not audio").unwrap();
    let db = root.join("library.db");

    // 1. Folder scan: tags where present, file name otherwise.
    let summary = analyze(Source::Directory(root.join("music")), &db);
    assert_eq!((summary.ok, summary.failed), (2, 0));
    let store = Store::open(&db).unwrap();
    let mut rows = store.list_all().unwrap();
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (rows[0].name.as_str(), rows[0].artist.as_str()),
        ("Name Title", "Name Artist")
    );
    assert_eq!(
        (rows[1].name.as_str(), rows[1].artist.as_str()),
        ("Tagged Title", "Tagged Artist")
    );
    assert_eq!(rows[1].genre, "Drum & Bass");
    assert_eq!(rows[1].sub_genre.as_deref(), Some("Liquid"));
    assert!(rows
        .iter()
        .all(|r| r.track_id.starts_with("path:") && r.energy.is_some()));

    // 2. Traktor export → import into the same DB: same rows, no duplicates.
    let nml = root.join("set.nml");
    let report = write_playlist(&nml, "Set", &rows, Some("Macintosh HD")).unwrap();
    assert_eq!(report.format, ExportFormat::TraktorNml);
    assert_eq!(report.written, 2);
    let summary = analyze(Source::Traktor(nml), &db);
    assert_eq!((summary.ok, summary.failed), (2, 0));
    assert_eq!(store.list_all().unwrap().len(), 2);

    // 3. Serato crate export → import.
    let subcrates = root.join("_Serato_/Subcrates");
    std::fs::create_dir_all(&subcrates).unwrap();
    let crate_path = subcrates.join("Set.crate");
    let report = write_playlist(&crate_path, "ignored", &rows, None).unwrap();
    assert_eq!(report.format, ExportFormat::SeratoCrate);
    assert_eq!(report.written, 2);
    let summary = analyze(Source::Serato(crate_path), &db);
    assert_eq!((summary.ok, summary.failed), (2, 0));
    let after = store.list_all().unwrap();
    assert_eq!(after.len(), 2);
    // Names from the first import survive: the crate carries no metadata and
    // the tags/file name fill the same values back in.
    assert!(after.iter().any(|r| r.name == "Tagged Title"));

    // 4. skip_analyzed recognises files analyzed via another source.
    let skipped = analyze::run(
        AnalyzeOptions {
            source: Source::Directory(root.join("music")),
            db_path: db.clone(),
            limit: None,
            skip_analyzed: true,
            dry_run: false,
            path_map: Vec::new(),
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(skipped.skipped_already_analyzed, 2);
    assert_eq!(skipped.total_considered, 0);

    std::fs::remove_dir_all(&root).ok();
}
