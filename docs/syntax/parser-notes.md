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

A checkpoint contains the cursor position, pending token length, previous-token
span, and diagnostic length. Failed speculation rolls all four back. Node IDs
are not rolled back.

## Synchronization sets

| Context | Synchronize at |
|---|---|
| Expression | `NL`, `;`, `,`, `)`, `]`, `}`, EOF |
| Statement | `NL`, `;`, `}`, EOF |
| Declaration | `NL`, EOF, and every token that can start a top-level declaration |

Errors use stable `DiagCode` values. One `(span, code)` pair is emitted once;
`DiagCollector` keeps the first visible id per key and treats speculative ids
below the visible diagnostic length as stale, so rollback is O(1). Error/missing
nodes suppress downstream cascades.

## Depth

Expression, type, pattern, block, declaration, and macro-group recursion shares
`MAX_PARSE_DEPTH = 50`. Left-associative binary chains stay in the Pratt loop;
only prefix, postfix, nested groups, and declaration/member recursion consume
budget. Overflow produces `DepthLimitExceeded` plus an error/missing node; it is
never a machine `Err`.

## Namespace decisions (v0.4)

| Decision | Rule |
|---|---|
| Module declaration removed | No per-file package header; module paths are compiler metadata derived from the manifest package name + file path; one directory = one module. |
| Path separator | Qualified names use `.` only; `::` is no longer a token. |
| Import suffixes | `.*` (wildcard) and `as` IDENT (alias) only. |
| Visibility | Top-level `private` = file-private; `internal` = package-private; the semantic phase owns the full ladder. |
| Import transitivity | Imports are non-transitive explicit edges; cycle detection runs on this graph. |
| String class escapes | In-class `\r` `\n` `\t` now decode as control characters (root-cause fix for the string-literal lexer bug). |
| Named raw strings | Native import paths and extern languages are decoded from their raw `String` token before interning; invalid escapes emit `InvalidEscape`; expression strings retain raw spans for interpolation. |
| Newline policy (v0.4) | Swift-style: newline inside `{}` terminates statements; an expression continues across a newline only for a trailing binary operator or a leading binary-only operator / `.`. |
| Assignment target (v0.4) | The parser enforces the EBNF `Place := PostfixExpr`; binary/control-flow targets emit `InvalidAssignmentTarget`. |
| When branches (v0.4) | Separators: comma or newline; branches are real patterns with optional `if` guards (speculative pattern parse, rollback to expression). |
| Loop labels (v0.4, queued) | Rust-style quoted labels: definition `'ident :` loop, reference `break 'ident [Expr]`; identifier values become reachable. |
| Self receiver (deferred) | No method receiver syntax yet; design during the semantic phase together with symbol tables. |
| Nullability (v0.4, decided) | Kotlin/Swift family: `T?` with `?.` / `!!` / `??` for absence; errors dispatch via `when` on Result-style enums; lowering deferred to the semantic phase. |
| Error propagation | No `?`-propagation operator; a Swift-style `try` marker may be added later without touching `?.`. |
| Async (D-014) | async/await/spawn/concurrent grammar removed from the lexicon (2026-11-25); parse-level rejection. Concurrency is deferred to the next stage. |
| Receiver (v0.4, decided) | Ctor params may carry `val`/`var` (grammar supported) and become properties; methods see properties with implicit self; locals shadow properties (warn-grade diagnostic recommended). |
| Enum payload | Named declaration; construction and pattern matching bind positionally by declaration order (reordering is breaking). |
| Struct separators (v0.4) | Fields allow optional trailing comma, matching enum entries and when-branches. |
| Named arguments (v0.4, decided) | Call site `name = value` (Kotlin/Python style); positional-first mixing allowed; names stored in the AST. |
| Arrays (v0.4, decided) | Fixed-size value arrays `[T; N]` with `[value; N]` literal; postfix indexing/assignment; large-array copy semantics documented; dynamic arrays deferred. |
| Operator overloading (v0.4, decided) | Member `operator fun` with fixed mapping plus/minus/times/div (unary minus = unaryMinus); `+=` desugars to plus + rebind; `==`/`!=` deferred to structural equality. |
| by/to (D-014) | `by` kept for superitem delegation (inheritance stage); `to` removed from the lexicon (grow/provider grammar deleted). |
| stdlib (deferred) | API shape follows semantic-phase decisions; sample APIs are proposals, not commitments. |
| FFI (deferred) | No compatibility target; `extern`/native import stays as-is until the project needs C/C++/Rust interop. |
| Macro call (v0.4) | `name!(token-stream)`; the EBNF `#` form is dropped; the stream is captured raw, expansion belongs to a later phase; macro declaration removed (D-014), sem rejects calls via SemMacroExpansion. |
| String interpolation (v0.4) | Parser sub-scan: `$IDENT` / `${Expr}` re-lexed with span rebasing; the matching `}` is found with the real sub-lexer; `\$` stays literal; a shared interner/id-gen/diag child session parses the inner expression. |

## Syntax gap closures (v0.4)

Each row is a decision, the grammar/AST change it forced, and the external
precedent it rests on. `starry.ebnf` is the source of truth for the shapes.

| Decision | Change | Precedent |
|---|---|---|
| `Self` is the type, `self` is the value | `TypeKind::SelfType` is now produced for `Self`; lowercase `self` is rejected in type position | Rust `PathIdentSegment → … \| self \| Self \| …` keeps the two apart |
| `Self` takes no type arguments | `Self<T>` is a diagnostic, not a silently accepted `Named` | EBNF `PrimaryType := … \| "Self" \| …` has no `TypeArgs` |
| Unit type | `()` produces `TypeKind::Unit` (the variant existed but was never built) | EBNF `UnitType := "(" ")"` |
| `!is` matches `!in` | `ExprKind::Is` gained `negated`, mirroring `ExprKind::In` | Kotlin `isOperator : 'is' \| NOT_IS` and `inOperator : 'in' \| NOT_IN` are a symmetric pair |
| Rest patterns | `rest: Option<usize>` on `PatKind::Array` and `PatKind::Tuple`; `..` may sit in any position, at most once | Rust RestPattern: "may only appear once as one of the elements" in tuple, tuple struct and slice patterns |
| When-branch annotations | a top-level binding annotation in a `WhenPattern` is a no-arrow type | Kotlin avoids the clash by having no typed binding patterns in `whenCondition`; Starry's `->` is both a branch arrow and a function-type arrow |
| `impl` type parameters | `ImplDecl := «impl» TypeParams? Type "for" Type …` | Rust `GenericParams` on `impl` |
| Intersection bounds | `Bound := Type { "&" Type }`, shared by inline and `where` slots | Swift protocol composition `A & B`; `+` is the addition operator here |
| Extern blocks | `extern "c" { … }` lists multiple `FunSignature`s | Rust `ExternBlock → … { InnerAttribute* ExternalItem* }` |
| Signature terminators | `;` is optional on `FunSignature` and `AssociatedType`; a redundant `;` is accepted without a diagnostic | Rust needs `;` because Rust statements are not newline-terminated; Starry's are, and `AssociatedType` already omitted it |
| Class bodyless functions | `ClassFunDecl := ... WhereClause? [FunctionBody]`; sem restricts the missing body to abstract methods in abstract classes | Kotlin `abstract fun f()` declarations have no body and sem fills the implementation contract in subclasses |
| Import members | comma-separated per the EBNF, with `;` and newline accepted as redundant terminators | EBNF `ImportMember { "," ImportMember } [ "," ]` |
| `let` is not a keyword | a statement starting with bare `let` + identifier is `DiagCode::LetKeyword`, recovered as **one** binding statement | Deleting the keyword left `let x = 1` silently parsing as an ignored expression statement plus an assignment — Rust calls the former an expression statement whose "result is ignored" |

### Why `let` is an error, not a warning

The grammar is technically legal (`let` is an identifier; `x = 1` is an
assignment), so a lint-level message would be defensible — rustc routes the
equivalent "path statement has no effect" case to a lint. But `let x = 1` cannot
express a Starry program: the language has no `let`, and the only reading is a
misremembered binding. The behaviour being deleted is the split itself, so the
statement is recovered as a single binding and the parse does not produce two
statements. `let` remains a legal identifier everywhere else (`val let = 1`,
`let(1)`, `let.x`, `f(let)`), so nothing is reserved.

### Block tail expressions are positional, not per-statement

EBNF `Block := "{" [ NL ] { Statement } [ NL ] [ Expr ] [ NL ] "}"` makes the
trailing `Expr` a **position** fact: it is the block value only when nothing
follows it. The statement parser classifies a bare expression statement as
`ParsedStatement::Tail` without positional knowledge, so the block loop gates
the classification: after the expression, skip newlines and check for `}` —
tail only when the brace is next; otherwise the expression is demoted to
`StmtKind::Expr` and keeps source order. Without the gate, an `if` in the
middle of a block landed in `tail` while later statements accumulated in
`stmts`, scrambling AST order (consumer-visible through `type_block` and any
pass that reads statements before the tail).

Two span corollaries ship with the same change: `parse_block_after_open`
returns the close-brace span (`Block` stores neither brace), and `parse_if`
ends at that close brace for a then/else block instead of
`block_tail_span` — an empty block used to collapse the if-expression span to
`0..0` via `Span::default`.
### Pointer types are prefix, `const`/`mut` are token-class split

`PointerType := "*" [ "const" | "mut" ] PrimaryType` lives in
`parse_atomic_type` (D-007 slice). The qualifier predicate must split by
token class: `const` is a modifier word (lexically IDENT, matched by
text) while `mut` is a hard keyword (`HardKeyword::Mut`) — a single
text predicate misses the latter. Bare `*T` parses as const.
The `?` suffix loop is duplicated from `parse_named_suffix` so `*T?`
(nullable pointer, the FFI-common shape) reads as
`Nullable(Pointer(T))`; `*(T?)` stays available through parens.

### Prefix type constructors chain (`?` / `&` / `*const` / `*mut`)

D-007 / M2 makes type constructors prefix and self-contained:
`?T` nullable, `&T`/`&mut T` borrow, `*const T`/`*mut T` pointer parse in
`parse_atomic_type` and recurse into `parse_atomic_type` under `try_with_depth`
(each star/amp/question spends depth budget — deep chains would otherwise
overflow). `const` is a modifier word (lexical IDENT, matched by text);
`mut` is a hard keyword — the predicate must split by token class. Bare `*T`
recovers as const after an ExpectedToken diagnostic. The atomic suffix
sugar (`User?` = `?User`) lives only in `parse_named_suffix`; array/tuple
types reject a trailing `?` with a hint pointing at the prefix form
(`?[T; N]`, `?(T, U)`). `*const T?` reads as `*const (?T)` because the
inner named type consumes the `?` — nullable pointer is only `?*const T`.

### Block-tail `loop` normalizes to `LoopExpr`

The same position gate applies to `loop`: a `LoopStatement` whose block
ends right after it (`}` next, newlines aside) is rewritten in
`parse_block_after_open` into `Tail(ExprKind::Loop)` — keeping the node id,
so label/flow resolution (`LabelFrame.node`) is unchanged. A `loop` followed
by more statements stays a statement (a break-with-value there discards its
value, Rust discipline). This makes `fun f() : int { 'outer: loop { break
'outer 42 } }` return 42 instead of silently dropping the value:
`ExprKind::Loop` carries the label (parser-accepted only via this
normalization; value-position `loop` remains unlabeled), and `resolve`
pushes the label frame exactly like `StmtKind::Loop` does.

### `is` operands never take a struct tail

An `is` operand is always followed by a mandatory statement block (`if e is
A { ... }`), so a trailing `{` belongs to the block, never to a struct
pattern. `parse_is_operand_pattern` parses the operand with struct tails
suppressed; this resolves the ambiguity that previously swallowed the
then-block (`if e is A { }` failed to parse). Nested patterns inside the
operand keep full pattern syntax; `in` is unaffected — its right operand
is a collection expression parsed at range binding power (17), matching
EBNF `RelationalExpr := RangeExpr { ( ... | "in" | "!in" ) RangeExpr }`.

Known EBNF gap: the grammar gives `is`/`in` operands as `RangeExpr` only,
with no is-pattern production; the AST carries `ExprKind::Is { pattern }`
for pattern matching. The grammar needs an explicit is-pattern production
(tracked here until the EBNF is amended).
### Deferred

| Gap | Why it is still open |
|---|---|
| `impl … where …` | `ImplDecl` has no `WhereClause` in the EBNF. Inline bounds (`impl<T : Show>`) already bind `T`. Needs its own decision. |
| Unbound `T` in `impl Show for List<T>` | Whether `T` is bound is a semantic question, not a parse one. |
| `rest @ ..` binding | The representation (`rest: Option<usize>`) can carry it; the EBNF has no `@` binding operator yet. |
| Symbolic operator members (`a.+`) | `PostfixSuffix := "." IDENT`; operator overloading maps to named methods (`plus`, `unaryMinus`). |

## Keyword convergence (D-014, 2026-11-25)

Removed from the lexicon (parse-level rejection): async/await/spawn/
detached/concurrent/thread/fiber/process/actor/aspect/provide/macro/
comptime/grow/bits/before/after/around/proceed/singleton/scoped/
transient/bind/to/out/vararg/const/inline/infix/package/namespace/static.

Added: "alias" (type-alias hard keyword; "type" remains for
associated types only) and "deinit" (slot word, parsed via
SlotWord::Deinit). Frozen v1 budget: 35 hard / 10 modifier / 8 slot
= 53 keywords; `namespace` and `static` removed 2026-11-25 (namespace:
parse-only, no C++ FFI design; static: top-level fun/val/var and package
namespaces cover static members; re-add with a spec). No further reduction
(capability needs).
