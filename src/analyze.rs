use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::audio;
use crate::features;
use crate::library::{self, Source, SourceTrack, TrackLocation};
use crate::store::Store;

pub const ANALYSIS_SR: u32 = 22050;

#[derive(Debug, Clone)]
pub struct AnalyzeOptions {
    pub source: Source,
    pub db_path: PathBuf,
    pub limit: Option<usize>,
    pub skip_analyzed: bool,
    pub dry_run: bool,
    pub path_map: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProgressEvent {
    Parsed {
        /// `rekordbox | traktor | serato | directory`
        source: String,
        total_in_source: usize,
    },
    Resolved {
        ready: usize,
        no_location: usize,
        unparseable_uri: usize,
        missing_file: usize,
        missing_examples: Vec<String>,
        duplicate: usize,
        scan_errors: Vec<String>,
        already_analyzed: usize,
    },
    Track {
        done: usize,
        total: usize,
        name: String,
        artist: String,
    },
    Failed {
        name: String,
        error: String,
    },
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Dropped {
    pub no_location: usize,
    pub unparseable_uri: usize,
    pub missing_file: usize,
    pub missing_examples: Vec<String>,
    /// The same file listed more than once by the source.
    pub duplicate: usize,
    /// Folders a directory scan could not read.
    pub scan_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalyzeSummary {
    pub ok: usize,
    pub failed: usize,
    pub skipped_already_analyzed: usize,
    pub dropped: Dropped,
    pub elapsed_secs: f32,
    pub total_considered: usize,
    pub db_path: String,
}

pub fn run<F>(opts: AnalyzeOptions, on_progress: F) -> Result<AnalyzeSummary>
where
    F: Fn(ProgressEvent) + Send + Sync,
{
    let loaded = library::load(&opts.source).with_context(|| {
        format!("reading {} library at {}", opts.source.kind(), opts.source.path().display())
    })?;
    let mut tracks = loaded.tracks;
    on_progress(ProgressEvent::Parsed {
        source: opts.source.kind().to_string(),
        total_in_source: tracks.len(),
    });

    if let Some(n) = opts.limit {
        tracks.truncate(n);
    }

    let mut resolved: Vec<(SourceTrack, PathBuf)> = Vec::new();
    let mut dropped = Dropped {
        scan_errors: loaded.scan_errors,
        ..Dropped::default()
    };
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for t in tracks {
        let path = match &t.location {
            TrackLocation::Missing => {
                dropped.no_location += 1;
                continue;
            }
            TrackLocation::Unparseable(_) => {
                dropped.unparseable_uri += 1;
                continue;
            }
            TrackLocation::File(p) => apply_path_map(p.clone(), &opts.path_map),
        };
        if !path.exists() {
            dropped.missing_file += 1;
            if dropped.missing_examples.len() < 10 {
                dropped.missing_examples.push(path.display().to_string());
            }
        } else if !seen.insert(path.clone()) {
            dropped.duplicate += 1;
        } else {
            resolved.push((t, path));
        }
    }

    let store = Store::open(&opts.db_path)
        .with_context(|| format!("opening db {}", opts.db_path.display()))?;

    let skipped = if opts.skip_analyzed {
        let before = resolved.len();
        let mut keep = Vec::with_capacity(before);
        for (t, p) in resolved {
            if !store.already_analyzed(t.native_id.as_deref(), &p.to_string_lossy())? {
                keep.push((t, p));
            }
        }
        resolved = keep;
        before - resolved.len()
    } else {
        0
    };
    let store = Mutex::new(store);

    on_progress(ProgressEvent::Resolved {
        ready: resolved.len(),
        no_location: dropped.no_location,
        unparseable_uri: dropped.unparseable_uri,
        missing_file: dropped.missing_file,
        missing_examples: dropped.missing_examples.clone(),
        duplicate: dropped.duplicate,
        scan_errors: dropped.scan_errors.clone(),
        already_analyzed: skipped,
    });

    let total = resolved.len();
    if opts.dry_run || total == 0 {
        return Ok(AnalyzeSummary {
            ok: 0,
            failed: 0,
            skipped_already_analyzed: skipped,
            dropped,
            elapsed_secs: 0.0,
            total_considered: total,
            db_path: opts.db_path.display().to_string(),
        });
    }

    let start = Instant::now();
    let done = AtomicUsize::new(0);
    let ok = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    // Cap UI updates: for a 5000-track run we emit ~200 Track events, not 5000.
    // The first few and the last one always emit so the bar starts moving
    // immediately and lands exactly on 100%.
    let emit_every = (total / 200).max(1);

    resolved.par_iter().for_each(|(source_track, path)| {
        let mut track = source_track.clone();
        let res = (|| -> Result<()> {
            let audio = audio::decode_mono(path, ANALYSIS_SR)
                .with_context(|| format!("decoding {}", path.display()))?;
            track.fill_missing(&audio.tags, path);
            let feats = features::extract(&audio.samples, audio.sample_rate);
            let path_str = path.to_string_lossy().to_string();
            let store = store.lock().unwrap();
            let id = store.resolve_track_id(track.native_id.as_deref(), &path_str)?;
            store.upsert(&id, &track, &feats, &path_str)?;
            Ok(())
        })();
        let display_name = if track.name.trim().is_empty() {
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            track.name.clone()
        };
        let n = done.fetch_add(1, Ordering::SeqCst) + 1;
        match res {
            Ok(()) => {
                ok.fetch_add(1, Ordering::SeqCst);
            }
            Err(e) => {
                failed.fetch_add(1, Ordering::SeqCst);
                on_progress(ProgressEvent::Failed {
                    name: display_name.clone(),
                    error: format!("{:#}", e),
                });
            }
        }
        let emit = n <= 3 || n == total || n % emit_every == 0;
        if emit {
            on_progress(ProgressEvent::Track {
                done: n,
                total,
                name: display_name,
                artist: track.artist.clone(),
            });
        }
    });

    let elapsed = start.elapsed().as_secs_f32();
    Ok(AnalyzeSummary {
        ok: ok.load(Ordering::SeqCst),
        failed: failed.load(Ordering::SeqCst),
        skipped_already_analyzed: skipped,
        dropped,
        elapsed_secs: elapsed,
        total_considered: total,
        db_path: opts.db_path.display().to_string(),
    })
}

/// Rewrite the first matching `OLD` prefix to `NEW`.
pub fn apply_path_map(path: PathBuf, path_map: &[(String, String)]) -> PathBuf {
    if path_map.is_empty() {
        return path;
    }
    let s = path.to_string_lossy();
    for (old, new) in path_map {
        if let Some(rest) = s.strip_prefix(old.as_str()) {
            return PathBuf::from(format!("{}{}", new, rest));
        }
    }
    path
}
