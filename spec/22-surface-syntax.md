# CobaltC Surface Syntax

Status: normative artifact
Version: 2.45.0
Conforms to: `spec/02-schema.md`
Governed by: `CobaltC_Master_Instructions.md` §1, §10, §23

## Purpose

Assigns concrete syntax to every construct given semantics in
`spec/05`–`spec/21`; introduces no meaning of its own (§10). This
grammar is complete for the language as specified: every production
maps to a named rule, and no rule lacks a production. Disambiguation
is stated where the grammar alone would be ambiguous.

**Syntax errors.** A text this grammar does not derive — lexically (§1)
or syntactically (§2–§3), or a form a rule below calls a syntax error —
is rejected before any of the program runs: `diag.syntax-error`
(static), at the first token (or character) that does not fit, its
message saying what was expected there and what was found (CHG-0082).

## 1. Lexical structure

    program        ::= item*
    whitespace     ::= (' ' | '\t' | '\r' | '\n')+                 -- ignored between tokens
    comment        ::= '//' any* newline  |  '/*' any* '*/'         -- ignored; block comments do not nest
    identifier     ::= (letter | '_') (letter | digit | '_')*      -- not a keyword and not a type-name; letters are ASCII
    int-literal    ::= (digits(digit) | '0x' digits(hex-digit) | '0o' digits(oct-digit) | '0b' digits(bin-digit)) (':' int-type)?
                                                                     -- no sign (unary '-' is an operator); D-0035:
                                                                     -- base 16, 8, 2 after `0x`, `0o`, `0b`
    digits(d)      ::= d ('_'? d)*                                   -- D-0043: a `_` between two digits, ignored:
                                                                     -- `1_000_000`, `0xFFFF_0000`, `3.141_592`
    oct-digit      ::= '0'..'7'        bin-digit ::= '0' | '1'
    float-literal  ::= digits(digit) ('.' digits(digit) exponent? | exponent) (':' ('f32' | 'f64'))?
    exponent       ::= ('e' | 'E') ('+' | '-')? digits(digit)         -- D-0033: `1.0e-7`, `2.5E3`, `1e5`
    bool-literal   ::= 'true' | 'false'
    str-literal    ::= '"' (str-char | str-escape)* '"'               -- type `str`; spec/21 [Str-Literal]
    str-char       ::= any source character except '"', '\', newline  -- source text is UTF-8, so every str-char
                                                                     -- contributes one well-formed scalar value
    str-escape     ::= '\n' | '\r' | '\t' | '\0' | '\\' | '\"'
                     | '\u{' hex-digit{1,6} '}'                      -- a Unicode scalar value, encoded as UTF-8;
                                                                     -- a surrogate or > 10FFFF is ill-formed
    byte-literal   ::= 'b' '"' (byte-char | byte-escape)+ '"'       -- type `array<u8, N>`, N the byte count;
                                                                     -- sugar for [Array-Construct] (spec/16 §3)
    byte-char      ::= any ASCII character except '"', '\', newline  -- a non-ASCII character is ill-formed here
    byte-escape    ::= '\n' | '\r' | '\t' | '\0' | '\\' | '\"'
                     | '\x' hex-digit hex-digit                       -- any byte value, 00..FF
    byte-char-literal ::= 'b' "'" (byte-char1 | byte-escape | "\'") "'"   -- D-0033: one byte; an unsigned type from
                                                                     -- context, `u8` otherwise (D-0102)
    byte-char1     ::= any ASCII character except "'", '\', newline
    hex-digit      ::= digit | 'a'..'f' | 'A'..'F'
    keyword        ::= 'fn' | 'struct' | 'bitstruct' | 'enum' | 'resource' | 'match' | 'if' | 'else' | 'while'
                     | 'return' | 'auto' | 'module' | 'import' | 'export' | 'unsafe' | 'extern'
                     | 'break' | 'continue' | 'move' | 'mut' | 'true' | 'false' | 'as' | 'void'
                     | 'for' | 'const'                                 -- D-0035, D-0036
                     | 'foreach'                                       -- D-0042; `in` is not reserved
    int-type       ::= 'i8' | 'i16' | 'i32' | 'i64' | 'i128' | 'u8' | 'u16' | 'u32' | 'u64' | 'u128' | 'isize' | 'usize'
    type-name      ::= int-type | 'f32' | 'f64' | 'bool' | 'str' | 'ref' | 'rawptr' | 'array' | 'handle' | 'mutex' | 'guard' | 'slice'
                                                                     -- the built-in type constructors of §3; reserved like
                                                                     -- keywords (C reserves `int`, `char`, …); `shared`/
                                                                     -- `exclusive` occur only inside `ref<τ, ·>`, where no
                                                                     -- identifier can, and are not reserved

Two `str-literal`s with only whitespace and comments between them are one
`str-literal` whose text is theirs in order (D-0078), as in C, so a long
text or format is written a line at a time: `"a\n" "b\n"` is `"a\nb\n"`.
Byte literals are not joined.

Tokens are the longest match. A numeric literal's type: `rule.arith.literal`;
a `str-literal` is always `str` and a `byte-literal` always
`array<u8, N>` — neither takes an expected type (`rule.type.expected`).
A literal that is not closed on its own line, an escape outside the
listed set (`\x` in a `str-literal`, `\u` in a `byte-literal`), a
non-ASCII character in a `byte-literal`, and the empty `b""` are all
lexically ill-formed: the program does not parse. Because a
`str-literal` admits no way to write an arbitrary byte, its decoded
bytes are well-formed UTF-8 by construction (`inv.str.utf8-validity`,
`spec/03`). There is no character literal (a deliberate absence, §5).
A single byte is a `byte-char-literal`, `b'a'` (D-0033): an integer
literal of type `u8`, whatever its context, whose value is the byte;
`b''`, a second byte, and a non-ASCII character are lexically
ill-formed. A `float-literal` with an exponent denotes the decimal
number it spells (`rule.arith.literal`), as `String::parse<f64>` reads it.
A `_` inside a number (D-0043) separates two digits and is otherwise
ignored: `1_000` is `1000`. One elsewhere — doubled, first or last,
beside `0x`, the point or the exponent's `e` or sign — makes the literal
lexically ill-formed. The separator is source syntax only: `String::parse<T>`
does not accept it, and no conversion writes it.

**Brace placement is not a grammar fact.** Since `whitespace` (including
newlines) is insignificant between tokens, no brace-placement
convention — Allman, K&R, or anything else — is more "correct" than
another to a conforming implementation; a program's meaning cannot
depend on which one its author used. This specification's own code —
`spec/examples.md`, `spec/21-standard-library-semantics.md`, and every
standalone code block elsewhere — uses **Allman style** (the opening
brace of every brace-delimited construct, including `if`/`while`/
`match`, on its own line) as a documentation convention, by the human
owner's direction, not a language requirement: a CobaltC programmer
may use any brace placement. Two structural exceptions, for content
that cannot hold real line breaks without breaking its container:
code embedded in a markdown table cell (`spec/conformance.md`, and the
`Result::map_err` row of `spec/21` §0) and inline backtick-quoted code
embedded in a prose sentence (e.g. `spec/examples.md`'s
`**Corrected:** \`...\`` notes) are both single-line by necessity, not
by exception to the convention. (Non-normative: the formatter `cobfmt`,
`impl/cobfmt/`, writes this convention for any program; it changes only
whitespace, never a token, so it cannot change what a program means.)

## 2. Expressions

    expr        ::= assign
    assign      ::= logic-or (assign-op assign)?                     -- right-assoc; LHS must be a place; spec/05 [Write]
    assign-op   ::= '=' | '+=' | '-=' | '*=' | '/=' | '%=' | '&=' | '|=' | '^=' | '<<=' | '>>='
                                                                     -- `p op= e`: spec/13 [Compound-Assign] (D-0035)
    logic-or    ::= logic-and ('||' logic-and)*                     -- spec/13 §4
    logic-and   ::= compare ('&&' compare)*
    compare     ::= bitor (('==' | '!=' | '<' | '<=' | '>' | '>=') bitor)?   -- non-associative; spec/06 [Cmp-*]
    bitor       ::= bitxor ('|' bitxor)*
    bitxor      ::= bitand ('^' bitand)*
    bitand      ::= shift ('&' shift)*                              -- spec/06 [Bit-*]
    shift       ::= additive (('<<' | '>>') additive)*              -- spec/06 [Shl-Checked]/[Shr-Checked]; D-0086
    additive    ::= multiplicative (('+' | '-') multiplicative)*
    multiplicative ::= unary (('*' | '/' | '%') unary)*
    unary       ::= ('-' | '!' | '~') unary                          -- spec/06 [Neg]/[Bool-Not]/[Bit-Not]
                  | '&' 'mut' unary  |  '&' unary                    -- spec/09 [Ref-Form]: exclusive / shared
                  | '&' 'mut'? postfix '[' expr '..' expr ']'         -- a slice, spec/16 [Slice-Form] (D-0047); a range
                                                                     -- `[lo .. hi]` is written only here, and last
                  | '*' unary                                        -- spec/09 [Ref-Deref-Place]
                  | postfix
    postfix     ::= primary postfix-op*
    postfix-op  ::= '.' identifier                                   -- spec/16 [Field-Access]
                  | '[' expr ']'                                     -- spec/16 [Index-Checked], [Slice-Index], [Index-Vec]
                  | '(' args? ','? ')'                               -- spec/15 [Call]; ','? D-0098
                  | '?'                                              -- spec/18 [Propagate]
    args        ::= expr (',' expr)*
    primary     ::= int-literal | float-literal | byte-char-literal | bool-literal | '(' ')'      -- literals, unit
                  | '$'                                              -- inside `[…]` only: spec/16 rule.agg.dollar (D-0047)
                  | str-literal                                      -- spec/21 [Str-Literal]
                  | byte-literal                                     -- spec/16 [Array-Construct] on its bytes
                  | path type-args?                                  -- name, item, variant, or generic instantiation;
                                                                     -- spec/05 [Binding-Lookup], spec/17 [Resolve-*]
                  | path type-args? '{' field-inits? '}'            -- struct literal; spec/16 [Struct-Construct]
                  | '[' args? ','? ']'                               -- array literal; spec/16 [Array-Construct]
                                                                     -- (a trailing comma is allowed, D-0072)
                  | '[' expr ';' length ']'                          -- repeated value; spec/16 [Array-Repeat] (D-0072)
                  | '(' expr ')'
                  | block | 'unsafe' block                           -- spec/14 §1, spec/20 §1
                  | 'if' '(' cond ')' block ('else' (block | if-expr))?   -- spec/14 §3 (`else if` is sugar for else { if … })
                  | 'while' '(' cond ')' block                            -- spec/14 §4
                  | for-expr                                              -- spec/14 rule.control.for (D-0035)
                  | foreach-expr                                          -- spec/14 rule.control.foreach (D-0042)
                  | 'match' '(' expr ')' '{' (arm ',')+ '}'               -- spec/16 §4
                  | 'return' expr?  |  'break'  |  'continue'         -- spec/15 §4, spec/14 §4
                  | closure                                           -- disambiguation (6), below
    if-expr     ::= 'if' '(' cond ')' block ('else' (block | if-expr))?
    cond        ::= expr
                  | pattern '=' expr                                  -- D-0115: spec/14 [If-Pattern], [While-Pattern];
                                                                     -- the pattern's top is `path '(' pattern ')'`,
                                                                     -- a qualified path, or a literal (disambiguation (7))
    field-inits ::= field-init (',' field-init)* ','?
    field-init  ::= '.' identifier '=' expr                          -- C99 designated-initializer style
    arm         ::= pattern ':' expr                                  -- spec/16 §4 (D-0056)
    pattern     ::= '_'                                               -- wildcard
                  | '-'? integer-literal | byte-char-literal          -- a value it must equal (D-0057)
                  | 'true' | 'false'
                  | string-literal                                    -- text it must equal, byte for byte (D-0127)
                  | path                                              -- a variant (its payload unmatched), a
                                                                     -- constant (its value), else a binder
                                                                     -- of the whole value (D-0058)
                  | path '(' pattern ')'                              -- a variant and its payload's pattern
    closure     ::= 'move'? '[' captures? ']' '(' params? ','? ')' (':' type)? block   -- spec/15 §6; ': type' D-0081
    captures    ::= identifier (',' identifier)*
    type-args   ::= '<' type (',' type)* '>'                         -- `Vec<i32>::new()`, `widen<u64>(x)`,
                                                                     -- `apply<i32, i32>(f, x)` — one bare form
                                                                     -- for every generic instantiation, whether
                                                                     -- an intrinsic or not
    path        ::= identifier ('::' identifier)*

**Disambiguation.** (1) Comparison is non-associative, so `a < b > c` is
never a comparison: `compare` allows at most one comparison operator,
so nothing can ever continue as a second one immediately after a
closing `>`. The token sequence `identifier '<' type (',' type)* '>'`
is therefore always read as a generic-argument list regardless of what
follows — a call (`id<i32>(5)`), a further path segment
(`Vec<i32>::new()`), or nothing at all (a bare `id<i32>` names the
instantiated value without calling it). (2)
`&mut` is two tokens but one operator. (3) `x = y = z` assigns right
to left. (4) A statement beginning with a `type-name` token or the
keyword `fn` (a `fn(...)` type) is a local declaration (`decl-stmt`,
below); a statement beginning with an identifier is a local declaration
iff that identifier names a struct or enum type visible at that point,
or it is followed by a second identifier and then `=` or `;` (two
names in a row begin no expression, `CHG-0081`): a type parameter
(`T x = a;`), or a name that is no type (`int x = 10;`, reported by
name resolution as `diag.unbound-name`, static, at the statement);
otherwise it is an expression statement. Struct/enum type names are
known from the full item set (`Σ.items`, `spec/17`) before any
statement is parsed — information name resolution already requires,
not an additional pass. (`i32 x = 5;`, `usize n = 0;`, `ref<T, shared>
r = &x;` — every built-in-typed declaration in `spec/21` and
`spec/examples.md` — is a declaration by the first clause; the 2.7.1
wording covered only the second.) (5) A brace body
opening with a bare identifier followed by `,` or `}` is
destructuring (`decl-stmt`, below); a brace body whose
fields are `.`-prefixed is a struct literal (`field-init`, above) —
the two shapes never overlap. (6) `'[' args? ']'` (array literal) and
`'[' captures? ']' '(' params? ')' block` (closure) begin identically
— a bare identifier is a valid element of both `args` and `captures`.
A `[...]` is a closure iff its closing `]` is immediately followed by
`'(' params? ')' block`; otherwise it is an array literal. Only once
the closure interpretation applies must every bracket element be a
bare identifier naming one of the closure body's free variables
(`diag.capture-list-mismatch` otherwise, `rule.fn.closure`) — this
check is not what chooses between the two interpretations. (7) A
condition beginning `identifier '('`, `identifier '::'`, or a literal
(`-`? integer, byte, `true`, `false`, text), read as a `pattern` and then
followed by a single `=`, is `pattern '=' expr` (D-0115); anything
else is an `expr` — so `Some(x) == e` is a comparison, and `x = e` (a
bare name) the assignment it is everywhere, which `[T-If]` rejects as a
condition. Both readings are decided by tokens alone.

`if`, `while`, and `match` all require parentheses around their
condition/scrutinee (`spec/22` 2.2.0), so a struct literal may appear
there directly (`if (Point { .x = 1, .y = 2 } == p) { … }`) without a
separate disambiguation rule: the mandatory `(...)` already delimits
the condition from the following `{`. Earlier versions of this grammar
needed a rule exactly for this case; mandatory parentheses removed the
ambiguity instead of working around it.

**Statements and blocks.**

    block       ::= '{' statement* expr? '}'                         -- spec/14 [Block-Enter]; the optional final
                                                                     -- expr is the block's value
    statement   ::= decl-stmt ';'
                  | 'const' (type | 'auto') identifier '=' expr ';'   -- a local constant (D-0066, D-0068), spec/17 §1a
                  | struct-decl | enum-decl | bitstruct-decl         -- a local type (D-0069, D-0132), spec/17 §1c; no `export`
                  | fn-decl                                           -- only `fn T::name`, T a local type (D-0069)
                  | expr ';'
                  | '_' '=' expr ';'                                  -- discard on purpose (D-0099), spec/13 [Discard]
                  | block-like                                        -- no ';' needed
    block-like  ::= block | 'unsafe' block | if-expr | 'while' '(' cond ')' block | for-expr | foreach-expr | 'match' '(' expr ')' '{' (arm ',')+ '}'
    for-expr    ::= 'for' '(' (decl-stmt ';' | expr ';' | ';') expr? ';' expr? ')' block
                                                                     -- `for (init; cond; step) body`; each part
                                                                     -- optional; a missing cond is `true`
    foreach-expr ::= 'foreach' '(' identifier (',' identifier (',' identifier)?)? 'in' expr ('..' expr)? ')' block
                                                                     -- `in` is the identifier `in` in this position
                                                                     -- only; elsewhere it is an ordinary name;
                                                                     -- `lo '..' hi`: a range, spec/14
                                                                     -- rule.control.foreach-range (D-0097), written
                                                                     -- only here
    decl-stmt   ::= type identifier                                   -- spec/11 [Let-Uninit]
                  | type identifier '=' expr                          -- spec/11 [Let]
                  | 'auto' identifier '=' expr                        -- spec/11 [Let]; type synthesized, rule.type.typing
                  | path '{' dfield (',' dfield)* (',' '..')? '}' '=' expr   -- struct destructuring (D-0044): every
                                                                     -- field named once, spec/11 [Let-Destructure];
                                                                     -- each field bound to its name; a resource field
                                                                     -- is moved via [Store-Sub-Relocate]'s inverse
                                                                     -- (relocate-out) — spec/21 §2 String::into_bytes
                  | path '{' '..' '}' '=' expr                         -- D-0104: `..` for the fields not named
    dfield      ::= identifier (':' identifier)?                      -- D-0104: `field : name` binds to another name

The body of `if`, `while` and `for` is always a block, so C's
`for (…);`, `while (…);` and `if (…);` — an empty body followed by a
block that runs once, unconditionally — cannot be written: the `;`
where a block must begin is a syntax error, and both implementations
say so.

A `block-like` used as a statement has its value discarded; the last
element of a block without a trailing `;` is the block's value
expression, whether or not it is block-like.

## 3. Declarations

    item        ::= fn-decl | extern-decl | extern-code | struct-decl | bitstruct-decl | enum-decl | module-decl | import-decl | const-decl
    const-decl  ::= vis 'const' (type | 'auto') identifier '=' expr ';'   -- spec/17 rule.module.const (D-0036);
                                                                     -- `auto`: the type is e's (D-0068)
    vis         ::= 'export'?
    type-params ::= '<' type-param (',' type-param)* '>'             -- spec/12 §1
    type-param  ::= identifier (':' bound)?                         -- a bound only on a function's (D-0090)
    bound       ::= 'eq' | 'ordered' | 'number' | 'integer' | 'clone'   -- contextual words, not keywords; 'clone': D-0116
    fn-decl     ::= vis 'fn' (identifier | identifier '::' identifier) type-params? '(' params? ','? ')' (':' type)? block
                                                                     -- `fn Type::name` declares an associated function
                                                                     -- (spec/17 §1); `fn Type::drop(ref<Type, exclusive> self)`
                                                                     -- is Type's destructor (spec/07 §1)
    params      ::= param (',' param)*
    param       ::= type identifier
    extern-decl ::= vis 'extern' 'fn' identifier '(' params? ','? ')' (':' type)? ';'    -- spec/20 §3; FfiType only
    extern-code ::= 'extern' str-literal ';'                         -- spec/20 §3 rule.trust.extern-code; declares
                                                                     -- no name, so it takes no vis
    struct-decl ::= vis 'resource'? 'struct' identifier type-params? '{' field* '}'     -- spec/16 §2; 'resource'
                                                                     -- sets is-resource = true (spec/12 §3)
    field       ::= vis type identifier ';'                          -- field visibility as item visibility (spec/17 §2)
    bitstruct-decl ::= vis 'bitstruct' identifier ':' int-type '{' bitfield* '}'   -- D-0118, spec/16 §2a: the int-type unsigned;
    bitfield    ::= vis identifier ':' int-literal ';'               -- the widths (1 .. 128) fill it exactly, low bit first,
                                                                     -- else a syntax error naming the shortfall
    enum-decl   ::= vis 'resource'? 'enum' identifier type-params? (':' int-type)? '{' (variant ',')* '}'
                                                                     -- ': int-type': codes, D-0106 (no type-params)
    variant     ::= identifier ('(' type ')')?                       -- payload-less ⇒ payload `unit`
                  | identifier '=' '-'? integer-literal              -- a code (D-0106), only with ': int-type'

    module-decl ::= vis 'module' identifier ( '{' item* '}' | str-literal ';' )
                                                                     -- the second form: the file the literal names
                                                                     -- supplies the body (spec/17 §5); the literal is
                                                                     -- a path in item position, not an expression
    import-decl ::= vis 'import' path ';'                            -- spec/17 §3; `export import`: a re-export (D-0136)

    type        ::= int-type | 'f32' | 'f64' | 'bool' | 'str' | 'void'   -- `void` is `unit`; the unit *value* is
                                                                     -- still written `()` (primary, above) — C
                                                                     -- has nothing to lend a value-position token
                  | 'ref' '<' type ',' ('shared' | 'exclusive') '>'
                  | 'slice' '<' type ',' ('shared' | 'exclusive') '>'   -- spec/16 type.slice (D-0047)
                  | 'rawptr' '<' type '>'
                  | 'array' '<' type ',' length '>'
                  | 'fn' '(' (type (',' type)* ','?)? ')' (':' type)?
                  | 'handle' '<' type '>' | 'mutex' '<' type '>' | 'guard' '<' type '>'    -- spec/19
                  | path ('<' type (',' type)* '>')?                  -- declared struct/enum, optionally instantiated;
                                                                     -- omitted arguments are inferred where
                                                                     -- rule.type.expected fixes them
    length         ::= const-expr                                    -- D-0126: folded before the program runs
    const-expr     ::= int-literal | identifier ('::' identifier)*   -- D-0074: a name of a `const` whose
                     | '(' const-expr ')' | ('-' | '~') const-expr
                     | const-expr ('+' | '-' | '*' | '/' | '%' | '&' | '|' | '^' | '<<' | '>>') const-expr   -- spec/22 §2 precedence;
                                                                     -- outside parentheses `>` and `>>` end a length
                                                     -- initializer is an integer literal; D-0105: a
                                                     -- qualified name reaches another module's, exported

A function without `: type` returns `unit` (equivalently, `: void`
may be written explicitly). `main` is `fn main()`, or
`fn main() : u8`, at the root module (`spec/15` §7). There is no `->` token anywhere in
this grammar; every return type, wherever one is written, is
introduced by `:`.

## 4. Correspondence table

| Construct | Rule |
|---|---|
| name, `p::name` | `rule.value-object.binding-lookup`, `rule.module.resolve` |
| literals | `rule.arith.literal`, `spec/13` §2; `"…"`: `rule.stdlib.str` `[Str-Literal]`; `b"…"`: `rule.agg.array-construct` |
| `e1 op e2`, `-e`, `!e`, `~e` | `rule.arith.*`, `rule.expr.logic`; `p + n` on a raw pointer: `rule.trust.rawptr` `[Rawptr-Offset]` |
| `&e`, `&mut e`, `*e` | `rule.ref.form`, `rule.ref.deref`; `*g` on a guard: `rule.conc.lock` `[Guard-Deref]`; `*p` on a raw pointer: `rule.trust.rawptr` `[Rawptr-Read]`/`[Rawptr-Move-Out]` |
| `e.f`, `e[i]` | `rule.agg.field-access`, `rule.agg.index` |
| `e(args)` | `rule.fn.call`, `rule.fn.generic-call`, `rule.fn.closure` |
| `e?` | `rule.fail.propagate` |
| `p = e` | `rule.value-object.write` (via `rule.expr.context`); `*p = e` on a raw pointer: `rule.trust.rawptr` `[Rawptr-Write]`/`[Rawptr-Move-In]` |
| `Name{…}`, `[…]`, `Vi(e)`, `Vi` | `rule.agg.struct-construct`, `array-construct`, `enum-construct` |
| `match` | `rule.agg.match` |
| `if`, `while`, `break`, `continue` | `rule.control.if`, `rule.control.while` |
| `return` | `rule.fn.return` |
| `{…}`, `unsafe {…}` | `rule.control.block`, `rule.trust.unsafe` |
| local declaration, `auto` | `rule.init.let` |
| closures | `rule.fn.closure` |
| declarations | `rule.fn.program`, `rule.module.*`, `rule.type.*`, `spec/07` §1 |

`spawn` and `join` are not core grammar: they are ordinary calls to
prelude intrinsics (`e(args)`, above), governed by `rule.conc.spawn`/
`rule.conc.join` exactly as `drop`/`sizeof`/`allocate` are governed by
their own rules — see `spec/21` §0.

## 5. Open items and deliberate absences

Every open item across the specification, for the human owner
(Master Instructions §18/§20). Nothing listed here is a soundness
gap: each is a restriction the rules enforce, or a capability the
language does not provide.

**Restrictions the rules impose (candidates for future features):**

1. `feat.explicit-lifetime-parameters` (`UNDER_INVESTIGATION`): a
   function with zero or several reference parameters may not return
   a reference (`rule.temporal.elision`).
2. Patterns are `_`, a binder, an integer/byte/`bool`/text literal (or
   a constant naming an integer or `bool` one), a variant, and a variant
   with a pattern for its payload, nested to any depth (D-0056, D-0057,
   D-0058, D-0127), and nothing
   else: no range, struct, slice or or-patterns, no guards;
   `Name{f1, …, fn} = e` names every field of `Name`, or `..` for the
   rest, and `f : name` renames one (D-0044, D-0104).
3. Partial initialization is rejected (D-0019): an aggregate is
   initialized whole.
4. A resource payload can be matched only from a whole owned or
   temporary enum (`[Match-Move-Through-Ref]`).
5. Generic bodies have only the built-in bounds (D-0090, D-0116):
   nothing on a bare `T` beyond what its bound (`eq`, `ordered`,
   `number`, `integer`, `clone`) grants, and a bound only on a
   function's type parameters. A struct or enum satisfies `clone`
   alone, by its fields or its own `T::clone`; the four operator bounds
   it never satisfies (`rule.type.kind`, `rule.type.bound`).
6. A borrow-capturing closure cannot be `spawn`ed; a closure is called
   through an exclusive borrow of itself.
7. Moving a resource out of a field of a live object is rejected
   (`[Store-Binding-Place-Transfer-Sub]`); move the whole container,
   or take it apart. A field of a *temporary* may be moved out
   (D-0103).

**Deliberate absences (a considered "no"):** character literals
(D-0020; a single byte is `b'a'`, D-0033; `str` has no indexing
syntax, only `str_byte`); slicing a `str` (D-0020's revisit conditions;
a read-only view of one is `StringView::of`, D-0134); `import … as` renaming; **method-call syntax** in
every form — `v.push(x)` as receiver sugar for `fn Vec::push`, uniform
call syntax for free functions, an auto-borrowing `.`, call chaining —
(the owner's final decision, `CHG-0132`: a call names the function's
owner and writes the borrow it takes, `Vec::push(&mut v, x)`, so the
mode of every access path is visible where the path is formed, and `.`
means one thing, field access); module-boundary `unsafe` gating (`spec/17` §4);
byte mutation of `String` (`spec/21` §2); atomics, weak memory
orders, deadlock freedom (`spec/19`); catchable faults (D-0009);
implicit conversions (D-0006); global inference (D-0012); user-extensible
traits or bounds (D-0010; the four built-in bounds of D-0090 are the whole
bound system); a separate mutability qualifier on bindings —
e.g. a `let mut`-style marker (`spec/05`): mutability is governed
entirely by the alias/borrow discipline, not a declared property of
the binding; hardware-facing constructs beyond `rawptr` + `extern`;
calling convention, ABI, and linking (beyond naming foreign code with
`extern-code`, whose meaning is implementation-defined, `spec/20` §3),
and a standard library beyond
`spec/21` (a file-backed `module` declaration, `spec/17` §5, is
source assembly into one program, not linking: the program remains a
single item set in one root module, `term.program`).

**Implementation-defined choices a conforming implementation must
document** (every `outcome: impl-defined` in the specification):
`AddrWidth`; byte order `BO`; enum discriminant width `DW`; struct
padding (`rule.agg.layout`); `dangling<T>()`; the encoding of `fn`
values; the meaning of a mutex's extra cells; the address a `str`
value's bytes occupy and whether equal values share one (`[Str-Ptr]`,
`[Repr-Str]`); the form in which termination outcomes and diagnostics
are reported, including how a diagnostic's location names the file it
lies in; which path forms a file-backed `module` declaration accepts
beyond the required `/`-separated relative core, and how a resolved
path denotes a file (`rule.module.file`, `spec/17` §5).

**Specification status.** Every rule, type, term, invariant, and
state component is `ACCEPTED` as of this version; `spec/AUDIT-2.md`
records the audit that produced the 1.0.0 semantics and
`spec/AUDIT-STATUS.md` its closure. This artifact's own 2.0.0 revision
is a concrete-syntax-only change (`CHG-0001`): no rule, invariant,
type, or state entity's meaning changed.

## Change Log

- 2.45.0 — `CHG-0163` (D-0136): `import-decl` takes `vis`;
  `export import p;` is a re-export (`spec/17` §3).
- 2.44.1 — `CHG-0158` (D-0134): §5's absences: a view of a `str` is
  `StringView::of`, no longer deferred; `String::parse` and
  `Result::map_err` named as renamed.
- 2.44.0 — `CHG-0155` (D-0132): `statement` gains a local `bitstruct`.
- 2.43.1 — §1: a non-normative note that `cobfmt` writes the documentation
  convention (patch level: no rule changes).
- 2.43.0 — `CHG-0150` (D-0127): `pattern` gains `string-literal`; §5's restriction 2 lists text literals; a text literal starts a `pattern '=' expr` condition.
- 2.42.0 — `CHG-0149` (D-0126): `length ::= const-expr`.
- 2.41.0 — `CHG-0141` (D-0118): `bitstruct-decl`, `bitfield`; `bitstruct` is a
  keyword.
- 2.40.0 — `CHG-0139` (D-0116): `bound` takes `clone`; §5's restriction 5.
- 2.39.0 — `CHG-0138` (D-0115): `cond ::= expr | pattern '=' expr` for `if`
  and `while`; disambiguation (7).
- 2.38.2 — Non-normative: §5's restrictions 2, 5 and 7 and two of its
  absences brought up to date with D-0053 (`StringView`), D-0090
  (built-in bounds), D-0103 (a temporary's field) and D-0104 (`..` and
  renaming in destructuring); the text had still described the language
  before them.
- 2.38.1 — Non-normative (`CHG-0132`, the owner's final decision): §5's
  deliberate absences name method-call syntax in every form.
- 2.38.0 — `CHG-0125` (D-0106): `enum-decl` takes `: int-type` and a
  variant `= integer-literal` (codes).
- 2.37.0 — `CHG-0124` (D-0105): an array length may be a qualified
  constant `m::N`.
- 2.36.0 — `CHG-0123` (D-0104): `dfield ::= identifier (':' identifier)?`,
  and `..` last in a destructuring.
- 2.35.0 — `CHG-0121` (D-0102): a byte literal takes an unsigned type
  from its context.
- 2.34.0 — `CHG-0118` (D-0099): the statement `_ = expr;`.
- 2.33.0 — `CHG-0117` (D-0098): a trailing comma in argument and
  parameter lists (and in a closure's and a `fn` type's).
- 2.32.0 — `CHG-0112` (D-0097): `foreach-expr` takes a range,
  `expr '..' expr`.
- 2.31.0 — `CHG-0105` (D-0090): a function's type parameter may carry a
  bound (`<T: ordered>`).
- 2.30.0 — `CHG-0101` (D-0086): a shift binds tighter than `&`, `^` and
  `|` and looser than `+`/`-` (was: looser than all three).
- 2.29.0 — `CHG-0097` (D-0081): `closure` takes an optional `: type`.
- 2.28.0 — `CHG-0094` (D-0078): adjacent string literals are one literal.
- 2.27.0 — `CHG-0087` (D-0074): an array length may name a constant whose
  initializer is an integer literal.
- 2.26.0 — `CHG-0085` (D-0072): `[e; N]`; a trailing comma in an array
  literal; `T[N] x` is a syntax error whose message names `array<T, N>`.
- 2.25.0 — `CHG-0082`: syntax errors are `diag.syntax-error`.
- 2.24.0 — `CHG-0081`: disambiguation (4) also takes `identifier identifier`
  then `=` or `;` as a declaration.
- 2.23.0 — `CHG-0079` (D-0069): `statement` gains a local `struct` or
  `enum` and a function of a local type.
- 2.22.0 — `CHG-0078` (D-0068): a constant's type may be `auto`.
- 2.21.0 — `CHG-0076` (D-0066): `statement` gains a local constant,
  `'const' type identifier '=' expr ';'`.
- 2.20.0 — `CHG-0066` (D-0058): `pattern`'s `path` alternative is a
  variant, a constant or a binder; restriction 2 amended.
- 2.19.0 — `CHG-0065` (D-0057): `pattern` gains literals; §5's
  restriction 2 lists the four forms.
- 2.18.0 — `CHG-0064` (D-0056): `arm` takes a nested `pattern`; §5's
  restriction 2 narrowed to literal and struct patterns.

- 2.17.0 — `CHG-0055` (D-0047): tokens `..` and `$`; type-name `slice`;
  the slice form `&a[lo .. hi]`; `$` in brackets.

- 2.16.0 — `CHG-0052` (D-0044): destructuring names every field of a
  struct; `foreach-expr` takes a third name.

- 2.15.0 — `CHG-0051` (D-0043): `digits(d)`: a `_` between two digits of
  an integer or float literal.

- 2.14.0 — `CHG-0050` (D-0042): keyword `foreach`; `foreach-expr`.

- 2.13.1 — Non-normative (`CHG-0045`): §3 notes that `for (…);`,
  `while (…);` and `if (…);` are syntax errors, the body being a block.

- 2.13.0 — `CHG-0044` (D-0035, D-0036): `int-literal` in bases 16,
  8 and 2; keywords `for` and `const`; `assign-op`; `for-expr`;
  `const-decl`.

- 2.12.0 — `CHG-0042` (D-0033): §1's `float-literal` admits an
  exponent; new `byte-char-literal` `b'x'` of type `u8`; §5's
  character-literal absence names it.

- 2.11.1 — Non-normative (`CHG-0040`, D-0031): §3's note on `main`
  mentions `fn main() : u8`.

- 2.11.0 — `CHG-0039` (D-0030): `item` gains `extern-code`,
  `'extern' str-literal ';'`, naming foreign code to link
  (`rule.trust.extern-code`); §5's out-of-scope list says linking is
  out of scope beyond it.

- 2.10.0 — `CHG-0026` (D-0021, human-directed): `module-decl` gains
  the alternative `str-literal ';'`
  — a module whose body is the named file (`spec/17` §5,
  `rule.module.file`). No keyword added: after `module identifier`
  only `{` or a string literal can follow, so `from`/`file(…)` were
  rejected as redundant tokens. §5's "linking" absence notes that
  this is source assembly, not linking; the impl-defined list gains
  path forms beyond the required core and file naming in diagnostic
  locations. Additive: the form did not parse under 2.9.0.
- 2.9.0 — `CHG-0025` (D-0020, human-directed): `str-literal` (`"…"`,
  type `str`) and `byte-literal` (`b"…"`, type `array<u8, N>`) added
  to §1 with their escape sets; `str` added to `type-name` and to
  `type`; both literals added to `primary`; the correspondence table
  routes them to `rule.stdlib.str` `[Str-Literal]` and
  `rule.agg.array-construct`. §5's "string/char literals" absence
  narrowed to character literals, a string view, and slicing. The
  2.8.1 sentence "there is no string or character literal" is
  withdrawn.
- 2.8.1 — Non-normative: `?`'s grammar comment follows `spec/18`
  1.2.0's rename of `[Propagate-Ok]` to `[Propagate]` (`CHG-0010`).
- 2.8.0 — `CHG-0009` (consistency pass). §1: the built-in type
  constructors (`int-type`, `f32`, `f64`, `bool`, `ref`, `rawptr`,
  `array`, `handle`, `mutex`, `guard`) are collected as `type-name`
  and excluded from `identifier`, and disambiguation (4) treats a
  statement beginning with one (or with `fn`) as a declaration. Under
  2.7.1, (4) recognised a declaration only when its leading identifier
  named a struct or enum, so `i32 x = 5;` — the form every example,
  conformance case, and `spec/21` body uses — parsed as an expression
  statement and was ill-formed; the `decl-stmt ::= type identifier`
  production already admitted it, so this resolves the prose in favor
  of the production. §4: the correspondence table names the
  raw-pointer and guard rules that `*p`, `p + n`, and `*p = e` reach.
  Non-normative in the same pass: two grammar comments cited rule
  labels that do not exist (`[Shl]`/`[Shr]`, `[Propagate]`); corrected
  to `[Shl-Checked]`/`[Shr-Checked]`, `[Propagate-Ok]`.
- 2.7.1 — Documented Allman brace style as this specification's own
  documentation convention (human-directed), with the two structural
  exceptions (table cells, inline prose snippets). Non-normative per
  `spec/02-schema.md` §6 — no `CHG-XXXX` record: `whitespace` was
  already, and remains, insignificant between tokens (§1), so no
  grammar fact changed. Cascaded to `spec/examples.md` and
  `spec/21-standard-library-semantics.md`'s own code, each noting the
  reformatting in its own Change Log.
- 2.7.0 — `spawn`/`join` demoted from dedicated grammar to ordinary
  prelude intrinsics (`CHG-0008`, human-directed). Removed from the
  keyword list and from `primary`'s special-cased alternatives; both
  are now just calls (`e(args)`) to intrinsics defined in `spec/21`
  §0, the same category `drop`/`sizeof`/`allocate` already occupy.
  No program text changes — `spawn(f, args)`/`join(h)` already used
  call syntax, so this is a pure grammar simplification (two fewer
  reserved words, two fewer `primary` special cases), not a
  re-spelling. `rule.conc.spawn`/`rule.conc.join` and their typing/
  reduction rules are completely unchanged; only their classification
  (dedicated syntax vs. intrinsic) moved.
- 2.6.0 — Unit *type* spelled `void` (`CHG-0007`, human-directed;
  concrete syntax only). `type`'s unit alternative changed from
  `'(' ')'` to `'void'` — `fn f(i32 x) : void { ... }` now reads
  exactly like real C. The unit *value* is unaffected: `()` remains
  the only way to write it in expression position, since C has no
  expression-position token to borrow there (`void` is never a value
  in C either). `type.unit`'s id and semantics are unchanged — same
  stability already established for every other renamed keyword.
- 2.5.0 — Module vocabulary finished (`CHG-0006`, human-directed;
  concrete syntax only). `use` renamed `import`, `pub` renamed
  `export` — completing what `CHG-0001` started when `mod` became
  `module`, so the whole module system now consistently mirrors
  C++20's own `module`/`import`/`export` keywords instead of mixing
  that with Rust's `use`/`pub`. `use-decl` renamed `import-decl` for
  the same reason `mod-decl` was renamed `module-decl`. `rule.module.
  use`'s id is unchanged (`spec/17` §3) — the same stability already
  established for `rule.init.let` after `let`'s removal.
- 2.4.0 — Match-arm separator changed from `=>` to `:` (`CHG-0005`,
  human-directed; concrete syntax only). `arm` now reads
  `Vi(x) : expr` / `Vi : expr` / `_ : expr`, matching C's own
  `case X:` idiom instead of Rust/ML's `=>`. No new ambiguity: an arm
  list is already a fully self-contained grammatical context, so `:`
  overloads with the return-type colon only nominally, never in a way
  that requires disambiguation.
- 2.3.0 — Turbofish removed (`CHG-0004`, human-directed; concrete
  syntax only). `type-args` drops the `'::'` token entirely: every
  generic instantiation, intrinsic or not, now uses the one bare
  `<...>` form (`Vec<i32>::new()`, `apply<i32, i32>(f, x)`) instead of
  intrinsics getting a bare-form exception while everything else
  required `::<...>`. Disambiguation (1) restated to justify the bare
  form unconditionally (it always did, for intrinsics; the "followed
  by `(`" qualifier in 2.2.0's wording was narrower than the actual
  reasoning required). No cascade needed in `spec/examples.md`,
  `spec/conformance.md`, or `spec/21` — none of them used the
  `::<...>` form in the first place.
- 2.2.0 — Mandatory condition parentheses (`CHG-0003`, human-directed;
  concrete syntax only). `if`, `while`, and `match` now require
  `(...)` around their condition/scrutinee — `if (c) b`,
  `while (c) b`, `match (e) { … }` — matching C's `if`/`while`/
  `switch`. The old disambiguation (1) ("a struct literal is not
  permitted as the bare condition... parenthesize it") is removed
  outright rather than renumbered-and-kept: mandatory parentheses make
  the ambiguity it worked around impossible, not just resolved.
  Disambiguations (2)-(7) from 2.1.0 are renumbered (1)-(6) — earlier
  Change records and this file's own 2.0.0/2.1.0 changelog entries
  cite the old numbers, which were correct as of those versions and
  are left as historical record, not updated.
- 2.1.0 — C-style closure syntax (`CHG-0002`, human-directed;
  concrete syntax only). `closure` changed from `move? '|' params? '|'
  expr` to `move? '[' captures? ']' '(' params? ')' block` (C++11
  capture-list style); the body is now always a brace block, never a
  bare expression. New disambiguation (7) for the shared `[` with
  array literals. `rule.fn.closure` (`spec/15` §6) gains one new
  static check — the written capture list must match the body's
  derived free-variable set — but capture-mode inference, ownership,
  borrowing, lifetimes, and type inference are unchanged.
- 2.0.0 — C-style declarator syntax (`CHG-0001`, human-directed
  redesign; concrete syntax only, no semantic change). Local
  declarations, parameters, and struct fields flipped to type-first
  order (`i32 x` instead of `x: i32`). `let` removed: local
  declarations are now a bare type-first statement or `auto` for
  inferred locals, disambiguated from expression statements by the
  known-type-name rule (disambiguation (5)). `mod` renamed `module`.
  Every return-type position (`fn-decl`, `extern-decl`, the `fn(...)`
  type constructor) now uses `:` instead of `->`; `->` is no longer a
  token in the grammar. Struct-literal field initializers changed from
  `f: e` to C99-style `.f = e` (disambiguation (6)); struct fields are
  now semicolon-terminated rather than comma-separated. Every
  production still maps to the same rule id as before this revision.
- 1.0.0 — Rewritten (`spec/AUDIT-2.md` B-20, B-21): complete lexical
  and expression grammar with precedence encoded in productions;
  `resource` marker, `?`, `else if`, block-like statements,
  single-field destructuring, `handle`/`mutex`/`guard` types, comment
  syntax; correspondence table; §5 restated as restrictions,
  absences, and implementation-defined choices.
- 0.9.0 and earlier — superseded.
