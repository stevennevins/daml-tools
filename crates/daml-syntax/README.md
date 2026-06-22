# daml-syntax

`daml-syntax` is the shared parsed-source surface for Daml tools in this
workspace. It owns source presentation around `daml-parser`: diagnostics, line
mapping, UTF-16 offsets, token/trivia access, and parser span conversion.

```rust
let source = "module M where\nfoo : Int\nfoo = 1\n";
let file = daml_syntax::SourceFile::parse(source);

assert!(file.diagnostics().is_empty());
assert_eq!(file.module().name, "M");
assert_eq!(file.line_index().line_col(0.into()).line, 1);
```
