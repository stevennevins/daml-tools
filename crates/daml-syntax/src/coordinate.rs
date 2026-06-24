//! Domain-specific source coordinates.
//!
//! Byte offsets, line numbers, and column positions use distinct newtypes so
//! they cannot be passed to the wrong API by accident. Conversions to raw
//! `usize` are explicit via [`Coordinate::get`].

use std::fmt;
use text_size::TextSize;

macro_rules! define_coordinate {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(usize);

        impl $name {
            #[must_use]
            pub const fn new(value: usize) -> Self {
                Self(value)
            }
        }

        impl Coordinate for $name {
            fn get(self) -> usize {
                self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

/// Explicit conversion from a typed coordinate to a raw `usize`.
pub trait Coordinate {
    fn get(self) -> usize;
}

define_coordinate!(/// 1-based line number in a source file.
    LineNumber);
define_coordinate!(/// 1-based byte column offset from the start of a line.
    ByteColumn);
define_coordinate!(/// 1-based Unicode scalar column offset from the start of a line.
    CharColumn);
define_coordinate!(/// 0-based UTF-8 byte offset into source text.
    ///
    /// Prefer [`TextSize`] for [`text_size::TextRange`] construction; this
    /// newtype marks raw parser byte offsets at crate boundaries.
    ByteOffset);
define_coordinate!(/// 0-based UTF-16 code-unit offset into source text.
    Utf16Offset);

impl From<TextSize> for ByteOffset {
    fn from(offset: TextSize) -> Self {
        Self::new(usize::from(offset))
    }
}

impl TryFrom<ByteOffset> for TextSize {
    type Error = std::num::TryFromIntError;

    fn try_from(offset: ByteOffset) -> Result<Self, Self::Error> {
        Self::try_from(offset.get())
    }
}

/// 1-based line and byte-column position returned by [`super::LineIndex::line_col`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteLineCol {
    pub line: LineNumber,
    pub column: ByteColumn,
}

/// 1-based line and character-column position returned by
/// [`super::LineIndex::char_line_col`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharLineCol {
    pub line: LineNumber,
    pub column: CharColumn,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_distinct_coordinate_types() {
        let line = LineNumber::new(1);
        let byte_col = ByteColumn::new(4);
        let char_col = CharColumn::new(4);
        let byte = ByteOffset::new(10);
        let utf16 = Utf16Offset::new(10);

        assert_eq!(line.get(), 1);
        assert_eq!(byte_col.get(), char_col.get());
        assert_ne!(byte.get(), line.get());

        let _: ByteColumn = byte_col;
        let _: CharColumn = char_col;
        let _: ByteOffset = byte;
        let _: Utf16Offset = utf16;
        // `let _mixed: ByteColumn = char_col;` must not compile.
    }

    #[test]
    fn coordinate_newtypes_are_not_interchangeable() {
        assert_distinct_coordinate_types();
    }

    #[test]
    fn byte_offset_converts_to_text_size() {
        let offset = ByteOffset::new(5);
        assert_eq!(TextSize::try_from(offset).unwrap(), TextSize::from(5));
        assert_eq!(ByteOffset::from(TextSize::from(5)), ByteOffset::new(5));
    }
}
