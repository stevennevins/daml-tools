//! Integration tests for coordinate newtypes through the public `daml-syntax` API.

#![allow(clippy::unwrap_used)]

use daml_syntax::{
    ByteColumn, ByteOffset, CharColumn, Coordinate, LineNumber, TextSize, Utf16Offset, Utf16Range,
};

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
fn one_based_coordinates_support_typed_try_from() {
    assert_eq!(LineNumber::try_from(3), Ok(LineNumber::new(3)));

    let err = ByteColumn::try_from(0).unwrap_err();
    assert_eq!(err.value(), 0);
    assert_eq!(err.to_string(), "1-based coordinate value must be non-zero");
}

#[test]
fn coordinate_newtypes_expose_distinct_values() {
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
    let utf16_range = Utf16Range::new(Utf16Offset::new(1), Utf16Offset::new(3));
    assert_eq!(utf16_range.start(), Utf16Offset::new(1));
    assert_eq!(utf16_range.end(), Utf16Offset::new(3));
}

#[test]
fn byte_offset_converts_to_text_size() {
    let offset = ByteOffset::new(5);
    assert_eq!(TextSize::try_from(offset).unwrap(), TextSize::from(5));
    assert_eq!(ByteOffset::from(TextSize::from(5)), ByteOffset::new(5));
}

#[test]
fn coordinate_column_types_are_not_interchangeable() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/coordinate_*.rs");
}
