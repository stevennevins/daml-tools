//! daml-lint as a library.
//!
//! The binary (`src/main.rs`) is a thin CLI over these modules. The parser
//! pipeline — `lexer` → `layout` → `parse` over the AST types — lives in the
//! separate [`daml_parser`] crate. This crate lowers that AST into a
//! rule-facing IR ([`ir`], via [`parser`]) and runs [`detectors`] over it.
//! Start at [`parser::parse_daml_with_diagnostics`].
//! The IR is public so custom rules and library callers can inspect it; parser
//! lowering is the supported construction path.
//!
//! ## API posture
//!
//! This crate is pre-1.0. Its public IR is also intentionally data-oriented;
//! adding/removing fields, changing enum variants, or altering shapes is
//! SemVer-relevant. Use constructors in the same crate for creation and treat
//! these as versioned contracts with downstream rules/detectors.
//! Matching contract for rule-facing IR:
//! `TypeNode`, `LiteralKind`, `Expr`, `Consuming`, `Statement`, and
//! `ImportStyle` are intentionally `#[non_exhaustive]`.
//! Read-mostly IR/report DTO structs (`DamlModule`, `Template`, `Finding`, and
//! related nodes) are also `#[non_exhaustive]` so fields can evolve in 0.x
//! without breaking downstream field reads.
//! Downstream code should include wildcard arms when matching any of these
//! enums; adding variants is a compatible evolution for new Daml syntax and
//! recovery paths. Construct IR through [`parser::parse_daml_with_diagnostics`]
//! or documented constructors such as [`detector::Finding::new`].
//!
//! Parse diagnostics use [`parser::ParseDiagnosticCategory`] (not the parser
//! crate's internal category enum) and [`parser::ParseResult`] (`module` +
//! `diagnostics`) as the supported lowering entry point. For severity thresholds
//! and report ordering, use [`detector::Severity::rank`] and
//! [`detector::Severity::meets_or_exceeds`]; `Severity` does not implement
//! `Ord` because declaration order does not match risk rank.
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
//! let parse_result =
//!     daml_lint::parser::parse_daml_with_diagnostics(source, Path::new("M.daml"));
//! let module = parse_result.module;
//! let diagnostics = parse_result.diagnostics;
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
