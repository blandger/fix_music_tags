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

pub const LATIN_EXT_MIN_COUNT: usize = 3;

/// Common Western European diacritics (French, Spanish, German, etc.)
/// These are VALID Latin-1 characters, not mojibake artifacts.
const WESTERN_DIACRITICS: &[char] = &[
    // French/Spanish/Portuguese vowels with accents (most common)
    'à', 'á', 'â', 'ã',  // a variants (but NOT ä — overlaps with Cyrillic)
    'ç',                  // c cedilla
    'é', 'è', 'ê',       // e variants (but NOT ë — overlaps)
    'í', 'î',            // i variants (but NOT ì, ï — overlap)
    'ó', 'ô', 'õ',       // o variants (but NOT ò, ö — overlap)
    'ú', 'û',            // u variants (but NOT ù, ü — overlap)
    // Uppercase variants of the above
    'À', 'Á', 'Â', 'Ã',
    'Ç',
    'É', 'È', 'Ê',
    'Í', 'Î',
    'Ó', 'Ô', 'Õ',
    'Ú', 'Û',
    // Special ligatures and common symbols
    'æ', 'œ', 'Æ', 'Œ',
];

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

    // Guard: already contains Cyrillic
    if chars.iter().any(|&c| ('\u{0400}'..='\u{04FF}').contains(&c)) {
        return None;
    }

    // Guard: non-Latin-1 codepoints indicate real UTF-8
    if chars.iter().any(|&c| (c as u32) > 0xFF) {
        return None;
    }

    // Count Latin Extended characters (0x80–0xFF)
    let latin_ext_chars: Vec<char> = chars
        .iter()
        .copied()
        .filter(|&c| (c as u32) > 0x7F && (c as u32) <= 0xFF)
        .collect();

    let latin_ext_count = latin_ext_chars.len();

    if latin_ext_count == 0 {
        // No Latin Extended → check ASCII ratio
        let ascii_count = chars
            .iter()
            .filter(|&&c| (c as u32) >= 0x20 && (c as u32) <= 0x7E)
            .count();
        let ascii_ratio = ascii_count as f32 / total as f32;
        if ascii_ratio > ASCII_RATIO_SKIP_THRESHOLD {
            return None;
        }
        return None; // No suspicious characters at all
    }

    // Guard: if ALL Latin Extended chars are valid Western diacritics,
    // this is a valid Western European language (French, Spanish, etc.)
    let all_are_western_diacritics = latin_ext_chars
        .iter()
        .all(|c| WESTERN_DIACRITICS.contains(c));

    if all_are_western_diacritics {
        return None; // Valid Western European text, not mojibake
    }

    // Check if we have enough non-diacritic Latin Extended chars
    let non_diacritic_count = latin_ext_chars
        .iter()
        .filter(|c| !WESTERN_DIACRITICS.contains(c))
        .count();

    let latin_ext_ratio = latin_ext_count as f32 / total as f32;

    // High ratio of suspicious (non-diacritic) Latin Extended chars
    if latin_ext_ratio >= LATIN_EXT_RATIO_THRESHOLD && non_diacritic_count >= 2 {
        return Some(EncodingIssue::Win1251AsLatin1);
    }

    // Or absolute count of suspicious chars
    if non_diacritic_count >= LATIN_EXT_MIN_COUNT {
        return Some(EncodingIssue::Win1251AsLatin1);
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
        assert_eq!(
            detect_encoding_issue("Ïðèïàäêè ìîëîäîñòè"),
            Some(EncodingIssue::Win1251AsLatin1)
        );
    }

    #[test]
    fn detect_skips_french() {
        // French name with diacritics — should NOT be detected as broken
        assert_eq!(detect_encoding_issue("Irénée"), None);
        assert_eq!(detect_encoding_issue("François"), None);
        assert_eq!(detect_encoding_issue("Hôtel de Ville"), None);
        assert_eq!(detect_encoding_issue("Björk"), None); // Swedish
        assert_eq!(detect_encoding_issue("José García"), None); // Spanish
        assert_eq!(detect_encoding_issue("Dançar pra Não Dançar"), None); // Spanish
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
