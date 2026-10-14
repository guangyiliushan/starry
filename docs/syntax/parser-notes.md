# Parser decision notes

This page is the implementation gate for declaration, statement, and recovery
parsing. `docs/syntax/starry.ebnf` remains the grammar source of truth.

## Dispatch

| Token form | Parser decision |
|---|---|
| Hard keyword | Exhaustive `TokenKind::Keyword(HardKeyword::...)` dispatch |
| Modifier word | Accepted only in `Modifiers` |
| Slot word | `Identifier` token checked only in its named EBNF slot |
| Identifier | Name, path segment, call, or expression |

There is no `let` dispatch. Ordinary bindings start with `val` / `var`.
Refutable conditions use `Expr` (often `is`) or `val Pattern = Expr`.

## Speculative points

| Point | Protocol |
|---|---|
| `f<T>(...)` vs `f < T` | Save cursor + diagnostics, parse type args, require `(`. On failure roll both back. |
| Nested generic close | Lexer `>>`, `>>=`, and `>=` are split only in type-argument close context. |
| Lambda | `{ -> ... }`, `{ it ... }`, and `{ pattern -> ... }` are parsed without emitting speculative diagnostics. |

A checkpoint contains the cursor position, pending token length, and diagnostic
length. Failed speculation rolls all three back. Node IDs are not rolled back.

## Synchronization sets

| Context | Synchronize at |
|---|---|
| Expression | `NL`, `;`, `,`, `)`, `]`, `}`, EOF |
| Statement | `NL`, `;`, `}`, EOF |
| Declaration | `NL`, EOF, and every token that can start a top-level declaration |

Errors use stable `DiagCode` values. One `(span, code)` pair is emitted once;
error/missing nodes suppress downstream cascades.

## Depth

Recursive expression parsing shares `MAX_PARSE_DEPTH = 100`. Left-associative
binary chains stay in the Pratt loop; only prefix, postfix, nested groups, and

## Namespace decisions (v0.4)

| Decision | Rule |
|---|---|
| Module declaration removed | `package` is the only header namespace; a compilation unit = files with the exact same package name. |
| Path separator | Qualified names use `.` only; `::` is no longer a token. |
| Import suffixes | `.*` (wildcard) and `as` IDENT (alias) only. |
| Visibility | Top-level `private` = package-visible (declaration-level); the semantic phase owns the full ladder. |
| Import transitivity | Imports are non-transitive explicit edges; cycle detection runs on this graph. |
| String class escapes | In-class `\r` `\n` `\t` now decode as control characters (root-cause fix for the string-literal lexer bug). |
| Newline policy (v0.4) | Swift-style: newline inside `{}` terminates statements; an expression continues across a newline only for a trailing binary operator or a leading binary-only operator / `.`. |
| When branches (v0.4) | Separators: comma or newline; branches are real patterns with optional `if` guards (speculative pattern parse, rollback to expression). |
| Loop labels (v0.4, queued) | Rust-style quoted labels: definition `'ident :` loop, reference `break 'ident [Expr]`; identifier values become reachable. |
| Self receiver (deferred) | No method receiver syntax yet; design during the semantic phase together with symbol tables. |
| Nullability (v0.4, decided) | Kotlin/Swift family: `T?` with `?.` / `!!` / `??` for absence; errors dispatch via `when` on Result-style enums; lowering deferred to the semantic phase. |
| Error propagation | No `?`-propagation operator; a Swift-style `try` marker may be added later without touching `?.`. |
| Async (v0.4, decided) | `async`/`await` are reserved with zero semantics; `spawn thread` is the only implemented primitive; `fiber`/`process` parse legally but are unimplemented (remove the grammar if no runtime lands). |
| Receiver (v0.4, decided) | Ctor params become properties only with `val`/`var` prefix; methods see properties with implicit self; locals shadow properties (warn-grade diagnostic recommended). |
| Enum payload | Named declaration; construction and pattern matching bind positionally by declaration order (reordering is breaking). |
| Struct separators (v0.4) | Fields allow optional trailing comma, matching enum entries and when-branches. |
| Named arguments (v0.4, decided) | Call site `name = value` (Kotlin/Python style); positional-first mixing allowed; names stored in the AST. |
| Arrays (v0.4, decided) | Fixed-size value arrays `[T; N]` with `[value; N]` literal; postfix indexing/assignment; large-array copy semantics documented; dynamic arrays deferred. |
| Operator overloading (v0.4, decided) | Member `operator fun` with fixed mapping plus/minus/times/div (unary minus = unaryMinus); `+=` desugars to plus + rebind; `==`/`!=` deferred to structural equality. |
| by/to (v0.4, decided) | `by` demoted to slot word (superitems); `to` was already a slot word (grow/provider); plain identifiers unblocked. |
| stdlib (deferred) | API shape follows semantic-phase decisions; sample APIs are proposals, not commitments. |
| FFI (deferred) | No compatibility target; `extern`/native import stays as-is until the project needs C/C++/Rust interop. |
| Macro call (v0.4) | `name!(token-stream)`; the EBNF `#` form is dropped; the stream is captured raw, expansion belongs to a later phase. |
| String interpolation (v0.4) | Parser sub-scan: `$IDENT` / `${Expr}` re-lexed with span rebasing; `\$` stays literal; a shared interner/id-gen/diag child session parses the inner expression. |
