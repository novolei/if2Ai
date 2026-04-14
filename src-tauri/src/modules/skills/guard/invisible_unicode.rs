#![allow(unused)]

//! Invisible Unicode detection — zero-width and bidirectional characters.
//!
//! Ported from Hermes `tools/skills_guard.py` lines 505-523.

use std::fmt;

/// Zero-width and invisible unicode characters used for injection attacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InvisibleUnicodeChar {
    // Zero-width characters
    ZeroWidthSpace,        // \u200b
    ZeroWidthNonJoiner,    // \u200c
    ZeroWidthJoiner,       // \u200d
    WordJoiner,            // \u2060
    InvisibleTimes,        // \u2062
    InvisibleSeparator,    // \u2063
    InvisiblePlus,         // \u2064
    ZeroWidthNoBreakSpace, // \uFEFF (BOM)
    // Bidirectional characters
    LeftToRightEmbedding,     // \u202a
    RightToLeftEmbedding,     // \u202b
    PopDirectionalFormatting, // \u202c
    LeftToRightOverride,      // \u202d
    RightToLeftOverride,      // \u202e
    LeftToRightIsolate,       // \u2066
    RightToLeftIsolate,       // \u2067
    FirstStrongIsolate,       // \u2068
    PopDirectionalIsolate,    // \u2069
}

impl InvisibleUnicodeChar {
    /// Returns the Unicode character for this variant.
    pub fn as_char(&self) -> char {
        match self {
            Self::ZeroWidthSpace => '\u{200b}',
            Self::ZeroWidthNonJoiner => '\u{200c}',
            Self::ZeroWidthJoiner => '\u{200d}',
            Self::WordJoiner => '\u{2060}',
            Self::InvisibleTimes => '\u{2062}',
            Self::InvisibleSeparator => '\u{2063}',
            Self::InvisiblePlus => '\u{2064}',
            Self::ZeroWidthNoBreakSpace => '\u{FEFF}',
            Self::LeftToRightEmbedding => '\u{202a}',
            Self::RightToLeftEmbedding => '\u{202b}',
            Self::PopDirectionalFormatting => '\u{202c}',
            Self::LeftToRightOverride => '\u{202d}',
            Self::RightToLeftOverride => '\u{202e}',
            Self::LeftToRightIsolate => '\u{2066}',
            Self::RightToLeftIsolate => '\u{2067}',
            Self::FirstStrongIsolate => '\u{2068}',
            Self::PopDirectionalIsolate => '\u{2069}',
        }
    }

    /// Returns the Unicode code point as U+XXXX format.
    pub fn code_point(&self) -> String {
        format!("U+{:04X}", self.as_char() as u32)
    }

    /// Returns a human-readable name for this character.
    pub fn name(&self) -> &'static str {
        match self {
            Self::ZeroWidthSpace => "ZERO-WIDTH SPACE",
            Self::ZeroWidthNonJoiner => "ZERO-WIDTH NON-JOINER",
            Self::ZeroWidthJoiner => "ZERO-WIDTH JOINER",
            Self::WordJoiner => "WORD JOINER",
            Self::InvisibleTimes => "INVISIBLE TIMES",
            Self::InvisibleSeparator => "INVISIBLE SEPARATOR",
            Self::InvisiblePlus => "INVISIBLE PLUS",
            Self::ZeroWidthNoBreakSpace => "ZERO-WIDTH NO-BREAK SPACE (BOM)",
            Self::LeftToRightEmbedding => "LEFT-TO-RIGHT EMBEDDING",
            Self::RightToLeftEmbedding => "RIGHT-TO-LEFT EMBEDDING",
            Self::PopDirectionalFormatting => "POP DIRECTIONAL FORMATTING",
            Self::LeftToRightOverride => "LEFT-TO-RIGHT OVERRIDE",
            Self::RightToLeftOverride => "RIGHT-TO-LEFT OVERRIDE",
            Self::LeftToRightIsolate => "LEFT-TO-RIGHT ISOLATE",
            Self::RightToLeftIsolate => "RIGHT-TO-LEFT ISOLATE",
            Self::FirstStrongIsolate => "FIRST STRONG ISOLATE",
            Self::PopDirectionalIsolate => "POP DIRECTIONAL ISOLATE",
        }
    }
}

impl fmt::Display for InvisibleUnicodeChar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.code_point(), self.name())
    }
}

/// All invisible Unicode characters used for injection detection.
pub const INVISIBLE_UNICODE_CHARS: &[InvisibleUnicodeChar] = &[
    InvisibleUnicodeChar::ZeroWidthSpace,
    InvisibleUnicodeChar::ZeroWidthNonJoiner,
    InvisibleUnicodeChar::ZeroWidthJoiner,
    InvisibleUnicodeChar::WordJoiner,
    InvisibleUnicodeChar::InvisibleTimes,
    InvisibleUnicodeChar::InvisibleSeparator,
    InvisibleUnicodeChar::InvisiblePlus,
    InvisibleUnicodeChar::ZeroWidthNoBreakSpace,
    InvisibleUnicodeChar::LeftToRightEmbedding,
    InvisibleUnicodeChar::RightToLeftEmbedding,
    InvisibleUnicodeChar::PopDirectionalFormatting,
    InvisibleUnicodeChar::LeftToRightOverride,
    InvisibleUnicodeChar::RightToLeftOverride,
    InvisibleUnicodeChar::LeftToRightIsolate,
    InvisibleUnicodeChar::RightToLeftIsolate,
    InvisibleUnicodeChar::FirstStrongIsolate,
    InvisibleUnicodeChar::PopDirectionalIsolate,
];

/// Represents an invisible Unicode finding.
#[derive(Debug, Clone)]
pub struct InvisibleUnicodeFinding {
    /// The character that was found
    pub character: InvisibleUnicodeChar,
    /// The line number where it was found
    pub line: u32,
    /// The file where it was found
    pub file: String,
}

impl InvisibleUnicodeFinding {
    /// Returns the match text for display (e.g., "U+200B (ZERO-WIDTH SPACE)".
    pub fn match_text(&self) -> String {
        format!(
            "{} ({})",
            self.character.code_point(),
            self.character.name()
        )
    }
}

/// Scan a single line for invisible Unicode characters.
/// Returns all found invisible characters with their positions.
pub fn scan_line_for_invisible_unicode(
    line: &str,
    line_number: u32,
    file: &str,
) -> Vec<InvisibleUnicodeFinding> {
    let mut findings = Vec::new();
    let mut seen_chars = std::collections::HashSet::new();

    for ch in line.chars() {
        for inv_char in INVISIBLE_UNICODE_CHARS {
            if ch == inv_char.as_char() {
                // Only report one finding per character per line
                if seen_chars.insert(inv_char) {
                    findings.push(InvisibleUnicodeFinding {
                        character: *inv_char,
                        line: line_number,
                        file: file.to_string(),
                    });
                }
                break;
            }
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invisible_char_as_char() {
        assert_eq!(InvisibleUnicodeChar::ZeroWidthSpace.as_char(), '\u{200b}');
        assert_eq!(
            InvisibleUnicodeChar::ZeroWidthNoBreakSpace.as_char(),
            '\u{FEFF}'
        );
    }

    #[test]
    fn test_scan_line_with_zero_width_space() {
        let line = "Hello\u{200b}World";
        let findings = scan_line_for_invisible_unicode(line, 1, "test.md");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].character, InvisibleUnicodeChar::ZeroWidthSpace);
        assert_eq!(findings[0].line, 1);
        assert_eq!(findings[0].file, "test.md");
    }

    #[test]
    fn test_scan_line_with_bidirectional_override() {
        let line = "Hello\u{202e}World";
        let findings = scan_line_for_invisible_unicode(line, 5, "test.md");
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].character,
            InvisibleUnicodeChar::RightToLeftOverride
        );
    }

    #[test]
    fn test_scan_line_clean() {
        let line = "Hello, this is a normal line with no invisible characters!";
        let findings = scan_line_for_invisible_unicode(line, 1, "test.md");
        assert!(findings.is_empty());
    }

    #[test]
    fn test_multiple_invisible_chars_same_line() {
        let line = "Test\u{200b}\u{200c}Word";
        let findings = scan_line_for_invisible_unicode(line, 1, "test.md");
        // Should have both zero-width space and zero-width non-joiner
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn test_dedup_same_char_same_line() {
        let line = "Test\u{200b}\u{200b}\u{200b}Word";
        let findings = scan_line_for_invisible_unicode(line, 1, "test.md");
        // Should only report once per character per line
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn test_code_point_format() {
        assert_eq!(InvisibleUnicodeChar::ZeroWidthSpace.code_point(), "U+200B");
        assert_eq!(
            InvisibleUnicodeChar::ZeroWidthNoBreakSpace.code_point(),
            "U+FEFF"
        );
    }
}
