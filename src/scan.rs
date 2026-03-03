use std::path::PathBuf;
use tracing::{debug, error, info, warn};
use lofty::probe::Probe;
use lofty::config::WriteOptions;
use lofty::tag::items::RUSSIAN;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::prelude::Accessor;
use crate::detect::detect_encoding_issue;
use crate::fixing::fix_encoding;
use crate::types::{Patch, ScanStats};

// ------------------------------------------------------------------ //
//  Scan logic                                                        //
// ------------------------------------------------------------------ //

pub fn scan_directory(dir: &PathBuf, dry_run: bool, stats: &mut ScanStats) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            info!(file = %path.display(), "Processing SUB-FOLDER !");
            scan_directory(&path, dry_run, stats)?;
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
                warn!(file = %path.display(), error = %e, "lofty could not read file, skipping");
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
        // Try primary_tag first, fall back to first available tag.
        // Determine tag type and get reference for reading
        let (tag, had_id3v1_only) = match tagged_file.primary_tag() {
            Some(t) => (t, false), // Has primary tag → ID3v2 or modern format
            None => {
                // No primary tag → check if ID3v1-only
                match tagged_file.first_tag() {
                    Some(t) => {
                        let is_id3v1 = t.tag_type() == lofty::tag::TagType::Id3v1;
                        (t, is_id3v1)
                    }
                    None => {
                        debug!(file = %path.display(), "No tags found in file");
                        continue;
                    }
                }
            }
        };

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

            match detect_encoding_issue(value.trim()) {
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
                        error!(
                            file = %path.display(),
                            tag      = field_name,
                            original = value.trim(),
                            fixed    = %fixed,
                            dry_run,
                            "Tag needs fixing !!"
                        );
                        if !dry_run {
                            patches.push(Patch {
                                field_name: field_name,
                                fixed_value: fixed,
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
                    // Upgrade ID3v1 → ID3v2 if needed (we saved this info earlier)
                    if had_id3v1_only {
                        info!(file = %path.display(), "Upgrading ID3v1 → ID3v2 for UTF-8 support");
                        upgrade_to_id3v2(&mut tagged_file);
                    }

                    // Get mutable tag reference
                    let tag = if let Some(t) = tagged_file.primary_tag_mut() {
                        t
                    } else if let Some(t) = tagged_file.first_tag_mut() {
                        t
                    } else {
                        warn!(file = %path.display(), "No writable tag found");
                        stats.errors += 1;
                        continue;
                    };

                    if !patches.is_empty() {
                        info!(file = %path.display(), "will be patched...");
                    }
                    // Apply patches
                    for patch in &patches {
                        match patch.field_name {
                            "title"  => tag.set_title(patch.fixed_value.clone()),
                            "artist" => tag.set_artist(patch.fixed_value.clone()),
                            "album"  => tag.set_album(patch.fixed_value.clone()),
                            "genre"  => tag.set_genre(patch.fixed_value.clone()),
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
                }
            }
        } else if file_had_fix {
            stats.fixed += 1;
        }
    }

    Ok(())
}

/// Converts ID3v1 tag to ID3v2 with UTF-8 support.
fn upgrade_to_id3v2(tagged_file: &mut lofty::file::TaggedFile) {
    // Read existing ID3v1 data to preserve it
    let id3v1_data = if let Some(tag) = tagged_file.first_tag() {
        (
            tag.title().as_deref().map(String::from),
            tag.artist().as_deref().map(String::from),
            tag.album().as_deref().map(String::from),
            tag.genre().as_deref().map(String::from),
        )
    } else {
        (None, None, None, None)
    };
    info!("Upgrading ID3v1 → ID3v2: tags data = {:?}", &id3v1_data);

    use lofty::tag::TagType;
    use tracing::info;
    // Remove old ID3v1 (can't store UTF-8)
    tagged_file.remove(TagType::Id3v1);

    // Create new ID3v2 tag with old data
    let mut id3v2 = lofty::tag::Tag::new(TagType::Id3v2);

    if let Some(t) = id3v1_data.0 { id3v2.set_title(t); }
    if let Some(a) = id3v1_data.1 { id3v2.set_artist(a); }
    if let Some(al) = id3v1_data.2 { id3v2.set_album(al); }
    if let Some(g) = id3v1_data.3 { id3v2.set_genre(g); }

    tagged_file.insert_tag(id3v2);
}