import Cobalt.Preservation
/-
  What the invariants give an access in a reachable state, and the adequacy
  of `clash`'s fuel (the ancestor walk is bounded by lemma (A)).
-/
namespace Cobalt
open State

/-- An access through a temporally valid path reaches a live object (`inv.temporal-validity`'s
    consequence for the fragment: no path to an ended object is ever valid). -/
theorem access_reaches_live (σ : State) (h : Reachable σ) (a : Token) (p : PathRecord)
    (hp : σ.paths a = some p) (hv : p.valid = true) : σ.alive p.of :=
  (reachable_invariants σ h).1 a p hp hv

/-- `held-by` is exactly the live holders (`spec/04` §1's definition, kept by every step). -/
theorem heldBy_exact (σ : State) (h : Reachable σ) (a : Token) (p : PathRecord) (hp : σ.paths a = some p)
    (o : Identity) : o ∈ p.heldBy ↔ (σ.alive o ∧ a ∈ σ.contents o) :=
  (reachable_invariants σ h).2.1 a p hp o

/-- Every obligation is alive, has exactly one authority thread, and is owned (`inv.resource-authority`). -/
theorem obligation_owned (σ : State) (h : Reachable σ) (o : Identity) (ho : σ.obligation o = true) :
    ∃ r ℓ tr, σ.objects o = some r ∧ r.alive = true ∧ σ.authority o = some ℓ ∧ σ.threads ℓ = some tr ∧ Owner σ o r tr :=
  (reachable_invariants σ h).2.2.2.2.1 o ho

/-- The `read` and `write` steps are admitted only with `¬clash` (`inv.alias-validity`, D-0018): in a
    reachable state no access is performed while an unpermitted valid non-ancestor path overlaps it, by
    the steps' own premises. Stated as the inversion: a step that leaves the state unchanged and is a
    read of `a` carries `¬clash`. -/
theorem read_no_clash (σ : State) (ℓ : Thread) (a : Token) (p : PathRecord) (hrun : σ.running ℓ)
    (hp : σ.paths a = some p) (htv : σ.temporallyValid a) (hc : ¬ σ.clash a Mode.shared)
    (haff : p.mode = Mode.exclusive → p.thread = some ℓ) :
    ∃ s : Step σ σ, s = Step.read σ ℓ a p hrun hp htv hc haff ∧ ¬ σ.clash a Mode.shared :=
  ⟨Step.read σ ℓ a p hrun hp htv hc haff, rfl, hc⟩

/-- At most one lock path of a mutex is valid at a time (`inv.concurrency-validity`'s one-holder
    consequence): two valid lock paths of one object are the same token. -/
theorem one_lock_path (σ : State) (h : Reachable σ) (a b : Token) (p q : PathRecord)
    (hp : σ.paths a = some p) (hq : σ.paths b = some q) (hpl : p.lock = true) (hql : q.lock = true)
    (hpv : p.valid = true) (hqv : q.valid = true) (hof : p.of = q.of) : a = b := by
  have h2 := (reachable_invariants σ h).2.2.2.2.2.2
  obtain ⟨ℓa, _, ha⟩ := h2 a p hp hpl hpv
  obtain ⟨ℓb, _, hb⟩ := h2 b q hq hql hqv
  rw [hof, hb] at ha
  exact (SyncState.locked.inj ha).2.symm

/-! ### the ancestor walk's fuel -/

/-- Under (A), `ancestors` with fuel at least the token's own value lists the whole chain: a base's
    token is smaller, so the chain from `a` has at most `a` links. More fuel adds nothing. -/
theorem ancestors_fuel_adequate (σ : State) (hA : σ.A) :
    ∀ (fuel : Nat) (a : Token), a ≤ fuel → ∀ extra, ancestors σ (fuel + extra) a = ancestors σ fuel a := by
  intro fuel
  induction fuel with
  | zero =>
    intro a ha extra
    have ha0 : a = 0 := Nat.le_zero.mp ha
    subst ha0
    cases extra with
    | zero => rfl
    | succ e =>
      rw [Nat.zero_add]
      simp only [ancestors]
      cases hp : σ.paths 0 with
      | none => rfl
      | some p =>
        cases hb : p.base with
        | none => simp [hb]
        | some b => exact absurd (hA 0 p b hp hb).1 (Nat.not_lt_zero b)
  | succ n ih =>
    intro a ha extra
    cases extra with
    | zero => rfl
    | succ e =>
      have h1 : n + 1 + (e + 1) = (n + (e + 1)) + 1 := by omega
      rw [h1]
      simp only [ancestors]
      cases hp : σ.paths a with
      | none => rfl
      | some p =>
        cases hb : p.base with
        | none => simp [hb]
        | some b =>
          obtain ⟨hlt, _⟩ := hA a p b hp hb
          have hbn : b ≤ n := Nat.le_of_lt_succ (Nat.lt_of_lt_of_le hlt ha)
          simp [hb]
          exact ih b hbn (e + 1)

/-- So `clash`'s walk with fuel `nextId` is complete under (A) and (Fresh): every path's token is below
    `nextId`, and the walk from it needs no more than its own token's worth of fuel. -/
theorem ancestors_nextId_complete (σ : State) (hA : σ.A) (hf : σ.Fresh) (a : Token) (p : PathRecord)
    (hp : σ.paths a = some p) (more : Nat) : ancestors σ (σ.nextId + more) a = ancestors σ σ.nextId a :=
  ancestors_fuel_adequate σ hA σ.nextId a (Nat.le_of_lt (hf.1 a p hp)) more

end Cobalt
