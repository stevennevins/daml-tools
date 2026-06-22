//! daml-lint as a library.
//!
//! The binary (`src/main.rs`) is a thin CLI over these modules. The parser
//! pipeline — `lexer` → `layout` → `parse` over the AST types — is consumed
//! through the shared [`daml_syntax`] seam. This crate lowers that AST into a
//! rule-facing IR ([`ir`], via [`parser`]) and runs [`detectors`] over it.
//! Start at [`parser::parse_daml_with_diagnostics`].
//! The IR is public so custom rules and library callers can inspect it; parser
//! lowering is the supported construction path. This crate is pre-1.0, so
//! breaking public API changes use 0.x minor bumps and patch releases should
//! stay compatible.
//!
//! # Example
//!
//! ```
//! use std::path::Path;
//!
//! let source = "\
//! module M where
//!
//! template T
//!   with
//!     owner : Party
//!   where
//!     signatory owner
//! ";
//!
//! let (module, diagnostics) =
//!     daml_lint::parser::parse_daml_with_diagnostics(source, Path::new("M.daml"));
//!
//! assert!(diagnostics.is_empty());
//! assert_eq!(module.name, "M");
//! assert_eq!(module.templates.len(), 1);
//! ```

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
