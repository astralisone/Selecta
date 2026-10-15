use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use track2mix_core::analyze::{self, AnalyzeOptions, ProgressEvent};
use track2mix_core::export::{energy_score, write_playlist, ExportReport};
use track2mix_core::keys::Camelot;
use track2mix_core::library::Source;
use track2mix_core::store::{Store, TrackRow};

#[derive(Parser)]
#[command(
    name = "track2mix",
    about = "Import a Rekordbox, Traktor or Serato library (or scan a folder), extract audio features, query the result and export playlists."
)]
struct Cli {
    /// Path to the SQLite database used by every subcommand.
    #[arg(long, default_value = "library.db", global = true)]
    db: PathBuf,

    /// Traktor volume name for files on the boot disk when exporting `.nml`.
    /// Detected automatically on macOS; defaults to "Macintosh HD" elsewhere.
    #[arg(long = "traktor-volume", global = true)]
    traktor_volume: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Import a library (Rekordbox, Traktor, Serato) or scan a folder, decode
    /// each track, and write features to the DB.
    Analyze {
        #[command(flatten)]
        source: SourceArgs,
        /// Limit number of tracks processed (useful for a quick sanity pass).
        #[arg(long)]
        limit: Option<usize>,
        /// Skip tracks already present and analyzed in the DB.
        #[arg(long)]
        skip_analyzed: bool,
        /// Report coverage (how many paths resolve) without decoding any audio.
        #[arg(long)]
        dry_run: bool,
        /// Rewrite a path prefix before resolving. Repeatable.
        /// Format: `OLD=NEW`. Example: `--path-map '/Volumes/Old/music=/Volumes/New/music'`.
        #[arg(long = "path-map", value_parser = parse_path_map)]
        path_map: Vec<(String, String)>,
    },
    /// Show top-N tracks sorted by a feature column.
    Top {
        /// Sort metric: energy | bpm | rms | centroid | flux | onsets | rating | plays
        #[arg(long, default_value = "energy")]
        by: String,
        /// Number of rows to show.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Case-insensitive substring filter on the Rekordbox Genre tag.
        #[arg(long)]
        genre: Option<String>,
        /// Case-insensitive substring filter on the parent-folder name on disk
        /// (e.g. "liquid", "neuro") — useful when the Genre tag is too coarse.
        #[arg(long = "sub-genre")]
        sub_genre: Option<String>,
        /// Sort ascending instead of descending.
        #[arg(long)]
        asc: bool,
        /// Also write the matching tracks to a playlist at this path. The
        /// extension picks the app: `.m3u8` Rekordbox, `.nml` Traktor,
        /// `.crate` Serato (save inside `_Serato_/Subcrates`).
        #[arg(long)]
        export: Option<PathBuf>,
        /// Playlist name embedded in the file. Defaults to the file stem.
        #[arg(long = "playlist-name")]
        playlist_name: Option<String>,
    },
    /// Suggest harmonically + BPM-compatible tracks around a reference track.
    Compat {
        /// Substring search against track name or artist; the top-ranked match is used as the anchor.
        query: String,
        /// BPM tolerance (± this value around the anchor's BPM).
        #[arg(long, default_value_t = 3.0)]
        bpm_tol: f32,
        /// Maximum number of suggestions to return.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Ignore Camelot compatibility (BPM-only matching).
        #[arg(long)]
        any_key: bool,
        /// Also match tracks at half and double the anchor's BPM
        /// (e.g. a 175 BPM DnB anchor matches 86–88 BPM half-time tagging of the same tempo).
        #[arg(long = "half-double-ok")]
        half_double_ok: bool,
        /// Also write the compatible tracks to a playlist at this path
        /// (`.m3u8` Rekordbox, `.nml` Traktor, `.crate` Serato).
        #[arg(long)]
        export: Option<PathBuf>,
        /// Playlist name embedded in the file. Defaults to the file stem.
        #[arg(long = "playlist-name")]
        playlist_name: Option<String>,
    },
    /// Export analyzed tracks as a playlist for Rekordbox, Traktor or Serato.
    Export {
        /// Output file. The extension picks the app: `.m3u8` Rekordbox,
        /// `.nml` Traktor, `.crate` Serato (save inside `_Serato_/Subcrates`).
        out: PathBuf,
        /// Case-insensitive substring filter on the Genre tag.
        #[arg(long)]
        genre: Option<String>,
        /// Case-insensitive substring filter on the parent-folder name on disk.
        #[arg(long = "sub-genre")]
        sub_genre: Option<String>,
        /// Playlist name embedded in the file. Defaults to the file stem.
        #[arg(long = "playlist-name")]
        playlist_name: Option<String>,
    },
    /// Summary of what's in the database.
    Stats,
}

/// Exactly one library source for `analyze`.
#[derive(Args)]
#[group(required = true, multiple = false)]
struct SourceArgs {
    /// Rekordbox XML export (File → Export Collection in xml format).
    #[arg(long)]
    xml: Option<PathBuf>,
    /// Traktor collection (`collection.nml`) or an exported `.nml` playlist.
    #[arg(long)]
    nml: Option<PathBuf>,
    /// Serato library: the `_Serato_` folder, its `database V2` file, or a `.crate`.
    #[arg(long)]
    serato: Option<PathBuf>,
    /// Folder to scan recursively for audio files (tags supply title/artist/BPM/key).
    #[arg(long)]
    dir: Option<PathBuf>,
}

impl SourceArgs {
    fn into_source(self) -> Source {
        match (self.xml, self.nml, self.serato, self.dir) {
            (Some(p), _, _, _) => Source::Rekordbox(p),
            (_, Some(p), _, _) => Source::Traktor(p),
            (_, _, Some(p), _) => Source::Serato(p),
            (_, _, _, Some(p)) => Source::Directory(p),
            (None, None, None, None) => unreachable!("clap enforces exactly one source"),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Analyze {
            source,
            limit,
            skip_analyzed,
            dry_run,
            path_map,
        } => cmd_analyze(
            &cli.db,
            source.into_source(),
            limit,
            skip_analyzed,
            dry_run,
            path_map,
        ),
        Command::Top {
            by,
            limit,
            genre,
            sub_genre,
            asc,
            export,
            playlist_name,
        } => cmd_top(
            &cli.db,
            &by,
            limit,
            genre.as_deref(),
            sub_genre.as_deref(),
            asc,
            export.as_deref(),
            playlist_name.as_deref(),
            cli.traktor_volume.as_deref(),
        ),
        Command::Compat {
            query,
            bpm_tol,
            limit,
            any_key,
            half_double_ok,
            export,
            playlist_name,
        } => cmd_compat(
            &cli.db,
            &query,
            bpm_tol,
            limit,
            any_key,
            half_double_ok,
            export.as_deref(),
            playlist_name.as_deref(),
            cli.traktor_volume.as_deref(),
        ),
        Command::Export {
            out,
            genre,
            sub_genre,
            playlist_name,
        } => cmd_export(
            &cli.db,
            &out,
            genre.as_deref(),
            sub_genre.as_deref(),
            playlist_name.as_deref(),
            cli.traktor_volume.as_deref(),
        ),
        Command::Stats => cmd_stats(&cli.db),
    }
}

fn parse_path_map(s: &str) -> Result<(String, String), String> {
    let (old, new) = s
        .split_once('=')
        .ok_or_else(|| format!("expected OLD=NEW, got `{}`", s))?;
    if old.is_empty() {
        return Err("OLD prefix must not be empty".into());
    }
    Ok((old.to_string(), new.to_string()))
}

fn cmd_analyze(
    db: &Path,
    source: Source,
    limit: Option<usize>,
    skip_analyzed: bool,
    dry_run: bool,
    path_map: Vec<(String, String)>,
) -> Result<()> {
    println!("Reading {} library at {}…", source.kind(), source.path().display());
    if let Some(n) = limit {
        println!("  limited to {}", n);
    }
    if !path_map.is_empty() {
        println!("  path remaps:");
        for (old, new) in &path_map {
            println!("    {} → {}", old, new);
        }
    }

    let pb = ProgressBar::hidden();
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner} [{elapsed_precise}] [{bar:40}] {pos}/{len} ({eta}) {msg}",
        )
        .unwrap()
        .progress_chars("=>-"),
    );
    let failures = Mutex::new(Vec::<(String, String)>::new());

    let summary = analyze::run(
        AnalyzeOptions {
            source,
            db_path: db.to_path_buf(),
            limit,
            skip_analyzed,
            dry_run,
            path_map,
        },
        |ev| match ev {
            ProgressEvent::Parsed { total_in_source, .. } => {
                println!("  found {} tracks", total_in_source);
            }
            ProgressEvent::Resolved {
                ready,
                no_location,
                unparseable_uri,
                missing_file,
                missing_examples,
                duplicate,
                scan_errors,
                already_analyzed,
            } => {
                println!("  {} resolved to an existing local file", ready + already_analyzed);
                if no_location + unparseable_uri + missing_file + duplicate + scan_errors.len() > 0 {
                    println!("  Dropped:");
                    if no_location > 0 {
                        println!("    {} with no file location", no_location);
                    }
                    if unparseable_uri > 0 {
                        println!("    {} with unparseable locations (streaming entries?)", unparseable_uri);
                    }
                    if missing_file > 0 {
                        println!(
                            "    {} pointing at files not on disk (external drive unmounted? moved library? try --path-map)",
                            missing_file
                        );
                        for ex in missing_examples.iter().take(3) {
                            println!("      e.g. {}", ex);
                        }
                    }
                    if duplicate > 0 {
                        println!("    {} listed more than once", duplicate);
                    }
                    if !scan_errors.is_empty() {
                        println!("    {} folder(s) could not be read:", scan_errors.len());
                        for e in scan_errors.iter().take(5) {
                            println!("      {}", e);
                        }
                    }
                }
                if already_analyzed > 0 {
                    println!("  {} skipped (already analyzed)", already_analyzed);
                }
                if dry_run {
                    println!("  dry-run: skipping audio decode. {} track(s) would be analyzed.", ready);
                } else if ready == 0 {
                    println!("  nothing to do.");
                } else {
                    println!("  analyzing {}…", ready);
                    pb.set_length(ready as u64);
                    pb.set_draw_target(indicatif::ProgressDrawTarget::stderr());
                }
            }
            ProgressEvent::Track { done, artist, name, .. } => {
                pb.set_position(done as u64);
                pb.set_message(format!("{} - {}", artist, name));
            }
            ProgressEvent::Failed { name, error } => {
                failures.lock().unwrap().push((name, error));
            }
        },
    )?;
    pb.finish();

    if dry_run || summary.total_considered == 0 {
        return Ok(());
    }
    println!();
    println!("Done in {:.1}s", summary.elapsed_secs);
    println!(
        "  {} succeeded, {} failed, {:.2}s/track avg",
        summary.ok,
        summary.failed,
        summary.elapsed_secs / summary.total_considered.max(1) as f32
    );
    let failures = failures.into_inner().unwrap();
    if !failures.is_empty() {
        println!("  first 5 failures:");
        for (name, err) in failures.iter().take(5) {
            println!("    {}: {}", name, err);
        }
    }
    Ok(())
}

fn print_export_report(out: &Path, name: &str, report: &ExportReport) {
    println!(
        "\nWrote {} track(s) to {} for {} (playlist: \"{}\")",
        report.written,
        out.display(),
        report.format.app_name(),
        name
    );
    if report.skipped_no_path > 0 {
        println!("  skipped {} with no local file", report.skipped_no_path);
    }
    if report.skipped_other_volume > 0 {
        println!(
            "  skipped {} on a different drive than the crate (Serato crates can only hold files on their own drive):",
            report.skipped_other_volume
        );
        for ex in &report.other_volume_examples {
            println!("    e.g. {}", ex);
        }
    }
}

fn cmd_top(
    db: &Path,
    metric: &str,
    limit: usize,
    genre: Option<&str>,
    sub_genre: Option<&str>,
    asc: bool,
    export: Option<&Path>,
    playlist_name: Option<&str>,
    traktor_volume: Option<&str>,
) -> Result<()> {
    let store = Store::open(db)?;
    let rows = store.top_by(metric, limit, genre, sub_genre, asc)?;
    if rows.is_empty() {
        println!("no matching tracks — run `analyze` first or loosen --genre / --sub-genre.");
        return Ok(());
    }
    let stats = store.stats()?;
    let (emin, emax) = (stats.min_energy.unwrap_or(0.0), stats.max_energy.unwrap_or(1.0));
    print_table(&rows, emin, emax);
    if let Some(out) = export {
        let name = resolve_playlist_name(playlist_name, out, "top");
        let report = write_playlist(out, &name, &rows, traktor_volume)?;
        print_export_report(out, &name, &report);
    }
    Ok(())
}

fn cmd_compat(
    db: &Path,
    query: &str,
    bpm_tol: f32,
    limit: usize,
    any_key: bool,
    half_double_ok: bool,
    export: Option<&Path>,
    playlist_name: Option<&str>,
    traktor_volume: Option<&str>,
) -> Result<()> {
    let store = Store::open(db)?;
    let matches = store.search(query, 5)?;
    let Some(anchor) = matches.first().cloned() else {
        println!("no track found matching `{}`.", query);
        return Ok(());
    };
    let anchor_bpm = anchor.bpm.ok_or_else(|| {
        anyhow::anyhow!("anchor track has no BPM; re-run `analyze` on this track")
    })?;
    let anchor_camelot = Camelot::parse(&anchor.tonality);

    let mut ranges = vec![(anchor_bpm - bpm_tol, anchor_bpm + bpm_tol)];
    if half_double_ok {
        let half = anchor_bpm / 2.0;
        let dbl = anchor_bpm * 2.0;
        ranges.push((half - bpm_tol, half + bpm_tol));
        ranges.push((dbl - bpm_tol, dbl + bpm_tol));
    }
    let range_descriptions: Vec<String> = ranges
        .iter()
        .map(|(lo, hi)| format!("{:.1}–{:.1}", lo, hi))
        .collect();

    println!("Anchor: {} — {}", anchor.artist, anchor.name);
    println!(
        "  {:.1} BPM · key {} · {}",
        anchor_bpm,
        format_key(&anchor.tonality, anchor_camelot.as_ref()),
        if any_key {
            "BPM-only matching".to_string()
        } else {
            format!(
                "Camelot matching {}",
                anchor_camelot
                    .as_ref()
                    .map(|c| format!("({})", c))
                    .unwrap_or_else(|| "(key unparsed — falling back to no key filter)".into())
            )
        }
    );
    if half_double_ok {
        println!("  BPM windows: {}", range_descriptions.join(", "));
    }
    if matches.len() > 1 {
        println!(
            "  ({} other tracks matched the search — refine the query to pick a different anchor)",
            matches.len() - 1
        );
    }
    println!();

    let mut pool_by_id: std::collections::HashMap<String, TrackRow> =
        std::collections::HashMap::new();
    for (lo, hi) in ranges {
        for t in store.in_bpm_range(lo, hi, &anchor.track_id)? {
            pool_by_id.entry(t.track_id.clone()).or_insert(t);
        }
    }
    let apply_key_filter = !any_key && anchor_camelot.is_some();

    let mut compatible: Vec<_> = pool_by_id
        .into_values()
        .filter(|t| {
            if !apply_key_filter {
                return true;
            }
            let anchor_c = anchor_camelot.unwrap();
            match Camelot::parse(&t.tonality) {
                Some(c) => anchor_c.compatible(&c),
                None => false,
            }
        })
        .collect();

    compatible.sort_by(|a, b| {
        let ae = a.energy.unwrap_or(0.0);
        let be = b.energy.unwrap_or(0.0);
        be.partial_cmp(&ae).unwrap_or(std::cmp::Ordering::Equal)
    });
    compatible.truncate(limit);

    if compatible.is_empty() {
        println!(
            "no compatible tracks in ±{:.1} BPM{}{}.",
            bpm_tol,
            if half_double_ok { " (with half/double)" } else { "" },
            if apply_key_filter { " with a compatible key" } else { "" },
        );
        return Ok(());
    }
    let stats = store.stats()?;
    let (emin, emax) = (stats.min_energy.unwrap_or(0.0), stats.max_energy.unwrap_or(1.0));
    print_table(&compatible, emin, emax);
    if let Some(out) = export {
        let default = format!("{} — compat", anchor.name);
        let name = playlist_name.map(|s| s.to_string()).unwrap_or_else(|| {
            out.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
                .unwrap_or(default)
        });
        let report = write_playlist(out, &name, &compatible, traktor_volume)?;
        print_export_report(out, &name, &report);
    }
    Ok(())
}

fn cmd_export(
    db: &Path,
    out: &Path,
    genre: Option<&str>,
    sub_genre: Option<&str>,
    playlist_name: Option<&str>,
    traktor_volume: Option<&str>,
) -> Result<()> {
    let store = Store::open(db)?;
    let contains = |hay: &str, needle: Option<&str>| {
        needle.is_none_or(|n| hay.to_lowercase().contains(&n.to_lowercase()))
    };
    let rows: Vec<TrackRow> = store
        .list_all()?
        .into_iter()
        .filter(|r| contains(&r.genre, genre) && contains(r.sub_genre.as_deref().unwrap_or(""), sub_genre))
        .collect();
    if rows.is_empty() {
        println!("no matching tracks — run `analyze` first or loosen --genre / --sub-genre.");
        return Ok(());
    }
    let name = resolve_playlist_name(playlist_name, out, "Track2Mix");
    let report = write_playlist(out, &name, &rows, traktor_volume)?;
    print_export_report(out, &name, &report);
    Ok(())
}

fn cmd_stats(db: &Path) -> Result<()> {
    let store = Store::open(db)?;
    let s = store.stats()?;
    println!("Database: {}", db.display());
    println!("  total rows           {}", s.total);
    println!("  analyzed             {}", s.analyzed);
    println!("  distinct genres      {}", s.distinct_genres);
    println!("  distinct keys        {}", s.distinct_keys);
    println!(
        "  avg BPM              {}",
        s.avg_bpm
            .map(|v| format!("{:.1}", v))
            .unwrap_or_else(|| "—".into())
    );
    println!(
        "  energy (min/avg/max) {} / {} / {}",
        s.min_energy
            .map(|v| format!("{:.2}", v))
            .unwrap_or_else(|| "—".into()),
        s.avg_energy
            .map(|v| format!("{:.2}", v))
            .unwrap_or_else(|| "—".into()),
        s.max_energy
            .map(|v| format!("{:.2}", v))
            .unwrap_or_else(|| "—".into()),
    );
    Ok(())
}

fn resolve_playlist_name(supplied: Option<&str>, out: &Path, fallback: &str) -> String {
    supplied
        .map(|s| s.to_string())
        .or_else(|| {
            out.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| fallback.to_string())
}

fn format_key(raw: &str, parsed: Option<&Camelot>) -> String {
    let trimmed = raw.trim();
    match (trimmed, parsed) {
        ("", None) => "—".into(),
        (r, None) => format!("{} (unparsed)", r),
        (r, Some(c)) if r.eq_ignore_ascii_case(&c.to_string()) => c.to_string(),
        (r, Some(c)) => format!("{} [{}]", r, c),
    }
}

fn genre_display(r: &TrackRow) -> String {
    match (r.genre.trim(), r.sub_genre.as_deref()) {
        ("", Some(sg)) => sg.to_string(),
        (g, Some(sg)) if !g.eq_ignore_ascii_case(sg) => format!("{} › {}", g, sg),
        (g, _) => g.to_string(),
    }
}

fn print_table(rows: &[TrackRow], lib_min_energy: f32, lib_max_energy: f32) {
    let name_w = rows
        .iter()
        .map(|r| truncate(&r.name, 40).chars().count())
        .max()
        .unwrap_or(10)
        .max(4);
    let artist_w = rows
        .iter()
        .map(|r| truncate(&r.artist, 28).chars().count())
        .max()
        .unwrap_or(10)
        .max(6);
    let genre_w = rows
        .iter()
        .map(|r| truncate(&genre_display(r), 24).chars().count())
        .max()
        .unwrap_or(5)
        .max(5);

    println!(
        "{:<name_w$}  {:<artist_w$}  {:<genre_w$}  {:>6}  {:<8}  {:>3}",
        "name",
        "artist",
        "genre",
        "bpm",
        "key",
        "E",
        name_w = name_w,
        artist_w = artist_w,
        genre_w = genre_w,
    );
    println!(
        "{}",
        "-".repeat(name_w + artist_w + genre_w + 6 + 8 + 3 + 10)
    );
    for r in rows {
        let key_display = Camelot::parse(&r.tonality)
            .map(|c| c.to_string())
            .unwrap_or_else(|| {
                if r.tonality.trim().is_empty() {
                    "—".into()
                } else {
                    r.tonality.clone()
                }
            });
        println!(
            "{:<name_w$}  {:<artist_w$}  {:<genre_w$}  {:>6}  {:<8}  {:>3}",
            truncate(&r.name, 40),
            truncate(&r.artist, 28),
            truncate(&genre_display(r), 24),
            r.bpm
                .filter(|v| *v > 0.0)
                .map(|v| format!("{:.1}", v))
                .unwrap_or_else(|| "—".into()),
            truncate(&key_display, 8),
            r.energy
                .map(|v| format!("{}", energy_score(v, lib_min_energy, lib_max_energy)))
                .unwrap_or_else(|| "—".into()),
            name_w = name_w,
            artist_w = artist_w,
            genre_w = genre_w,
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
