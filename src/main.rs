use clap::Parser;
use lofty::config::WriteOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::Accessor;
use lofty::tag::items::RUSSIAN;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

use fix_music_tags::{detect_encoding_issue, fix_encoding};

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

// ------------------------------------------------------------------ //
//  Entry point                                                       //
// ------------------------------------------------------------------ //
fn main() {
    // Initialize tracing; RUST_LOG env variable controls the filter level
    // e.g. RUST_LOG=debug ./fix_music_tags --dir ./music
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug")),
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
//  Scan logic                                                        //
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

        if path.is_dir() {
            info!(file = %path.display(), "Processing SUB-FOLDER !");
            scan_directory(&path, dry_run)?;
        }
        if !path.is_file() {
            continue;
        }

        // Log the file name before any processing begins.
        info!(file = %path.display(), "Processing file");

        // Let lofty decide whether it can read tags from this file.
        // This covers MP3, FLAC, WAV, OGG, AIFF, AAC, and more.
        let tagged_file = match Probe::open(&path).and_then(|p| p.read()) {
            Ok(tf) => tf,
            Err(e) => {
                error!(file = %path.display(), error = %e, "lofty could not read file, skipping");
                stats.errors += 1;
                continue;
            }
        };

        stats.processed += 1;

        // Collect all tags attached to the file (a file may have more than one).
        let mut file_had_fix = false;

        // lofty's TaggedFile owns the tags; we need a mutable reference later,
        // so we build a list of (field_name, broken_value, fixed_value) first,
        // then apply them in a second pass.
        // Read only the primary tag (avoids duplicates from ID3v1 + ID3v2).
        let Some(tag) = tagged_file.primary_tag() else {
            debug!(file = %path.display(), "No primary tag found");
            continue;
        };

        // Gather fixes for every tag on the file.
        struct Patch {
            field: &'static str,
            fixed: String,
        }
        let mut patches: Vec<Patch> = Vec::new();

        let title = tag.title();
        let artist = tag.artist();
        let album = tag.album();
        let genre = tag.genre();
        let comment = tag.comment();

        let fields: &[(&'static str, Option<&str>)] = &[
            ("title", title.as_deref()),
            ("artist", artist.as_deref()),
            ("album", album.as_deref()),
            ("genre", genre.as_deref()),
            ("comment", comment.as_deref()),
        ];

        for &(field_name, maybe_value) in fields {
            let Some(value) = maybe_value else { continue };

            match detect_encoding_issue(value) {
                None => {
                    debug!(
                        /*file = %path.display(), */ tag = field_name,
                        value, "Tag ok"
                    );
                    stats.skipped += 1;
                }
                Some(issue) => match fix_encoding(value, &issue) {
                    Err(e) => {
                        error!(
                            tag   = field_name,
                            value,
                            error = %e,
                            "Failed to fix tag"
                        );
                        stats.errors += 1;
                    }
                    Ok(fixed) => {
                        info!(
                            tag      = field_name,
                            original = value,
                            fixed    = %fixed,
                            dry_run,
                            "Tag needs fixing"
                        );
                        if !dry_run {
                            patches.push(Patch {
                                field: field_name,
                                fixed,
                            });
                        }
                        file_had_fix = true;
                    }
                },
            }
        }

        // Apply patches and write the file back to disk (non-dry-run only).
        if !dry_run && !patches.is_empty() {
            // Re-open mutably so we can write.
            match Probe::open(&path).and_then(|p| p.read()) {
                Err(e) => {
                    error!(file = %path.display(), error = %e, "Failed to re-open file for writing");
                    stats.errors += 1;
                }
                Ok(mut tagged_file) => {
                    if let Some(tag) = tagged_file.primary_tag_mut() {
                        // Remove existing comment frames to avoid validation errors.
                        // Many files have COMM frames with invalid lang=[0,0,0].
                        // tag.remove_key(ItemKey::Comment);

                        for patch in &patches {
                            match patch.field {
                                "title" => tag.set_title(patch.fixed.clone()),
                                "artist" => tag.set_artist(patch.fixed.clone()),
                                "album" => tag.set_album(patch.fixed.clone()),
                                "genre" => tag.set_genre(patch.fixed.clone()),
                                "comment" => tag.set_comment(patch.fixed.clone()),
                                _ => {}
                            }
                        }

                        let options = WriteOptions::default().preferred_language(Some(RUSSIAN));
                        match tagged_file.save_to_path(&path, options) {
                            Ok(_) => {
                                info!(file = %path.display(), "File saved successfully");
                                stats.fixed += 1;
                            }
                            Err(e) => {
                                error!(file = %path.display(), error = %e, "Failed to save file");
                                stats.errors += 1;
                            }
                        }
                    } else {
                        warn!(file = %path.display(), "No primary tag found, cannot write patches");
                        stats.errors += 1;
                    }
                }
            }
        } else if file_had_fix {
            stats.fixed += 1;
        }
    }

    Ok(stats)
}
