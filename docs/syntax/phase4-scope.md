# Phase 4 / v0.1 scope freeze

Baseline: `phase3` through `9ee0eaa` (WP1 bodyless abstract methods). This file is
the v0.1 admission gate; implementation status is tracked by
`feature-matrix.md`, and grammar behavior remains defined by `starry.ebnf`.

## Frozen in v0.1

- Scalar core, checked arithmetic, control flow, patterns, and value aggregates.
- Struct/enum/class layout, field ARC traversal, nullable class ABI, wide i128.
- Closures, function values, traits, generic monomorphization, class-root `dyn`.
- Single inheritance, virtual dispatch, `super`, protected access, abstract
  methods with override-completeness checking.
- Modules/packages, visibility, native build, FFI, Windows/Linux runtimes.

## Stable rejections

| Surface | Gate | Reason |
|---|---|---|
| `by` delegation and class downcast | sem | Phase 3b candidates; no v0.1 runtime need. |
| Secondary constructors (`init`) | sem | Primary constructor plus initializer path is the frozen model. |
| Class body fields / method or class defaults | sem | Layout and ABI remain tied to primary constructors. |
| Associated types / generic `dyn` | sem | Trait-object ABI is frozen for non-generic traits. |
| Union declarations, accessors, operator overloading | sem | Stable diagnostics until their ABI/semantics have a dedicated spec. |
| Generic `main`, weak reassignment, boxed `Fn` global | sem/irc | Entry/runtime ownership model is not defined. |
| `comptime`, macros, concurrency, checked pointers | parse/sem | Phase 5/6 triggers have not been met. |
| `extern "c"` i128 scalars | sem | Target C ABI mismatch; aggregates containing i128 remain allowed. |

## Admission rule

A capability enters v0.1 only with parser/sem behavior, lowering, tests, and a
feature-matrix row that agree. A surface may be excluded with a stable typed
diagnostic; a compiler path that accepts source but cannot lower or run is not
admissible.

## Phase 4 closeout

At `v0.1.0-rc`, WP2-WP5 close the correctness and release boundary: ABI cases
run end-to-end or reject with a stable diagnostic; inheritance, aggregate/enum
ARC, nullable ABI, and reassignment paths are in the Linux sanitizer corpus;
deterministic arithmetic programs fuzz the interpreter/native differential pair;
TypeInfo/vtable snapshots are pinned; the CI matrix runs Windows/Linux plus the
sanitizer job; a clean clone builds with `--locked` and the sample corpus runs;
the quickstart tutorial covers five runnable language slices.
