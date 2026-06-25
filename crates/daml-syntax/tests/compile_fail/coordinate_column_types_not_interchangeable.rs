use daml_syntax::{ByteColumn, CharColumn};

fn main() {
    let char_col = CharColumn::new(4);
    let _: ByteColumn = char_col;
}
