use crate::types::EncodingIssue;

// ------------------------------------------------------------------ //
//  Detection                                                           //
// ------------------------------------------------------------------ //

/// Fraction of characters in the Latin Extended range (0x80–0xFF)
/// required to trigger the Win1251-as-Latin1 heuristic.
pub const LATIN_EXT_RATIO_THRESHOLD: f32 = 0.30;
/// Fraction of plain ASCII printable characters (0x20–0x7E) above
/// which text is considered "mostly English / neutral" and skipped.
pub const ASCII_RATIO_SKIP_THRESHOLD: f32 = 0.60;

/// Returns the encoding issue detected in `text`, or `None` when the
/// text appears to be valid UTF-8 that does not require fixing.
///
/// Extend this function with additional `if`-branches or helper
/// functions for every new corruption pattern.
pub fn detect_encoding_issue(text: &str) -> Option<EncodingIssue> {
    if text.is_empty() {
        return None;
    }

    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();

    // Guard: already contains Cyrillic → nothing to fix.
    let has_cyrillic = chars
        .iter()
        .any(|&c| ('\u{0400}'..='\u{04FF}').contains(&c));
    if has_cyrillic {
        return None;
    }

    // Guard: non-Latin-1 codepoints (> 0xFF) indicate real UTF-8 language.
    let has_non_latin1 = chars.iter().any(|&c| (c as u32) > 0xFF);
    if has_non_latin1 {
        return None;
    }

    // Check for Latin Extended (0x80–0xFF) first — this is the primary signal.
    let latin_ext_count = chars
        .iter()
        .filter(|&&c| (c as u32) > 0x7F && (c as u32) <= 0xFF)
        .count();
    let latin_ext_ratio = latin_ext_count as f32 / total as f32;

    // If we have significant Latin Extended presence, this is likely mojibake.
    // Either high ratio OR absolute count >= 3 (to catch mixed strings like "The Best.×àñòü 2").
    if latin_ext_ratio >= LATIN_EXT_RATIO_THRESHOLD || latin_ext_count >= 3 {
        return Some(EncodingIssue::Win1251AsLatin1);
    }

    // Guard: mostly plain ASCII with no Latin Extended → treat as English.
    // This guard is now AFTER Latin Extended check, so mixed strings are caught.
    let ascii_count = chars
        .iter()
        .filter(|&&c| (c as u32) >= 0x20 && (c as u32) <= 0x7E)
        .count();
    let ascii_ratio = ascii_count as f32 / total as f32;
    if ascii_ratio > ASCII_RATIO_SKIP_THRESHOLD {
        return None;
    }

    None
}

#[cfg(test)]
mod tests {
    // ---- detect_encoding_issue ------------------------------------

    use crate::detect::detect_encoding_issue;
    use crate::types::EncodingIssue;

    #[test]
    fn detect_finds_win1251_latin1() {
        assert_eq!(
            detect_encoding_issue("Îäèíàêîâûå ñíû"),
            Some(EncodingIssue::Win1251AsLatin1)
        );
        assert_eq!(
            detect_encoding_issue("Îëåã Ìèòÿåâ"),
            Some(EncodingIssue::Win1251AsLatin1)
        );
        assert_eq!(
            detect_encoding_issue("Çàïàõ ñíåãà"),
            Some(EncodingIssue::Win1251AsLatin1)
        );
    }

    #[test]
    fn detect_skips_correct_cyrillic() {
        assert_eq!(detect_encoding_issue("Одинаковые сны"), None);
        assert_eq!(detect_encoding_issue("Олег Митяев"), None);
    }

    #[test]
    fn detect_skips_plain_english() {
        assert_eq!(detect_encoding_issue("The Dark Side of the Moon"), None);
        assert_eq!(detect_encoding_issue("Track 01"), None);
    }

    #[test]
    fn detect_skips_empty_string() {
        assert_eq!(detect_encoding_issue(""), None);
    }

    #[test]
    fn detect_skips_non_latin1_unicode() {
        // Japanese — codepoints > 0xFF, not a Latin-1 mojibake artifact.
        assert_eq!(detect_encoding_issue("日本語のテキスト"), None);
    }

    #[test]
    fn detect_skips_mixed_english_and_numbers() {
        assert_eq!(detect_encoding_issue("Album 2024 Remastered"), None);
    }
}
