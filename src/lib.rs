use encoding_rs::WINDOWS_1251;

// ------------------------------------------------------------------ //
//  Invariant: describes HOW the string is broken                      //
//  Each variant maps to exactly one detection + one fix strategy.     //
//  Add new variants here as new corruption patterns are discovered.   //
// ------------------------------------------------------------------ //
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodingIssue {
    /// Win-1251 bytes were mis-read as Latin-1 and stored as UTF-8.
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

/// Fraction of characters that must fall in Latin Extended (0x80–0xFF)
/// for the text to be considered a Win1251-as-Latin1 candidate.
const LATIN_EXT_RATIO_THRESHOLD: f32 = 0.30;

/// Returns the encoding issue detected in `text`, or `None` if the
/// text appears to be correct UTF-8 that does not need fixing.
pub fn detect_encoding_issue(text: &str) -> Option<EncodingIssue> {
    if text.is_empty() {
        return None;
    }

    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();

    // -- Guard: already contains Cyrillic → nothing to fix
    let has_cyrillic = chars
        .iter()
        .any(|&c| ('\u{0400}'..='\u{04FF}').contains(&c));
    if has_cyrillic {
        return None;
    }

    // -- Guard: mostly printable ASCII (English / neutral text) → skip
    // "Mostly" = more than 60 % of characters are plain ASCII (0x20–0x7E).
    let ascii_printable_count = chars
        .iter()
        .filter(|&&c| (c as u32) >= 0x20 && (c as u32) <= 0x7E)
        .count();
    let ascii_ratio = ascii_printable_count as f32 / total as f32;
    if ascii_ratio > 0.60 {
        return None;
    }

    // -- Guard: contains non-Latin-1 codepoints (>0xFF) that are not
    //    standard punctuation / spaces → likely a real UTF-8 language,
    //    not a mojibake artifact.
    let has_non_latin1 = chars
        .iter()
        .any(|&c| (c as u32) > 0xFF);
    if has_non_latin1 {
        return None;
    }

    // -- Heuristic: high ratio of Latin Extended (0x80–0xFF)
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

/// Attempts to repair `text` according to the detected `issue`.
/// Returns the corrected UTF-8 string or a `FixError`.
pub fn fix_encoding(text: &str, issue: &EncodingIssue) -> Result<String, FixError> {
    match issue {
        EncodingIssue::Win1251AsLatin1 => fix_win1251_as_latin1(text),
    }
}

/// Reverses the Win-1251 → Latin-1 misread:
///   char codepoint (u32) → u8 byte  → decode as Win-1251
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