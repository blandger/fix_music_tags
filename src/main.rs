use clap::Parser;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

use fix_music_tags::{detect_encoding_issue, fix_encoding};

// ------------------------------------------------------------------ //
//  CLI arguments                                                       //
// ------------------------------------------------------------------ //
#[derive(Parser, Debug)]
#[command(
    name = "fix_music_tags",
    about = "Detects and repairs broken text in MP3 tags",
    version
)]
struct Args {
    /// Directory to scan for MP3 files
    #[arg(short, long, value_name = "DIR")]
    dir: PathBuf,

    /// Dry-run mode: detect and report issues without modifying files
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    dry_run: bool,
}

// ------------------------------------------------------------------ //
//  Entry point                                                         //
// ------------------------------------------------------------------ //
fn main() {
    // Initialize tracing; RUST_LOG env variable controls the filter level
    // e.g. RUST_LOG=debug ./fix_music_tags --dir ./music
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    info!(dir = %args.dir.display(), dry_run = args.dry_run, "Starting scan");

    if args.dry_run {
        warn!("Dry-run mode is ON — no files will be modified");
    }

    match scan_directory(&args.dir, args.dry_run) {
        Ok(stats) => info!(
            processed = stats.processed,
            fixed = stats.fixed,
            skipped = stats.skipped,
            errors = stats.errors,
            "Scan complete"
        ),
        Err(e) => error!("Fatal error during scan: {e}"),
    }
}

// ------------------------------------------------------------------ //
//  Scan logic (skeleton — MP3 tag reading comes later)                //
// ------------------------------------------------------------------ //
#[derive(Default, Debug)]
struct ScanStats {
    processed: usize,
    fixed: usize,
    skipped: usize,
    errors: usize,
}

fn scan_directory(dir: &PathBuf, dry_run: bool) -> Result<ScanStats, std::io::Error> {
    let mut stats = ScanStats::default();

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        // Skip non-MP3 files (later this will be replaced by proper tag reading)
        if path.extension().and_then(|e| e.to_str()) != Some("mp3") {
            debug!(path = %path.display(), "Skipping non-MP3 file");
            continue;
        }

        stats.processed += 1;

        // Placeholder: use the filename stem as the "tag value" for now
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();

        process_tag_value(&stem, "filename", dry_run, &mut stats);
    }

    Ok(stats)
}

/// Runs detection + optional fix on a single tag value.
fn process_tag_value(value: &str, tag_name: &str, dry_run: bool, stats: &mut ScanStats) {
    match detect_encoding_issue(value) {
        None => {
            debug!(tag = tag_name, value, "Tag looks fine, skipping");
            stats.skipped += 1;
        }
        Some(issue) => {
            debug!(tag = tag_name, value, ?issue, "Broken encoding detected");

            match fix_encoding(value, &issue) {
                Err(e) => {
                    error!(tag = tag_name, value, error = %e, "Failed to fix tag");
                    stats.errors += 1;
                }
                Ok(fixed) => {
                    if dry_run {
                        info!(
                            tag = tag_name,
                            original = value,
                            fixed,
                            "[dry-run] Would fix tag"
                        );
                    } else {
                        info!(tag = tag_name, original = value, fixed, "Fixed tag");
                        // TODO: write fixed value back to the MP3 file
                    }
                    stats.fixed += 1;
                }
            }
        }
    }
}