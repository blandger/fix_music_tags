use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info, warn};

use fix_music_tags::scan;
use fix_music_tags::types::{ScanStats};

// ------------------------------------------------------------------ //
//  CLI arguments                                                       //
// ------------------------------------------------------------------ //
#[derive(Parser, Debug)]
#[command(
    name = "fix_music_tags",
    about = "Detects and repairs broken audio tags (MP3, FLAC, WAV, OGG, …)",
    version
)]
struct Args {
    /// Directory to scan recursively for audio files
    #[arg(short, long, value_name = "DIR")]
    dir: PathBuf,

    /// Dry-run: detect and report issues without modifying any files
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    dry_run: bool,
}

/// Main entry point                                                       //
fn main() {
    // Initialize tracing; RUST_LOG env variable controls the filter level
    // e.g. RUST_LOG=info ./fix_music_tags --dir ./music
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

    let mut stats = ScanStats::default();
    match scan::scan_directory(&args.dir, args.dry_run, &mut stats) {
        Ok(()) => info!(
            processed = stats.processed,
            fixed = stats.fixed,
            skipped = stats.skipped,
            errors = stats.errors,
            "Scan complete"
        ),
        Err(e) => error!("Fatal error during scan: {e}"),
    }
}
