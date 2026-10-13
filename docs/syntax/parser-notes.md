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
alternatives with different precedence recurse.
