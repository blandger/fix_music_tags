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

///  Public entry point: recursively scan directory                    //
pub fn scan_directory(dir: &PathBuf, dry_run: bool, stats: &mut ScanStats) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        // Recurse into subdirectories
        if path.is_dir() {
            info!(dir = %path.display(), "Processing subfolder");
            scan_directory(&path, dry_run, stats)?;
            continue;
        }

        if !path.is_file() {
            continue;
        }

        // Process single audio file
        process_single_file(&path, dry_run, stats);
    }

    Ok(())
}

///  Process a single audio file                                       //
fn process_single_file(path: &PathBuf, dry_run: bool, stats: &mut ScanStats) {
    info!(file = %path.display(), "Processing file");

    // Open file and read tags
    let tagged_file = match Probe::open(path).and_then(|p| p.read()) {
        Ok(tf) => tf,
        Err(e) => {
            warn!(file = %path.display(), error = %e, "lofty could not read file, skipping");
            stats.errors += 1;
            return;
        }
    };

    stats.processed += 1;

    // Determine tag type (ID3v1 vs ID3v2) and get tag reference
    let (tag, had_id3v1_only) = match get_tag_with_type(&tagged_file) {
        Some(result) => result,
        None => {
            debug!(file = %path.display(), "No tags found in file");
            return;
        }
    };

    // Collect patches for broken tags
    let patches = collect_patches_from_tag(tag, path, stats);

    if patches.is_empty() {
        return;
    }

    // Apply patches to file
    if !dry_run {
        apply_patches_to_file(path, patches, had_id3v1_only, stats);
    } else {
        // Dry-run: just count as fixed without writing
        stats.fixed += 1;
    }
}

///  Get tag with type detection (ID3v1 vs modern)                     //
fn get_tag_with_type(tagged_file: &lofty::file::TaggedFile) -> Option<(&lofty::tag::Tag, bool)> {
    match tagged_file.primary_tag() {
        Some(t) => Some((t, false)), // Has primary tag → ID3v2 or modern format
        None => {
            // No primary tag → check if ID3v1-only
            match tagged_file.first_tag() {
                Some(t) => {
                    let is_id3v1 = t.tag_type() == lofty::tag::TagType::Id3v1;
                    Some((t, is_id3v1))
                }
                None => None,
            }
        }
    }
}

///  Collect patches from tag fields                                   //
fn collect_patches_from_tag(
    tag: &lofty::tag::Tag,
    path: &PathBuf,
    stats: &mut ScanStats,
) -> Vec<Patch> {
    let mut patches = Vec::new();

    // Extract tag values (extend lifetime)
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
                debug!(tag = field_name, value, "Tag ok");
                stats.skipped += 1;
            }
            Some(issue) => match fix_encoding(value, &issue) {
                Err(e) => {
                    error!(
                        file  = %path.display(),
                        tag   = field_name,
                        value,
                        error = %e,
                        "Failed to fix tag"
                    );
                    stats.errors += 1;
                }
                Ok(fixed) => {
                    error!(
                        file     = %path.display(),
                        tag      = field_name,
                        original = value.trim(),
                        fixed    = %fixed,
                        "Tag needs fixing !!"
                    );
                    patches.push(Patch {
                        field_name,
                        fixed_value: fixed,
                    });
                }
            },
        }
    }

    patches
}

///  Apply patches to file and save                                    //
fn apply_patches_to_file(
    path: &PathBuf,
    patches: Vec<Patch>,
    had_id3v1_only: bool,
    stats: &mut ScanStats,
) {
    // Re-open file mutably for writing
    let mut tagged_file = match Probe::open(path).and_then(|p| p.read()) {
        Ok(tf) => tf,
        Err(e) => {
            error!(file = %path.display(), error = %e, "Failed to re-open file for writing");
            stats.errors += 1;
            return;
        }
    };

    // Upgrade ID3v1 → ID3v2 if needed
    if had_id3v1_only {
        info!(file = %path.display(), "Upgrading ID3v1 → ID3v2 for UTF-8 support");
        upgrade_to_id3v2(&mut tagged_file);
    }

    // Get mutable tag reference
    let tag = match get_mutable_tag(&mut tagged_file, path) {
        Some(t) => t,
        None => {
            warn!(file = %path.display(), "No writable tag found");
            stats.errors += 1;
            return;
        }
    };

    // Apply patches
    info!(file = %path.display(), patches = patches.len(), "Applying patches");
    for patch in &patches {
        match patch.field_name {
            "title" => tag.set_title(patch.fixed_value.clone()),
            "artist" => tag.set_artist(patch.fixed_value.clone()),
            "album" => tag.set_album(patch.fixed_value.clone()),
            "genre" => tag.set_genre(patch.fixed_value.clone()),
            "comment" => tag.set_comment(patch.fixed_value.clone()),
            _ => {}
        }
    }

    // Save to disk
    let options = WriteOptions::default().preferred_language(Some(RUSSIAN));
    match tagged_file.save_to_path(path, options) {
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

///  Get mutable tag reference                                         //
fn get_mutable_tag<'a>(
    tagged_file: &'a mut lofty::file::TaggedFile,
    path: &PathBuf,
) -> Option<&'a mut lofty::tag::Tag> {
    // Check what's available without consuming the borrow
    let has_primary = tagged_file.primary_tag().is_some();
    let has_first = tagged_file.first_tag().is_some();

    if has_primary {
        tagged_file.primary_tag_mut()
    } else if has_first {
        tagged_file.first_tag_mut()
    } else {
        debug!(file = %path.display(), "No mutable tag available");
        None
    }
}

///  Upgrade ID3v1 → ID3v2 for UTF-8 support                           //
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
    // Remove old ID3v1 (can't store UTF-8)
    tagged_file.remove(TagType::Id3v1);

    // Create new ID3v2 tag with old data
    let mut id3v2 = lofty::tag::Tag::new(TagType::Id3v2);

    if let Some(t) = id3v1_data.0 {
        id3v2.set_title(t);
    }
    if let Some(a) = id3v1_data.1 {
        id3v2.set_artist(a);
    }
    if let Some(al) = id3v1_data.2 {
        id3v2.set_album(al);
    }
    if let Some(g) = id3v1_data.3 {
        id3v2.set_genre(g);
    }

    tagged_file.insert_tag(id3v2);
}