---
description: How the TypeScript authoring contract and QuickJS runtime contract fit together for daml-lint custom rules.
---

# daml-lint rule authoring model

`daml-lint` custom rules have two related contracts: a TypeScript authoring
contract and a JavaScript runtime contract.

The authoring contract exists so rule authors can write against named IR types
such as `Template`, `Choice`, `Statement`, and `Expr`. TypeScript checks that a
rule object has valid metadata and visitor function shapes before the rule is
bundled.

The runtime contract exists because `daml-lint` embeds QuickJS. It loads a
single JavaScript file, evaluates it, reads top-level metadata constants, and
calls top-level visitor `function` declarations for each parsed Daml module.
QuickJS is intentionally isolated from Node APIs, so runtime imports,
filesystem access, and network access are unavailable.

## Why bundle TypeScript

Bundling turns the authoring project into one JavaScript artifact. Type-only
imports disappear, and any real helper imports are included in the output. That
keeps `daml-lint --rules dist/rule.js` independent of the author's source tree
and package manager.

This is why the examples use esbuild for emission and `tsc --noEmit` for type
checking. Esbuild proves the rule can be bundled, while TypeScript proves the
rule matches the declared contract.

## Why `__daml_lint_rule` is not enough

The `globalThis.__daml_lint_rule` object gives TypeScript one place to validate
the rule shape. The current loader does not discover visitors solely from that
object. It still evaluates `NAME`, `SEVERITY`, optional `DESCRIPTION`, and
visitor functions by their top-level names.

That split is deliberate for now. It lets TypeScript examples become safer
without changing the runtime loader in the same step. A future loader can make
`__daml_lint_rule` the runtime contract, but that would be a separate runtime
change with compatibility and error-message tests.

## Built-ins and external rules

Built-in rules live in `crates/daml-lint/rules`. They may import internal
helpers because the build bundles those helpers into the generated JavaScript
embedded in the Rust crate.

External rule authors should import only from
`@daml-tools/lint-plugin`. Internal helpers are not a public interface yet.
Keeping the public package to types and templates gives authors a stable
boundary without promising helper behavior before it has versioning
expectations.
