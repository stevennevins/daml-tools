use daml_lint::detector::FindingLocation;
use daml_syntax::{ByteOffset, CharColumn, LineNumber, Utf16Offset};

fn takes_line(_: LineNumber) {}
fn takes_column(_: CharColumn) {}
fn takes_utf16_offset(_: Utf16Offset) {}
fn takes_byte_offset(_: ByteOffset) {}

fn main() {
    takes_line(CharColumn::new(3));
    takes_column(LineNumber::new(3));
    takes_utf16_offset(ByteOffset::new(12));
    takes_byte_offset(Utf16Offset::new(12));

    let _bad_location = FindingLocation::new(
        "Test.daml",
        CharColumn::new(7),
        LineNumber::new(4),
    );
}
