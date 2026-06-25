use daml_syntax::{LineIndex, TextSize};

fn main() {
    let index = LineIndex::new("abc");
    let _: TextSize = index.source_len_bytes();
}
