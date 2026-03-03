
/// Fraction of characters in the Latin Extended range (0x80–0xFF)
/// required to trigger the Win1251-as-Latin1 heuristic.
pub const LATIN_EXT_RATIO_THRESHOLD: f32 = 0.30;
/// Fraction of plain ASCII printable characters (0x20–0x7E) above
/// which text is considered "mostly English / neutral" and skipped.
pub const ASCII_RATIO_SKIP_THRESHOLD: f32 = 0.60;


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
