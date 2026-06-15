//! daml-lint as a library.
//!
//! The binary (`src/main.rs`) is a thin CLI over these modules. The parser
//! pipeline — `lexer` → `layout` → `parse` over the AST types — lives in the
//! separate [`daml_parser`] crate. This crate lowers that AST into a
//! rule-facing IR ([`ir`], via [`parser`]) and runs [`detectors`] over it.
//! Start at [`parser::parse_daml_with_diagnostics`].

pub mod detector;
pub mod detectors;
pub mod ir;
/// Lowering: `daml-parser`'s typed AST → rule-facing IR ([`ir`]).
pub mod parser;
pub mod reporter;

#[cfg(test)]
mod adversarial_tests;
#[cfg(test)]
mod corpus_tests;
