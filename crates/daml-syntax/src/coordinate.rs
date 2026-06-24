//! Domain-specific source coordinates.
//!
//! Byte offsets, line numbers, and column positions use distinct newtypes so
//! they cannot be passed to the wrong API by accident. Conversions to raw
//! `usize` are explicit via [`Coordinate::get`].

use std::fmt;
use std::num::NonZeroUsize;
use text_size::TextSize;

macro_rules! define_zero_based_coordinate {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(usize);

        impl $name {
            /// Creates a 0-based coordinate.
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

macro_rules! define_one_based_coordinate {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(NonZeroUsize);

        impl $name {
            /// Creates a 1-based coordinate when `value` is non-zero.
            #[must_use]
            pub const fn try_new(value: usize) -> Option<Self> {
                match NonZeroUsize::new(value) {
                    Some(v) => Some(Self(v)),
                    None => None,
                }
            }

            /// Creates a 1-based coordinate.
            ///
            /// # Panics
            ///
            /// Panics when `value` is zero.
            #[must_use]
            pub const fn new(value: usize) -> Self {
                match NonZeroUsize::new(value) {
                    Some(v) => Self(v),
                    None => panic!("1-based coordinates must be non-zero"),
                }
            }
        }

        impl Coordinate for $name {
            fn get(self) -> usize {
                self.0.get()
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
    /// Returns the underlying numeric value for this coordinate.
    fn get(self) -> usize;
}

define_one_based_coordinate!(
    /// 1-based line number in a source file.
    ///
    /// Zero is invalid; use [`LineNumber::try_new`] to reject it explicitly.
    LineNumber
);
define_one_based_coordinate!(
    /// 1-based byte column offset from the start of a line.
    ///
    /// Zero is invalid; use [`ByteColumn::try_new`] to reject it explicitly.
    ByteColumn
);
define_one_based_coordinate!(
    /// 1-based Unicode scalar column offset from the start of a line.
    ///
    /// Zero is invalid; use [`CharColumn::try_new`] to reject it explicitly.
    CharColumn
);
define_zero_based_coordinate!(
    /// 0-based UTF-8 byte offset into source text.
    ///
    /// Prefer [`TextSize`] for [`text_size::TextRange`] construction; this
    /// newtype marks raw parser byte offsets at crate boundaries.
    ByteOffset
);
define_zero_based_coordinate!(
    /// 0-based UTF-16 code-unit offset into source text.
    Utf16Offset
);

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
    /// 1-based line number.
    pub line: LineNumber,
    /// 1-based byte column on that line.
    pub column: ByteColumn,
}

/// 1-based line and character-column position returned by
/// [`super::LineIndex::char_line_col`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharLineCol {
    /// 1-based line number.
    pub line: LineNumber,
    /// 1-based Unicode scalar column on that line.
    pub column: CharColumn,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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
    fn one_based_coordinates_reject_zero() {
        assert!(LineNumber::try_new(0).is_none());
        assert!(ByteColumn::try_new(0).is_none());
        assert!(CharColumn::try_new(0).is_none());
    }

    #[test]
    fn one_based_coordinates_accept_one() {
        assert_eq!(LineNumber::try_new(1), Some(LineNumber::new(1)));
        assert_eq!(ByteColumn::try_new(1), Some(ByteColumn::new(1)));
        assert_eq!(CharColumn::try_new(1), Some(CharColumn::new(1)));
    }

    #[test]
    fn byte_offset_converts_to_text_size() {
        let offset = ByteOffset::new(5);
        assert_eq!(TextSize::try_from(offset).unwrap(), TextSize::from(5));
        assert_eq!(ByteOffset::from(TextSize::from(5)), ByteOffset::new(5));
    }
}
