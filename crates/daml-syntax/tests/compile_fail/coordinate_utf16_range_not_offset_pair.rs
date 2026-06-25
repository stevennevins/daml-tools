use daml_syntax::{LineIndex, TextRange, Utf16Offset};

fn main() {
    let index = LineIndex::new("abc");
    let range = index.utf16_range(TextRange::new(0.into(), 1.into()));
    let _: (Utf16Offset, Utf16Offset) = range;
}
