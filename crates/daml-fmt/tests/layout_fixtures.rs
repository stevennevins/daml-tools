//! Black-box layout formatting fixtures exercised through the public
//! [`format_source`] API.

#![allow(clippy::unwrap_used)]

use daml_fmt::format_source;
use std::path::PathBuf;

fn layout_cases_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/layout-cases")
}

fn assert_layout_case(case: &str) {
    let case_dir = layout_cases_dir().join(case);
    let input_path = case_dir.join("input.daml");
    let expected_path = case_dir.join("expected.daml");
    assert!(
        input_path.is_file(),
        "missing layout input: {}",
        input_path.display()
    );
    assert!(
        expected_path.is_file(),
        "missing layout expected: {}",
        expected_path.display()
    );
    let src = std::fs::read_to_string(input_path).unwrap();
    let expected = std::fs::read_to_string(expected_path).unwrap();
    let out = format_source(&src);
    assert_eq!(out, expected, "layout case {case}");
    assert_eq!(
        format_source(&out),
        out,
        "layout case {case} not idempotent"
    );
}

#[test]
fn do_body_reindented_to_anchor_plus_two() {
    // do at col 0; body stmt at col 6 -> should move to col 2.
    let src = "f = do\n      pure ()\n";
    let out = format_source(src);
    assert_eq!(out, "f = do\n  pure ()\n");
}

#[test]
fn source_range_expectation_files_stay_byte_exact() {
    let src = "module M where\n-- @ WARN range=3:8-3:9; x\nfoo : Int\nfoo = 1\n";
    assert_eq!(format_source(src), src);
}

#[test]
fn source_location_query_files_stay_byte_exact() {
    let src = "-- @QUERY-LF .location.range | (.start_line == 8 and .start_col == 9)\n\n\nmodule Locations where\nfoo : Int\nfoo = 1\n";
    assert_eq!(format_source(src), src);
}

#[test]
fn idempotent_on_reindent() {
    let src = "f = do\n      pure ()\n";
    let once = format_source(src);
    let twice = format_source(&once);
    assert_eq!(once, twice);
}

#[test]
fn leading_comment_not_measured_or_moved() {
    // The first body line is a col-0 comment; the real stmt is at col 6.
    // The comment must stay at col 0; the stmt moves to col 2.
    let src = "f = do\n-- note\n      pure ()\n";
    let out = format_source(src);
    assert_eq!(out, "f = do\n-- note\n  pure ()\n");
    assert_eq!(format_source(&out), out); // idempotent
}

#[test]
fn inline_do_left_alone() {
    let src = "f = do pure ()\n";
    assert_eq!(format_source(src), src);
}

#[test]
fn tab_indented_body_left_verbatim() {
    // Tabs in indentation must never get spaces prepended in front of them.
    let src = "f = do\n\t\tpure ()\n";
    assert_eq!(format_source(src), src);
    assert_eq!(format_source(&format_source(src)), format_source(src));
}

#[test]
fn do_block_starting_with_let_is_reindented() {
    // A `do` whose first statement is a `let` is no longer verbatim. The
    // whole block shifts by ONE uniform delta to land the first stmt at
    // do_col + 2, so the `let` line, its continuation binding, and the
    // following statement all move together — the bindings stay aligned
    // (x and y both end at col 6) without a separate let-internal rule.
    let src = "f = do\n      let x = 1\n          y = 2\n      pure (x + y)\n";
    let out = format_source(src);
    assert_eq!(out, "f = do\n  let x = 1\n      y = 2\n  pure (x + y)\n");
    assert_eq!(format_source(&out), out); // idempotent
}

#[test]
fn do_block_with_try_is_reindented() {
    // A do-block containing try/catch is now owned by the do and try passes.
    let src = "f = do\n      _ <- try foo catch _ -> bar\n      pure ()\n";
    let out = format_source(src);
    assert_eq!(out, "f = do\n  _ <- try foo catch _ -> bar\n  pure ()\n");
    assert_eq!(format_source(&out), out);
}

#[test]
fn if_then_else_reindented_to_if_col_plus_two() {
    // `if` at col 2; then/else lines move to col 4 (if_col + 2).
    let src = "f x =\n  if x > 0\n      then 1\n      else 2\n";
    let out = format_source(src);
    assert_eq!(out, "f x =\n  if x > 0\n    then 1\n    else 2\n");
    assert_eq!(format_source(&out), out); // idempotent
}

#[test]
fn if_then_else_already_aligned_is_a_fixpoint() {
    let src = "f x =\n  if x > 0\n    then 1\n    else 2\n";
    assert_eq!(format_source(src), src);
}

#[test]
fn single_line_if_is_expanded() {
    let src = "g x = if x then 1 else 2\n";
    assert_eq!(
        format_source(src),
        "g x =\n  if x\n    then 1\n    else 2\n"
    );
}

#[test]
fn do_then_if_passes_reach_a_single_call_fixpoint() {
    // Regression: a do-block as the `then`-branch where `then`/`else` are at
    // different columns. In pass 1 the do-pass's body shift collides with
    // the not-yet-moved `else` (offside VSemi) so its gate rejects; the
    // if-pass then moves `else`, removing the collision. The structural
    // passes must iterate to a fixpoint so a SINGLE format call is already
    // idempotent — format(format(x)) == format(x).
    assert_layout_case("do_then_if_fixpoint");
}

#[test]
fn if_then_else_multiline_branch_rides_uniform_shift() {
    // A then-branch spanning extra lines shifts by ONE uniform delta, so the
    // branch's own indentation structure is preserved (8->6, 10->8).
    assert_layout_case("if_then_else_multiline_branch");
}

#[test]
fn case_alts_reindented_to_case_indent_plus_two() {
    // case-line indent 0; alts at col 6 move to col 2.
    let src = "f x = case x of\n      None -> 1\n      Some y -> y\n";
    let out = format_source(src);
    assert_eq!(out, "f x = case x of\n  None -> 1\n  Some y -> y\n");
    assert_eq!(format_source(&out), out); // idempotent
}

#[test]
fn case_alts_already_aligned_is_a_fixpoint() {
    let src = "f x = case x of\n  None -> 1\n  Some y -> y\n";
    assert_eq!(format_source(src), src);
}

#[test]
fn inline_case_alts_are_expanded() {
    let src = "f x = case x of None -> 1; Some y -> y\n";
    assert_eq!(
        format_source(src),
        "f x = case x of\n  None -> 1\n  Some y -> y\n"
    );
}

#[test]
fn nested_case_rides_outer_shift() {
    // Inner case (an alt body) rides the outer alt block's uniform shift; the
    // inner alts stay aligned relative to their own `case`.
    assert_layout_case("nested_case_outer_shift");
}

#[test]
fn letin_bindings_reindented_to_let_indent_plus_two() {
    // `let` on its own line at col 2; bindings at col 6 move to col 4; `in`
    // is left at col 2.
    let src = "f =\n  let\n      x = 1\n      y = 2\n  in x + y\n";
    let out = format_source(src);
    assert_eq!(out, "f =\n  let\n    x = 1\n    y = 2\n  in x + y\n");
    assert_eq!(format_source(&out), out); // idempotent
}

#[test]
fn letin_already_aligned_is_a_fixpoint() {
    let src = "f =\n  let\n    x = 1\n    y = 2\n  in x + y\n";
    assert_eq!(format_source(src), src);
}

#[test]
fn inline_letin_is_expanded() {
    let src = "f = let x = 1 in x\n";
    assert_eq!(format_source(src), "f =\n  let\n    x = 1\n  in x\n");
}

#[test]
fn con_with_fields_reindented_to_indent_plus_two() {
    // `create Asset with` at line indent 0; fields at col 6 move to col 2.
    let src = "f = create Asset with\n      issuer = a\n      owner = b\n";
    let out = format_source(src);
    assert_eq!(out, "f = create Asset with\n  issuer = a\n  owner = b\n");
    assert_eq!(format_source(&out), out); // idempotent
}

#[test]
fn record_update_fields_are_reindented() {
    // base is an expression (`this`), not a bare constructor: an update.
    let src = "f this p = this with\n      owner = p\n";
    let out = format_source(src);
    assert_eq!(out, "f this p = this with\n  owner = p\n");
    assert_eq!(format_source(&out), out);
}

#[test]
fn split_with_on_own_line_stays_verbatim() {
    // `with` is on its own line, not the `Con` line: reindenting the fields
    // to the Con line's indent + 2 would put them left of `with`, so the
    // rule leaves it verbatim.
    let src = "f = WithField\n    with\n        f1 = 10\n";
    assert_eq!(format_source(src), src);
}

#[test]
fn inline_con_with_fields_are_expanded() {
    let src = "f = Asset with issuer = a; owner = b\n";
    assert_eq!(
        format_source(src),
        "f = Asset with\n  issuer = a\n  owner = b\n"
    );
}

#[test]
fn con_with_before_where_keeps_fields_inside_expression() {
    assert_layout_case("con_with_before_where");
}

#[test]
fn template_four_space_ladder_canonicalized_to_two() {
    // The case the uniform shift could NOT fix: a 4-space ladder. The
    // structured reindent uses different deltas for keywords (-> +2) and
    // fields/decls (-> +4), so it becomes the canonical 2-space ladder, and
    // the choice's internal 2-space ladder rides the decl-block shift.
    assert_layout_case("template_four_space_ladder");
}

#[test]
fn interface_body_canonicalized_to_two() {
    // `interface X where` has `where` inline, so the body (viewtype +
    // methods + choices) sits at head + 2, and a choice's internals ride to
    // head + 4.
    assert_layout_case("interface_body_canonicalized");
}

#[test]
fn inline_with_template_keeps_fields_at_head_plus_four() {
    // Regression: `template T with` (with inline on the head line) is still
    // a 2-level ladder — fields at head + 4, NOT head + 2, because the
    // `where` at + 2 must close the with-block. (Sending them to + 2 made
    // the SDK reject the output.)
    let src = "template T with\n    p: Party\n  where\n    signatory p\n";
    assert_eq!(format_source(src), src);
}

#[test]
fn canonical_template_is_a_fixpoint() {
    let src = "template Coin\n  with\n    issuer: Party\n  where\n    signatory issuer\n";
    assert_eq!(format_source(src), src);
}

#[test]
fn under_indented_template_body_canonicalized() {
    // where-decls at the `where` column (2) move to template_indent + 4 = 4.
    let src = "template Coin\n  with\n    issuer: Party\n  where\n  signatory issuer\n";
    let out = format_source(src);
    assert_eq!(
        out,
        "template Coin\n  with\n    issuer: Party\n  where\n    signatory issuer\n"
    );
    assert_eq!(format_source(&out), out); // idempotent
}

#[test]
fn mid_line_let_is_left_verbatim() {
    // A `let` that does not start its line: the `in` stays at the keyword
    // column while the bindings would anchor on the (smaller) line indent,
    // which mismatches — so the rule leaves it alone rather than dedent the
    // bindings left of `let`/`in`.
    let src = "f x = let\n        a = 1\n        b = 2\n      in a + b\n";
    assert_eq!(format_source(src), src);
}

#[test]
fn choice_internal_ladder_is_canonicalized() {
    assert_layout_case("choice_internal_ladder");
}

#[test]
fn choice_keyword_scan_ignores_identifier_fragments() {
    assert_layout_case("choice_keyword_scan_identifier");
}

#[test]
fn type_def_ladders_are_canonicalized() {
    assert_layout_case("type_def_ladders");
}

#[test]
fn data_record_with_ladder_keeps_with_above_fields() {
    assert_layout_case("data_record_with_ladder");
}

#[test]
fn inline_data_record_with_braces_keeps_body_column() {
    let src = "data Data = Data with\n  { dummy : ()\n  , srcLoc : SrcLoc\n  }\n";
    assert_eq!(format_source(src), src);
}

#[test]
fn class_where_body_with_comments_keeps_body_indent() {
    assert_layout_case("class_where_body_comments");
}

#[test]
fn class_where_body_with_indented_pragma_keeps_pragma_indent() {
    assert_layout_case("class_where_body_indented_pragma");
}

#[test]
fn guards_and_where_bindings_are_canonicalized() {
    assert_layout_case("guards_and_where_bindings");
}

#[test]
fn multiline_try_catch_is_canonicalized() {
    assert_layout_case("multiline_try_catch");
}

#[test]
fn explicit_list_continuations_are_canonicalized() {
    let src = "x = [ 1\n      , 2\n      , 3 ]\n";
    let out = format_source(src);
    let want = "x = [ 1\n  , 2\n  , 3 ]\n";
    assert_eq!(out, want);
    assert_eq!(format_source(&out), out);
}

#[test]
fn module_and_import_continuations_are_canonicalized() {
    assert_layout_case("module_import_continuations");
}

#[test]
fn duplicate_space_after_colon_collapsed() {
    // The formatter owns type-annotation colon spacing, so `x:  T` must
    // canonicalize to `x: T` symmetrically with `x : T` -> `x: T`.
    let src = "module M where\nfoo:  Int -> Int\nfoo x = x\n";
    let out = format_source(src);
    assert_eq!(out, "module M where\nfoo: Int -> Int\nfoo x = x\n");
    assert_eq!(format_source(&out), out); // idempotent
}

#[test]
fn space_around_colon_canonicalized_both_sides() {
    let src = "module M where\nfoo  :  Int\nfoo = 1\n";
    assert_eq!(format_source(src), "module M where\nfoo: Int\nfoo = 1\n");
}

#[test]
fn after_colon_collapse_skips_braces_and_parens() {
    // At brace/paren depth the convention keeps the surrounding space, so
    // the after-colon collapse must NOT fire (same gate as before-colon).
    let braced = "module M where\nx = { a :  Int }\n";
    assert_eq!(format_source(braced), braced);
    let parened = "module M where\nf (n :  Int) = n\n";
    assert_eq!(format_source(parened), parened);
}

#[test]
fn crlf_final_newline_not_mixed() {
    // A CRLF file must not end up with a lone LF on its last line.
    let src = "module M where\r\nx = 1   \r\n";
    let out = format_source(src);
    assert!(out.ends_with("\r\n"), "got: {out:?}");
    assert!(!out.ends_with("\n\n"));
    assert_eq!(format_source(&out), out); // idempotent
}
