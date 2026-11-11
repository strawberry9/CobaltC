import Cobalt.Corollaries
import Cobalt.Elision
import Cobalt.Exec
import Cobalt.Statics
/-
  What `proofs/` claims, in one file: each result the development proves, restated with its full statement
  and discharged by the theorem that proves it, then the axioms that theorem rests on. Read this file to
  know what is proved; read the others only to see how.

  - An `example` below fails to compile if its theorem's statement ever changes under it, so a statement
    here cannot drift from the proof it names.
  - Each `#print axioms` is wrapped in `#guard_msgs`: the build fails if a theorem comes to rest on any
    axiom other than those listed, `sorryAx` (an unfinished proof) included. The development uses only
    Lean's core library; the axioms it may rest on are the three of Lean's own logic, `propext`,
    `Classical.choice` and `Quot.sound`, and `impl/tools/proofcover.py --check` fails on any other name
    listed here. The table in `proofs/COVERAGE.md` is written from these blocks.

  What no machine checks: that these statements say what `spec/23` and `spec/08` say (`proofs/README.md`,
  "What is proved, and what is not").
-/
namespace Cobalt
namespace Statements
open State
open Statics

/-! ## The machine: every reachable state satisfies the invariants (`spec/23`) -/

example (σ : State) (h : Reachable σ) : σ.Invariants :=
  reachable_invariants σ h

/-! ## The corollaries `spec/23` §3 draws from the invariants -/

example (σ : State) (h : Reachable σ) (a : Token) (p : PathRecord)
    (hp : σ.paths a = some p) (hv : p.valid = true) : σ.alive p.of :=
  access_reaches_live σ h a p hp hv

example (σ : State) (h : Reachable σ) (a : Token) (p : PathRecord) (hp : σ.paths a = some p)
    (o : Identity) : o ∈ p.heldBy ↔ (σ.alive o ∧ a ∈ σ.contents o) :=
  heldBy_exact σ h a p hp o

example (σ : State) (h : Reachable σ) (o : Identity) (ho : σ.obligation o = true) :
    ∃ r ℓ tr, σ.objects o = some r ∧ r.alive = true ∧ σ.authority o = some ℓ ∧ σ.threads ℓ = some tr ∧ Owner σ o r tr :=
  obligation_owned σ h o ho

example (σ : State) (h : Reachable σ) (o : Identity) (q : List Proj) (ℓ : Thread)
    (hs : σ.subs o q = some ℓ) :
    q ≠ [] ∧ σ.obligation o = true ∧
      ∃ r tr, σ.objects o = some r ∧ r.alive = true ∧ σ.authority o = some ℓ ∧ σ.threads ℓ = some tr ∧ Owner σ o r tr :=
  sub_obligation_owned σ h o q ℓ hs

example (σ : State) (h : Reachable σ) (o : Identity) (r : ObjectRecord)
    (hr : σ.objects o = some r) (hrecl : r.reclaimed = true) (ho : σ.obligation o = true) :
    r.alive = true ∧ ∃ ℓ tr, σ.authority o = some ℓ ∧ σ.threads ℓ = some tr :=
  reclaimed_obligation_owned σ h o r hr hrecl ho

example (σ : State) (ℓ : Thread) (a : Token) (p : PathRecord) (hrun : σ.running ℓ)
    (hp : σ.paths a = some p) (htv : σ.temporallyValid a) (hc : ¬ σ.clash a Mode.shared)
    (haff : p.mode = Mode.exclusive → p.thread = some ℓ) :
    ∃ s : Step σ σ, s = Step.read σ ℓ a p hrun hp htv hc haff ∧ ¬ σ.clash a Mode.shared :=
  read_no_clash σ ℓ a p hrun hp htv hc haff

example (σ : State) (h : Reachable σ) (a b : Token) (p q : PathRecord)
    (hp : σ.paths a = some p) (hq : σ.paths b = some q) (hpl : p.lock = true) (hql : q.lock = true)
    (hpv : p.valid = true) (hqv : q.valid = true) (hof : p.of = q.of) : a = b :=
  one_lock_path σ h a b p q hp hq hpl hql hpv hqv hof

example (σ : State) (h : Reachable σ) (a : Token) (p : PathRecord) (hp : σ.paths a = some p)
    (hpend : p.pending = true) : p.heldBy = [] :=
  pending_unheld σ h a p hp hpend

example (σ : State) (ℓ : Thread) (a a' : Token) (p p' : PathRecord) (m : Mode)
    (hp : σ.paths a = some p) (ha : a < σ.nextId) (hlock : p.lock = false) (hp' : σ.paths a' = some p') (hne : a' ≠ a) (ha' : a' < σ.nextId)
    (hv' : p'.valid = true) (hpend' : p'.pending = false) (hlock' : p'.lock = false) (hof : p'.of = p.of) (hov : overlap p.proj p'.proj)
    (hanc : a' ∉ ancestors (bindParam σ ℓ a) (bindParam σ ℓ a).nextId a) (hperm : ¬ permitted m p'.mode) :
    (bindParam σ ℓ a).clash a m :=
  bound_parameter_clashes σ ℓ a a' p p' m hp ha hlock hp' hne ha' hv' hpend' hlock' hof hov hanc hperm

/-! ## The executable machine (D-0270): the replays run what was proved -/

example (σ : State) (n : Nat) (act : Act) (hinv : σ.Invariants) (hth : ThreadsBelow σ n)
    (hc : check σ n act = true) : Step σ (apply σ act) :=
  exec_sound σ n act hinv hth hc

example (σ σ' : State) (n : Nat) (act : Act) (hr : Reachable σ) (hth : ThreadsBelow σ n)
    (h : exec σ n act = some σ') : Reachable σ' :=
  exec_reachable σ σ' n act hr hth h

/-! ## The elision lemmas (`spec/08` §4 `[Standing-Local]`, D-0200) -/

example (σ σ' : State) (ℓ : Thread) (a : Token) (hrun : OthersRun ℓ a σ σ') :
    ∀ (p : PathRecord) (r : ObjectRecord), Reachable σ → Standing σ ℓ a p r →
      ∃ (p' : PathRecord) (r' : ObjectRecord), Standing σ' ℓ a p' r' ∧ p'.of = p.of ∧ p'.proj = p.proj ∧ p'.mode = p.mode :=
  standing_run σ σ' ℓ a hrun

example (σ : State) (ℓ : Thread) (a a' : Token) (p p' : PathRecord) (r : ObjectRecord) (m' : Mode)
    (hst : Standing σ ℓ a p r) (hp' : σ.paths a' = some p') (hlock' : p'.lock = false) (hne : a ≠ a') (hof : p'.of = p.of)
    (hov : overlap p'.proj p.proj) (hanc : a ∉ ancestors σ σ.nextId a') (hperm : ¬ permitted m' p.mode) :
    σ.clash a' m' :=
  conflict_diagnosed σ ℓ a a' p p' r m' hst hp' hlock' hne hof hov hanc hperm

example (σ : State) (ℓ : Thread) (a a' : Token) (p p' : PathRecord) (r : ObjectRecord)
    (hst : Standing σ ℓ a p r) (hp' : σ.paths a' = some p') (hlock' : p'.lock = false) (hne : a ≠ a') (hof : p'.of = p.of)
    (hov : overlap p'.proj p.proj) (hanc : a ∉ ancestors σ σ.nextId a') : σ.clash a' Mode.exclusive :=
  write_conflict_diagnosed σ ℓ a a' p p' r hst hp' hlock' hne hof hov hanc

example (σ : State) (ℓ : Thread) (a a' : Token) (p p' : PathRecord) (r : ObjectRecord)
    (hst : Standing σ ℓ a p r) (hexcl : p.mode = Mode.exclusive) (hp' : σ.paths a' = some p') (hlock' : p'.lock = false) (hne : a ≠ a')
    (hof : p'.of = p.of) (hov : overlap p'.proj p.proj) (hanc : a ∉ ancestors σ σ.nextId a') : σ.clash a' Mode.shared :=
  read_conflict_diagnosed σ ℓ a a' p p' r hst hexcl hp' hlock' hne hof hov hanc

example (σ σ' : State) (ℓ ℓ' : Thread) (o : Identity) (r : ObjectRecord)
    (hinv : σ.Invariants) (hs : StepBy ℓ' σ σ') (hne : ℓ' ≠ ℓ) (hrunℓ : σ.running ℓ)
    (hr : σ.objects o = some r) (halive : r.alive = true) (hfresh : r.reclaimed = false) (hguard : r.guard = false)
    (hap : ApartObj σ ℓ' o r)
    (hnone : ∀ a p, σ.paths a = some p → p.of = o → p.valid = false) :
    (∀ a p, σ'.paths a = some p → p.of = o → p.valid = false) ∧
      ∃ r' : ObjectRecord, σ'.objects o = some r' ∧ r'.alive = true :=
  unreached_stays σ σ' ℓ ℓ' o r hinv hs hne hrunℓ hr halive hfresh hguard hap hnone

/-! ## The static rules for the core (D-0300, D-0299 step 3) -/

example {F E σ F' E' σ'} (h : Run F E σ F' E' σ') (hD : Describes F E σ)
    (x : Var) (ax : Token) (px : PathRecord) (a : Token) (p : PathRecord) (π : List Proj) (m : Mode)
    (hx : E'.owner x = some ax) (hpx : σ'.paths ax = some px) (hp : σ'.paths a = some p) (hof : p.of = px.of)
    (hproj : p.proj = π) (hanc : a = ax ∨ ax ∈ ancestors σ' σ'.nextId a) (hpr : Proven F' x π m) :
    ¬ σ'.clash a m :=
  run_sound h hD x ax px a p π m hx hpx hp hof hproj hanc hpr

/-! ## The axioms each result rests on -/

/-- info: 'Cobalt.reachable_invariants' depends on axioms: [propext, Classical.choice, Quot.sound] -/
#guard_msgs in #print axioms reachable_invariants
/-- info: 'Cobalt.access_reaches_live' depends on axioms: [propext, Classical.choice, Quot.sound] -/
#guard_msgs in #print axioms access_reaches_live
/-- info: 'Cobalt.heldBy_exact' depends on axioms: [propext, Classical.choice, Quot.sound] -/
#guard_msgs in #print axioms heldBy_exact
/-- info: 'Cobalt.obligation_owned' depends on axioms: [propext, Classical.choice, Quot.sound] -/
#guard_msgs in #print axioms obligation_owned
/-- info: 'Cobalt.sub_obligation_owned' depends on axioms: [propext, Classical.choice, Quot.sound] -/
#guard_msgs in #print axioms sub_obligation_owned
/-- info: 'Cobalt.reclaimed_obligation_owned' depends on axioms: [propext, Classical.choice, Quot.sound] -/
#guard_msgs in #print axioms reclaimed_obligation_owned
/-- info: 'Cobalt.read_no_clash' depends on axioms: [propext, Quot.sound] -/
#guard_msgs in #print axioms read_no_clash
/-- info: 'Cobalt.one_lock_path' depends on axioms: [propext, Classical.choice, Quot.sound] -/
#guard_msgs in #print axioms one_lock_path
/-- info: 'Cobalt.pending_unheld' depends on axioms: [propext, Classical.choice, Quot.sound] -/
#guard_msgs in #print axioms pending_unheld
/-- info: 'Cobalt.bound_parameter_clashes' depends on axioms: [propext] -/
#guard_msgs in #print axioms bound_parameter_clashes
/-- info: 'Cobalt.exec_sound' depends on axioms: [propext, Quot.sound] -/
#guard_msgs in #print axioms exec_sound
/-- info: 'Cobalt.exec_reachable' depends on axioms: [propext, Classical.choice, Quot.sound] -/
#guard_msgs in #print axioms exec_reachable
/-- info: 'Cobalt.standing_run' depends on axioms: [propext, Classical.choice, Quot.sound] -/
#guard_msgs in #print axioms standing_run
/-- info: 'Cobalt.conflict_diagnosed' does not depend on any axioms -/
#guard_msgs in #print axioms conflict_diagnosed
/-- info: 'Cobalt.write_conflict_diagnosed' does not depend on any axioms -/
#guard_msgs in #print axioms write_conflict_diagnosed
/-- info: 'Cobalt.read_conflict_diagnosed' does not depend on any axioms -/
#guard_msgs in #print axioms read_conflict_diagnosed
/-- info: 'Cobalt.unreached_stays' depends on axioms: [propext, Classical.choice, Quot.sound] -/
#guard_msgs in #print axioms unreached_stays
/-- info: 'Cobalt.Statics.run_sound' depends on axioms: [propext, Quot.sound] -/
#guard_msgs in #print axioms run_sound

end Statements
end Cobalt
