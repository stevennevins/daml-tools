# AST-based types

Status: implemented for the breaking custom-rule surface.

`daml-parser::ast::Type` is the type source of truth. Type nodes are parsed from
the token stream, carry byte spans, and replace the old duplicate type-string
fields on parser declarations. Consumers that need display text should slice the
original source by the type node span.

`daml-lint` uses two type views:

- `TypeNode` is serialized to custom rules. It preserves constructors,
  applications, lists, tuples, functions, variables, constraints, and source
  ranges. Its `span.start`/`span.end` fields are JavaScript string offsets for
  `module.source.slice(...)`; `span.byte_start`/`span.byte_end` preserve the
  parser's UTF-8 byte-span basis.
- `DamlType` is an internal coarse classifier for built-in Rust detectors. It is
  not serialized in the custom-rule contract.

The removed string-reparse path and duplicate aliases include parser
`FieldDecl` type text, choice return type text, key type text, function type
signature text, and interface method type text. The custom-rule contract exposes
`TypeNode` fields instead: `field.type_`, `choice.return_type`,
`template.key_type`, `function.type_signature`, and `interface.methods[].type_`.
