# D-0319 — A result that holds references names its sources too

Status: ACCEPTED (2026-11-12, the agent, under the owner's standing instruction for the next push: "I need a
really reliable static checker that programmers can rely on to not accept incorrect code"; the guide and the
book already described the rule this records); implemented by `CHG-0439`
Kind: Decision (`spec/02-schema.md`); one rule's premise widened
Governed by: `CobaltC_Master_Instructions.md` §9, §12, §17
Depends on: D-0011 (single-parameter elision), D-0088 (a result that holds a reference), D-0284 (decided
programs), D-0296 (both toolchains in step), D-0312 (the `from` clause), D-0318 (several sources)
Affects: `spec/10` §3 (`[Call-Multi-Ref-Return-Rejected]`, `[Call-Elided-Lifetime]`), `spec/registry/diagnostics.md`
(`diag.lifetime-elision-ambiguous`), seven `std` signatures, five in `impl/selfhost`, one showcase, both front
ends

## 1. Problem

`[Call-Multi-Ref-Return-Rejected]` asked for a `from` clause only when the result *is* a reference. A function
with two reference parameters whose result *holds* references, an `Option<ref<T, shared>>`, a `Vec<StringView>`,
a struct with a reference field, needed none, and its call was taken to hold what its first reference argument
reaches (D-0088). So

    fn pick(ref<i32, shared> a, ref<i32, shared> b) : Option<ref<i32, shared>>
    {
        if (*a > *b) { Some(a) } else { Some(b) }
    }

    o = pick(&x, &y);       // in a block that ends before `o` is read

was accepted, the program found nothing left to the run, `cobc` built it decided (D-0284), and the compiled
program read `y`'s dead stack slot (`stress/checker_gaps/probes/F23_holder_result_first_arg.cb`). The guide and
the book already said the clause is required here, so the documents described a stricter checker than the one
there was. A clause that *was* written was not read either: `from b` on such a result still borrowed `a`.

## 2. Candidates

1. **Narrow the documents**: say the clause is required only for a reference result, and leave a holder result's
   escapes to the run. Sound only if the call's facts are "unknown", which every such program then pays for at run
   time, and which the analysis did not do (it took the first argument, unsoundly).
2. **Take every reference argument** for a holder result without a clause. Sound, but a lookup's key would stay
   borrowed while the result lives, for every caller of `Json::get(&doc, key)`.
3. **Require the clause**, as for a reference result. **Selected.** The signature says what the call borrows; a
   caller of `StringView::strip_prefix(&s, &prefix)` can change `prefix` while the result is used.

## 3. Decision

`[Call-Multi-Ref-Return-Rejected]` (`spec/10` §3) applies to a function with two or more reference-typed
parameters, no `from` clause, and a result type that may hold a reference without being one (`spec/10`'s "may hold
a reference": a field, an element, a payload, a type argument, a closure's captures):
`diag.lifetime-elision-ambiguous`. A function with no reference parameter may still return such a value (`Vec::new()`, views of static
text); one with exactly one is tied to it, as before (D-0011, D-0088).

`[Call-Elided-Lifetime]` reads a holder result as it reads a reference result: the call borrows each argument the
clause names, for `rule.control.flow-analysis` and for `rule.temporal.ref-escape`, so a holder result stored past
its source's block is `diag.reference-escapes-scope` before the run.

The standard library names its sources: `StringView::strip_prefix` and `strip_suffix` `from v`, `Json::get` and
`Json::lookup` `from j`, `Toml::get` and `Toml::lookup` `from t`, and the private `http_date_words` `from text`.

## 4. Cost

A program with such a function and no clause is refused until the clause is written (the message names the
parameters to choose from). The corpus had one outside `std` (`showcase/tier3/hash_map.cb`'s `Map::get`) and five
in `impl/selfhost`. No run-time cost: the facts are the clause's, and fewer programs are left to the run.

## Revisit conditions

A holder result whose parts come from different arguments, which no clause describes (D-0318 §5).
