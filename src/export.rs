use anyhow::{Context, Result};
use serde::Serialize;
use std::io::Write;
use std::path::Path;

use crate::store::TrackRow;
use crate::{serato, traktor};

/// Which DJ app a playlist file is for. Picked from the output file's
/// extension so the same `--export` flag / save dialog covers all three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    /// `.m3u8` / `.m3u` (and anything else) — Rekordbox, and most other apps.
    M3u8,
    /// `.nml` — Traktor.
    TraktorNml,
    /// `.crate` — Serato.
    SeratoCrate,
}

impl ExportFormat {
    pub fn from_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref()
        {
            Some("nml") => ExportFormat::TraktorNml,
            Some("crate") => ExportFormat::SeratoCrate,
            _ => ExportFormat::M3u8,
        }
    }

    pub fn app_name(self) -> &'static str {
        match self {
            ExportFormat::M3u8 => "Rekordbox (M3U8)",
            ExportFormat::TraktorNml => "Traktor (NML)",
            ExportFormat::SeratoCrate => "Serato (crate)",
        }
    }
}

/// What an export wrote, and what it had to leave out and why.
#[derive(Debug, Clone, Serialize)]
pub struct ExportReport {
    pub format: ExportFormat,
    pub written: usize,
    /// Rows with no local file (e.g. streaming entries).
    pub skipped_no_path: usize,
    /// Serato only: files on a different drive than the crate.
    pub skipped_other_volume: usize,
    pub other_volume_examples: Vec<String>,
}

/// Write `rows` to `out` in the format implied by its extension.
/// `playlist_name` is embedded for M3U8 and Traktor; a Serato crate is named
/// after its file. `traktor_volume` overrides the boot-volume name used in
/// Traktor locations (detected automatically on macOS).
pub fn write_playlist(
    out: &Path,
    playlist_name: &str,
    rows: &[TrackRow],
    traktor_volume: Option<&str>,
) -> Result<ExportReport> {
    let format = ExportFormat::from_path(out);
    let mut report = ExportReport {
        format,
        written: 0,
        skipped_no_path: 0,
        skipped_other_volume: 0,
        other_volume_examples: Vec::new(),
    };
    match format {
        ExportFormat::M3u8 => {
            report.written = write_m3u8(out, playlist_name, rows)?;
            report.skipped_no_path = rows.len() - report.written;
        }
        ExportFormat::TraktorNml => {
            let r = traktor::write_nml(out, playlist_name, rows, traktor_volume)?;
            report.written = r.written;
            report.skipped_no_path = r.skipped_no_path;
        }
        ExportFormat::SeratoCrate => {
            let r = serato::write_crate(out, rows)?;
            report.written = r.written;
            report.skipped_no_path = r.skipped_no_path;
            report.skipped_other_volume = r.skipped_other_volume;
            report.other_volume_examples = r.other_volume_examples;
        }
    }
    Ok(report)
}

pub(crate) fn ensure_parent_exists(out: &Path) -> Result<()> {
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(anyhow::anyhow!(
                "output directory does not exist: {}",
                parent.display()
            ));
        }
    }
    Ok(())
}

/// Map a raw energy feature to a 0..=10 integer, normalised across the
/// library's observed min/max. This lets the scale use its full range even
/// when raw values cluster in a narrow slice.
pub fn energy_score(raw: f32, lib_min: f32, lib_max: f32) -> u8 {
    let range = lib_max - lib_min;
    let normalised = if range > 1e-6 {
        (raw - lib_min) / range
    } else {
        raw
    };
    (normalised * 10.0).round().clamp(0.0, 10.0) as u8
}

/// Write an M3U8 playlist that Rekordbox (and other DJ apps) can import.
/// Skips rows with no resolvable file path; returns the count actually written.
pub fn write_m3u8(out: &Path, playlist_name: &str, rows: &[TrackRow]) -> Result<usize> {
    ensure_parent_exists(out)?;
    let mut file = std::fs::File::create(out)
        .with_context(|| format!("creating {}", out.display()))?;
    writeln!(file, "#EXTM3U")?;
    writeln!(file, "#PLAYLIST:{}", playlist_name)?;
    let mut written = 0usize;
    for r in rows {
        let Some(path) = r.file_path.as_deref() else {
            continue;
        };
        writeln!(
            file,
            "#EXTINF:-1,{} - {}",
            sanitize_extinf(&r.artist),
            sanitize_extinf(&r.name)
        )?;
        writeln!(file, "{}", path)?;
        written += 1;
    }
    Ok(written)
}

fn sanitize_extinf(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
}
