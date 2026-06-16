//! daml-parser: a **lossless** lexer, layout resolver, and parser for the Daml
//! smart-contract language.
//!
//! This is the shared foundation under both `daml-lint` and `daml-fmt`. The
//! pipeline is `lexer` → `layout` → `parse` over the [`ast`] types. The tree is
//! lossless: the lexer records every comment and whitespace run as *trivia*
//! (see [`lexer::lex_with_trivia`]), so a consumer can reconstruct the original
//! bytes exactly. The linter ignores trivia and reads meaning; the formatter
//! keeps trivia and re-prints layout. One tree, two readers.
//!
//! Start at [`parse::parse_module`]. For byte-faithful reconstruction from the
//! parse tree, see [`ast_span::render_from_ast`] and [`lexer::render_lossless`].
//!
//! # Example
//!
//! ```
//! let (module, diagnostics) =
//!     daml_parser::parse::parse_module("module M where\nfoo : Int\nfoo = 1\n");
//!
//! assert!(diagnostics.is_empty());
//! assert_eq!(module.name, "M");
//! ```

pub mod ast;
/// AST byte-span losslessness oracle (`render_from_ast`): reconstruct source
/// from the parse tree to prove the tree lost nothing.
pub mod ast_span;
pub mod layout;
pub mod lexer;
pub mod parse;

#[cfg(test)]
mod data_tests;
#[cfg(test)]
mod diag_tests;
#[cfg(test)]
mod projection_tests;
#[cfg(test)]
mod span_tests;
