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

/// Statistics info gathered during scan check or files fixing
#[derive(Default, Debug)]
pub struct ScanStats {
    /// processed files number
    pub processed: usize,
    /// files number to be fixed in 'dry-mode' (or fixed in reality)
    pub fixed: usize,
    /// files skipped from processing (any reason)
    pub skipped: usize,
    /// files with errors
    pub errors: usize,
}

/// Gather fixes for every tag on the file. Which tag is fixed and new value.
pub struct Patch {
    pub field_name: &'static str,
    pub fixed_value: String,
}