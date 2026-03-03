pub(crate) mod types;
pub mod detect;

use encoding_rs::WINDOWS_1251;
use crate::types::{EncodingIssue, FixError};

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

    Ok(decoded.trim().to_string())
}

// ------------------------------------------------------------------ //
//  Tests                                                               //
// ------------------------------------------------------------------ //
#[cfg(test)]
mod tests {
    use crate::detect::detect_encoding_issue;
    use super::*;

    // ---- fix_win1251_as_latin1 ------------------------------------
    #[test]
    fn fix_mixed_value() {
        let broken = "The Best.×àñòü 2";
        assert_eq!(
            detect_encoding_issue(broken),
            Some(EncodingIssue::Win1251AsLatin1)
        );
        let fixed = fix_encoding(broken, &EncodingIssue::Win1251AsLatin1).unwrap();
        assert_eq!(fixed, "The Best.Часть 2");
    }

    #[test]
    fn fix_magic_lake() {
        let broken = "Âîëøåáíîå îçåðî";
        assert_eq!(
            detect_encoding_issue(broken),
            Some(EncodingIssue::Win1251AsLatin1)
        );
        let fixed = fix_encoding(broken, &EncodingIssue::Win1251AsLatin1).unwrap();
        assert_eq!(fixed, "Волшебное озеро");
    }

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

}