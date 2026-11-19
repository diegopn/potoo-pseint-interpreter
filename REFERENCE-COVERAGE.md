# PSeInt reference coverage

This crate targets PSeInt version 20220506 with the `<predeterminado>` profile. It does not claim compatibility with other configurable profiles or every release-specific option. This matrix maps the selected reference profile to the implementation and existing regression tests; it does not represent differential validation against the original executable.

| Reference area | Implemented profile | Boundary or validation status |
| --- | --- | --- |
| Profile and syntax settings | Default keywords, declarations, operators, literals, comments, and program structure from the pinned 20220506 settings and bundled help | Other profile files and configuration choices are outside this crate's target. |
| Values and declarations | Scalar values, typed declarations, inference, conversions, arithmetic, comparison, and colloquial comparison operators | Unicode text positions count Unicode scalar values; combining sequences and supplementary characters can differ from legacy builds. |
| Arrays and subprograms | Indexed arrays, bounds checks, value/reference parameters, array passage, local variables, procedures, functions, and recursion | Active nesting is bounded at 64 as a runtime-safety policy. |
| Control flow | Conditional branches, selection, loops, and documented loop execution rules | Covered by tests/conformance.rs and tests/runtime.rs; the whole original configurable language is not claimed. |
| Input, output, and standard functions | Leer, Escribir, text/numeric helpers, math functions, deterministic seeded randomness, and host-mediated wait/clear operations | Seeded random draws use the Linux libc sequence across hosts; another original C runtime may produce different sequences. |
| CLI and diagnostics | UTF-8 source and input, ISO-8859-1 fallback, source coordinates, check-only mode, supplied input values, and seeded runs | CLI prompts/status messages are runner behavior, not language semantics. |

## Sources

- [PSeInt pseudocode reference](https://pseint.sourceforge.net/index.php?page=pseudocodigo.php)
- [PSeInt features and language profiles](https://pseint.sourceforge.net/index.php?page=features.php)
- [Pinned default profile settings](https://sourceforge.net/p/pseint/code/ci/5ee1dc260181191e1cfa83fe02726f6812f6481c/tree/pseint/LangSettings.cpp)
- [Bundled language help for version 20220506](https://sourceforge.net/p/pseint/code/ci/5ee1dc260181191e1cfa83fe02726f6812f6481c/tree/bin/help/)
- [Existing conformance tests](tests/conformance.rs)
