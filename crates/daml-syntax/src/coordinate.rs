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

/// 0-based half-open range in UTF-16 code units.
///
/// Use this when interoperating with JavaScript-style string ranges. The named
/// endpoints avoid confusing UTF-16 offsets with byte ranges or line/column
/// pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Utf16Range {
    start: Utf16Offset,
    end: Utf16Offset,
}

impl Utf16Range {
    /// Creates a UTF-16 range from inclusive start and exclusive end offsets.
    #[must_use]
    pub const fn new(start: Utf16Offset, end: Utf16Offset) -> Self {
        Self { start, end }
    }

    /// Inclusive 0-based start offset in UTF-16 code units.
    #[must_use]
    pub const fn start(self) -> Utf16Offset {
        self.start
    }

    /// Exclusive 0-based end offset in UTF-16 code units.
    #[must_use]
    pub const fn end(self) -> Utf16Offset {
        self.end
    }
}

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
