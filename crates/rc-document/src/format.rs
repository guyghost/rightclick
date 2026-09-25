//! Metadata and conversion for a document's on-disk format.

use crate::encoding::{self, Encoding};
use rc_text::Newline;

/// Encoding and newline convention detected for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileFormat {
    encoding: Encoding,
    newline: Newline,
}

impl FileFormat {
    /// Decode bytes and retain the detected encoding and first newline style.
    pub fn decode(bytes: &[u8]) -> (String, Self) {
        let encoding = encoding::detect(bytes);
        let text = encoding::decode(bytes, encoding);
        let format = Self {
            encoding,
            newline: Newline::detect(&text),
        };
        (text, format)
    }

    /// Choose UTF-8 and the first newline style present in new text.
    pub fn for_text(text: &str) -> Self {
        Self {
            encoding: Encoding::Utf8,
            newline: Newline::detect(text),
        }
    }

    pub fn encoding(self) -> Encoding {
        self.encoding
    }

    pub fn newline(self) -> Newline {
        self.newline
    }

    /// Normalize all separators to this file's style and encode the result.
    pub fn encode_text(self, text: &str) -> Vec<u8> {
        let target = match self.newline {
            Newline::None => Newline::Lf,
            style => style,
        };
        let normalized = rc_text::normalize_newlines(text, target);
        encoding::encode(&normalized, self.encoding)
    }
}
