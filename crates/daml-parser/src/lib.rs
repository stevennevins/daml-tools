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
//! Start at [`parse::parse_module`] for tolerant parsing, or
//! [`parse::parse_module_strict`] when any diagnostic should fail the caller.
//! For byte-faithful reconstruction from the parse tree, see
//! [`ast_span::render_from_ast`] and [`lexer::render_lossless`].
//!
//! ## API posture
//!
//! This crate is pre-1.0. Its public AST/IR-like types are intentionally direct
//! data shapes that tools consume directly; additions or shape changes are
//! SemVer-relevant and should be treated as breaking contract changes.
//! Prefer adding helpers only when they can be used without changing these
//! shapes, and use `#[non_exhaustive]` enums/variants for forward-safe
//! extension. New enums introduced here should be documented as `non_exhaustive`
//! when future variants are expected (for example
//! [`SectionSide`](crate::ast::SectionSide)).
//!
//! Parser-created trees are the supported construction path.
//!
//! # Example
//!
//! ```rust
//! let result =
//!     daml_parser::parse::parse_module("module M where\nfoo : Int\nfoo = 1\n");
//!
//! assert!(result.diagnostics.is_empty());
//! assert_eq!(result.module.name, "M");
//! ```

/// Lossless AST node types produced by the parser.
pub mod ast;
/// AST byte-span losslessness oracle (`render_from_ast`): reconstruct source
/// from the parse tree to prove the tree lost nothing.
pub mod ast_span;
/// Indentation-sensitive layout resolver.
///
/// Inserts virtual braces and semicolons for the parser.
pub mod layout;
/// Lexer and token/trivia types for Daml source text.
pub mod lexer;
/// Recursive-descent parser entry points and diagnostics.
pub mod parse;
