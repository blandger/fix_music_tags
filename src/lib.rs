use encoding_rs::WINDOWS_1251;

// ------------------------------------------------------------------ //
//  Invariant: describes HOW a string is broken.                       //
//  Each variant maps to exactly one detection + one fix strategy.     //
//  Add new variants here as new corruption patterns are discovered.   //
// ------------------------------------------------------------------ //
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodingIssue {
    /// Win-1251 bytes were mis-read as Latin-1 and re-encoded as UTF-8.
    /// Each char's codepoint == the original Win-1251 byte (0x00–0xFF).
    Win1251AsLatin1,
    // Future variants, e.g.:
    // Cp866AsLatin1,
    // DoubleMojibake,
}

// ------------------------------------------------------------------ //
//  Fix error                                                           //
// ------------------------------------------------------------------ //
#[derive(Debug)]
pub enum FixError {
    /// A character's codepoint is outside the expected byte range.
    CodepointOutOfRange { ch: char, codepoint: u32 },
    /// The recovered byte sequence is not valid for the target encoding.
    DecodingFailed,
}

impl std::fmt::Display for FixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixError::CodepointOutOfRange { ch, codepoint } => {
                write!(f, "char '{ch}' has codepoint {codepoint:#X}, expected <= 0xFF")
            }
            FixError::DecodingFailed => write!(f, "byte sequence invalid for target encoding"),
        }
    }
}

impl std::error::Error for FixError {}

// ------------------------------------------------------------------ //
//  Detection                                                           //
// ------------------------------------------------------------------ //

/// Fraction of characters in the Latin Extended range (0x80–0xFF)
/// required to trigger the Win1251-as-Latin1 heuristic.
const LATIN_EXT_RATIO_THRESHOLD: f32 = 0.30;

/// Fraction of plain ASCII printable characters (0x20–0x7E) above
/// which text is considered "mostly English / neutral" and skipped.
const ASCII_RATIO_SKIP_THRESHOLD: f32 = 0.60;

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

    // Guard: non-Latin-1 codepoints (> 0xFF) that are not ASCII
    // indicate a real non-Latin UTF-8 language — do not touch.
    let has_non_latin1 = chars.iter().any(|&c| (c as u32) > 0xFF);
    if has_non_latin1 {
        return None;
    }

    // Guard: mostly plain ASCII → treat as English or neutral text, skip.
    let ascii_count = chars
        .iter()
        .filter(|&&c| (c as u32) >= 0x20 && (c as u32) <= 0x7E)
        .count();
    let ascii_ratio = ascii_count as f32 / total as f32;
    if ascii_ratio > ASCII_RATIO_SKIP_THRESHOLD {
        return None;
    }

    // Heuristic: high fraction of Latin Extended bytes → Win1251-as-Latin1.
    let latin_ext_count = chars
        .iter()
        .filter(|&&c| (c as u32) > 0x7F && (c as u32) <= 0xFF)
        .count();
    let latin_ext_ratio = latin_ext_count as f32 / total as f32;
    if latin_ext_ratio >= LATIN_EXT_RATIO_THRESHOLD {
        return Some(EncodingIssue::Win1251AsLatin1);
    }

    None
}

// ------------------------------------------------------------------ //
//  Fix                                                                 //
// ------------------------------------------------------------------ //

/// Repairs `text` according to the detected `issue`.
/// Returns the corrected UTF-8 string or a [`FixError`].
pub fn fix_encoding(text: &str, issue: &EncodingIssue) -> Result<String, FixError> {
    match issue {
        EncodingIssue::Win1251AsLatin1 => fix_win1251_as_latin1(text),
    }
}

/// Reverses the Win-1251 → Latin-1 misread:
///   char codepoint (u32) → u8 byte → decode as Win-1251 → UTF-8
fn fix_win1251_as_latin1(text: &str) -> Result<String, FixError> {
    let raw_bytes: Result<Vec<u8>, FixError> = text
        .chars()
        .map(|c| {
            let cp = c as u32;
            if cp <= 0xFF {
                Ok(cp as u8)
            } else {
                Err(FixError::CodepointOutOfRange { ch: c, codepoint: cp })
            }
        })
        .collect();

    let raw_bytes = raw_bytes?;

    let (decoded, _enc, had_errors) = WINDOWS_1251.decode(&raw_bytes);
    if had_errors {
        return Err(FixError::DecodingFailed);
    }

    Ok(decoded.into_owned())
}

// ------------------------------------------------------------------ //
//  Tests                                                               //
// ------------------------------------------------------------------ //
#[cfg(test)]
mod tests {
    use super::*;

    // ---- fix_win1251_as_latin1 ------------------------------------

    #[test]
    fn fix_known_broken_title() {
        let result = fix_encoding("Îäèíàêîâûå ñíû", &EncodingIssue::Win1251AsLatin1).unwrap();
        assert_eq!(result, "Одинаковые сны");
    }

    #[test]
    fn fix_known_broken_artist() {
        let result = fix_encoding("Îëåã Ìèòÿåâ", &EncodingIssue::Win1251AsLatin1).unwrap();
        assert_eq!(result, "Олег Митяев");
    }

    #[test]
    fn fix_known_broken_album() {
        let result = fix_encoding("Çàïàõ ñíåãà", &EncodingIssue::Win1251AsLatin1).unwrap();
        assert_eq!(result, "Запах снега");
    }

    #[test]
    fn fix_pure_ascii_unchanged() {
        // Plain ASCII is a valid subset of both Latin-1 and Win-1251,
        // so it round-trips without change.
        let result = fix_encoding("Hello", &EncodingIssue::Win1251AsLatin1).unwrap();
        assert_eq!(result, "Hello");
    }

    #[test]
    fn fix_rejects_codepoint_above_ff() {
        // U+0400 (Ѐ) is a Cyrillic char whose codepoint > 0xFF — must fail.
        let broken = "\u{0400}test";
        let err = fix_encoding(broken, &EncodingIssue::Win1251AsLatin1);
        assert!(matches!(err, Err(FixError::CodepointOutOfRange { .. })));
    }

    // ---- detect_encoding_issue ------------------------------------

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