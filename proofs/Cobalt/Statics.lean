import Cobalt.Preservation
/-
  D-0300 (D-0299 step 3): the static rules for a core of the fragment, stated against the proven
  machine. `rule.control.flow-analysis` and `rule.control.frame-references` (spec/14 §6) decide, before the
  run, that an access cannot clash; a decided body's compiled form then makes no check there (D-0284,
  D-0292). This file states what such a decision means as a property of the machine's states, and proves
  it for the rows of the table the core uses.

  The facts are `deriv(r, x.π, m)` (`Fact`): the reference binding `r` may hold a path derived from the
  owner `x`'s object at projection `π` in mode `m`. An environment (`Env`) says which path each binding
  carries: an owner its root, a reference binding the path its value is. The abstraction `Describes F E σ`
  is the facts' meaning: every valid, non-pending path on an owner's object, other than the owner's root
  and the lock paths `sync-exempt` sets aside, is the path some reference binding carries, and a fact says
  so with that path's projection and mode. (`escaped(x) = F` is what makes this so for `x`: no reference to
  `x`'s object went anywhere the analysis does not follow.)

  `proven_no_clash` is the discharge row: an access at `x.π` in mode `m` that the table proves (no fact on
  `x` overlaps `π` in a mode `m` does not permit) passes `¬clash` in any state the facts describe, so the
  machine admits it with no check made. The row lemmas are the transfer functions: a read and a write keep
  the description; a reference binding's end (`[Ref-Binding-Last-Use]`, the fragment's `dead` step) removes
  its facts; a borrow of an owner, or of one of its fields, adds the borrow's fact; more facts describe a
  state as well as fewer (`describes_mono`), which is the join at a merge and a loop's post-fixed point.
  `run_sound` composes them over a straight-line run of core commands.

  Outside this file, for now: reborrows through a reference (facts rooted at `*q`), copies of references
  (inherited facts), calls (the call-site rule and its summaries), threads (scoped lending), containers'
  elements, and the block's-end rule; `proofs/COVERAGE.md`'s table of static rows lists them.
-/
namespace Cobalt
namespace Statics
open State

abbrev Var := Nat

/-- `deriv(r, x.π, m)` (spec/14 §6). -/
structure Fact where
  r : Var
  x : Var
  proj : List Proj
  mode : Mode
  deriving DecidableEq

/-- The path each binding carries: an owner's root path, a reference binding's path. -/
structure Env where
  owner : Var → Option Token
  ref : Var → Option Token

/-- The facts describe the state (see the header). -/
def Describes (F : List Fact) (E : Env) (σ : State) : Prop :=
  ∀ x ax px, E.owner x = some ax → σ.paths ax = some px →
    ∀ a' p', σ.paths a' = some p' → p'.valid = true → p'.pending = false → p'.lock = false →
      p'.of = px.of → a' ≠ ax →
      ∃ f ∈ F, f.x = x ∧ E.ref f.r = some a' ∧ f.proj = p'.proj ∧ f.mode = p'.mode

/-- The discharge table's proven `¬clash` row for an access at `x.π` in mode `m`: every fact on `x` that
    overlaps `π` is in a mode `m` permits. -/
def Proven (F : List Fact) (x : Var) (π : List Proj) (m : Mode) : Prop :=
  ∀ f ∈ F, f.x = x → overlap π f.proj → permitted m f.mode

/-! ### the discharge -/

/-- An access the table proves passes `¬clash`: through the owner's root (`a = ax`) or through a path
    formed from it (`ax` among its ancestors), at the place the access names. -/
theorem proven_no_clash (F : List Fact) (E : Env) (σ : State) (x : Var) (ax : Token) (px : PathRecord)
    (a : Token) (p : PathRecord) (π : List Proj) (m : Mode)
    (hD : Describes F E σ) (hx : E.owner x = some ax) (hpx : σ.paths ax = some px)
    (hp : σ.paths a = some p) (hof : p.of = px.of) (hproj : p.proj = π)
    (hanc : a = ax ∨ ax ∈ ancestors σ σ.nextId a) (hpr : Proven F x π m) : ¬ σ.clash a m := by
  rintro ⟨a', q, q', hq, hq', hne, hv, hpend, _, hl', hofeq, hov, hna, hperm⟩
  rw [hp] at hq
  cases hq
  have hne' : a' ≠ ax := by
    intro h
    subst h
    rcases hanc with h | h
    · exact hne h.symm
    · exact hna h
  obtain ⟨f, hf, hfx, _, hfp, hfm⟩ := hD x ax px hx hpx a' q' hq' hv hpend hl' (by rw [hofeq, hof]) hne'
  exact absurd (by rw [← hfm]; exact hpr f hf hfx (by rw [hfp, ← hproj]; exact hov)) hperm

/-! ### the rows -/

/-- More facts describe a state as well as fewer: the join at a merge, and a loop's post-fixed point. -/
theorem describes_mono (F F' : List Fact) (E : Env) (σ : State) (hD : Describes F E σ) (hsub : ∀ f ∈ F, f ∈ F') :
    Describes F' E σ := by
  intro x ax px hx hpx a' p' hp' hv hpend hl hof hne
  obtain ⟨f, hf, h1, h2, h3, h4⟩ := hD x ax px hx hpx a' p' hp' hv hpend hl hof hne
  exact ⟨f, hsub f hf, h1, h2, h3, h4⟩

/-- A read changes no component of σ. -/
theorem row_read (F : List Fact) (E : Env) (σ : State) (hD : Describes F E σ) : Describes F E σ := hD

/-- A step that changes no path's validity, pendingness, lock flag, object, projection or mode keeps the
    description. -/
theorem describes_of_fields (F : List Fact) (E : Env) (σ σ' : State) (hD : Describes F E σ)
    (hfields : ∀ t p', σ'.paths t = some p' → ∃ p, σ.paths t = some p ∧ p.valid = p'.valid ∧ p.pending = p'.pending ∧
      p.lock = p'.lock ∧ p.of = p'.of ∧ p.proj = p'.proj ∧ p.mode = p'.mode) :
    Describes F E σ' := by
  intro x ax px' hx hpx' a' p' hp' hv hpend hl hof hne
  obtain ⟨px, hpx, _, _, _, hpxof, _, _⟩ := hfields ax px' hpx'
  obtain ⟨p, hp, hpv, hpp, hpl, hpo, hpj, hpm⟩ := hfields a' p' hp'
  obtain ⟨f, hf, h1, h2, h3, h4⟩ := hD x ax px hx hpx a' p hp (by rw [hpv]; exact hv) (by rw [hpp]; exact hpend)
    (by rw [hpl]; exact hl) (by rw [hpo, hof, hpxof]) hne
  exact ⟨f, hf, h1, h2, by rw [h3, hpj], by rw [h4, hpm]⟩

/-- A write moves references between slots (`held-by`) and nothing else. -/
theorem row_write (F : List Fact) (E : Env) (σ : State) (o : Identity) (removed added : List Token)
    (hD : Describes F E σ) : Describes F E (write σ o removed added) := by
  apply describes_of_fields F E σ _ hD
  intro t p' hp'
  simp only [write, writeWith] at hp'
  cases hq : σ.paths t with
  | none => rw [hq] at hp'; cases hp'
  | some q =>
    rw [hq] at hp'
    simp at hp'
    subst hp'
    exact ⟨q, rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩

/-- `[Ref-Binding-Last-Use]` (D-0111): the reference binding `r` ends after its last use, and its path with
    it (`[Object-End]`'s `Dead`, the fragment's `dead` step); its facts go. -/
theorem row_end (F : List Fact) (E : Env) (σ : State) (r : Var) (b : Token)
    (hD : Describes F E σ) (hb : E.ref r = some b) :
    Describes (F.filter (fun f => f.r ≠ r)) E (dead σ b) := by
  intro x ax px' hx hpx' a' p' hp' hv hpend hl hof hne
  have hpaths : ∀ t q, (dead σ b).paths t = some q → ∃ q0, σ.paths t = some q0 ∧ q0.of = q.of ∧ (t ≠ b → q0 = q) := by
    intro t q hq
    simp only [dead] at hq
    split at hq
    · cases h0 : σ.paths t with
      | none => rw [h0] at hq; cases hq
      | some q0 =>
        rw [h0] at hq
        simp at hq
        subst hq
        exact ⟨q0, rfl, rfl, fun hne => absurd ‹t = b› hne⟩
    · exact ⟨q, hq, rfl, fun _ => rfl⟩
  have hab : a' ≠ b := by
    intro h
    subst h
    simp [dead] at hp'
    obtain ⟨q0, _, rfl⟩ := hp'
    cases hv
  obtain ⟨px, hpx, hpxof, _⟩ := hpaths ax px' hpx'
  obtain ⟨p, hp, _, hpe⟩ := hpaths a' p' hp'
  have hpp : p = p' := hpe hab
  subst hpp
  obtain ⟨f, hf, h1, h2, h3, h4⟩ := hD x ax px hx hpx a' p hp hv hpend hl (by rw [hof, ← hpxof]) hne
  refine ⟨f, ?_, h1, h2, h3, h4⟩
  apply List.mem_filter.mpr
  refine ⟨hf, ?_⟩
  apply decide_eq_true
  intro hfr
  rw [hfr, hb] at h2
  exact hab (Option.some.inj h2).symm

/-- `auto r = &_m x;` (or `r = &_m x` once `r`'s old path ended): `[Borrow]` from the owner's root, the new
    path the next token. The fact `deriv(r, x, m)` describes it; `r` held no fact before. The owners' roots
    are in use (below `nextId`), and no other owner's object is `x`'s. -/
theorem row_borrow (F : List Fact) (E : Env) (σ : State) (ℓ : Thread) (r x : Var) (ax : Token) (px : PathRecord)
    (m : Mode) (hD : Describes F E σ) (_hx : E.owner x = some ax) (_hpx : σ.paths ax = some px)
    (hfo : ∀ v t, E.owner v = some t → t < σ.nextId) (hnor : ∀ f ∈ F, f.r ≠ r)
    (hsep : ∀ y ay py, E.owner y = some ay → σ.paths ay = some py → py.of = px.of → y = x) :
    Describes (⟨r, x, px.proj, m⟩ :: F) { E with ref := fun v => if v = r then some σ.nextId else E.ref v }
      (borrow σ ℓ ax px m false) := by
  intro y ay py hy hpy a' p' hp' hv hpend hl hof hne
  simp only [borrow] at hpy hp'
  have hay : ay ≠ σ.nextId := Nat.ne_of_lt (hfo y ay hy)
  simp [hay] at hpy
  by_cases ha : a' = σ.nextId
  · subst ha
    simp at hp'
    subst hp'
    have hyx : y = x := hsep y ay py hy hpy hof.symm
    subst hyx
    exact ⟨⟨r, y, px.proj, m⟩, List.Mem.head _, rfl, by simp, rfl, rfl⟩
  · simp [ha] at hp'
    obtain ⟨f, hf, h1, h2, h3, h4⟩ := hD y ay py hy hpy a' p' hp' hv hpend hl hof hne
    refine ⟨f, List.Mem.tail _ hf, h1, ?_, h3, h4⟩
    have hfr : f.r ≠ r := hnor f hf
    simp [hfr, h2]

/-- The pending projection path `[Field-Access]` forms from the owner's root (`project`'s record). -/
def fieldPath (σ : State) (ℓ : Thread) (ax : Token) (px : PathRecord) (i : Nat) : PathRecord :=
  { px with proj := px.proj ++ [Proj.field i], valid := true, base := some ax, thread := some ℓ, lock := false, taken := false, pending := true, frame := σ.frame ℓ, scope := σ.scope ℓ, heldBy := [] }

/-- `auto r = &_m x.f;`: `[Field-Access]` from the owner's root (a pending projection path), `[Borrow]` from
    it, and the projection's one access made (`settle`, D-0242). The fact `deriv(r, x.f, m)` describes the
    borrow's path; the projection's is invalid. -/
theorem row_borrow_field (F : List Fact) (E : Env) (σ : State) (ℓ : Thread) (r x : Var) (ax : Token) (px : PathRecord)
    (i : Nat) (m : Mode) (hD : Describes F E σ)
    (hfo : ∀ v t, E.owner v = some t → t < σ.nextId) (hnor : ∀ f ∈ F, f.r ≠ r)
    (hsep : ∀ y ay py, E.owner y = some ay → σ.paths ay = some py → py.of = px.of → y = x) :
    Describes (⟨r, x, px.proj ++ [Proj.field i], m⟩ :: F)
      { E with ref := fun v => if v = r then some (σ.nextId + 1) else E.ref v }
      (settle (borrow (project σ ℓ ax px (Proj.field i)) ℓ σ.nextId (fieldPath σ ℓ ax px i) m false) σ.nextId) := by
  intro y ay py hy hpy a' p' hp' hv hpend hl hof hne
  have hay0 : ay ≠ σ.nextId := Nat.ne_of_lt (hfo y ay hy)
  have hay1 : ay ≠ σ.nextId + 1 := Nat.ne_of_lt (Nat.lt_succ_of_lt (hfo y ay hy))
  simp only [settle, borrow, project, fieldPath] at hpy hp'
  simp [hay0, hay1] at hpy
  by_cases h0 : a' = σ.nextId
  · subst h0
    simp at hp'
    obtain ⟨q, _, rfl⟩ := hp'
    cases hv
  · by_cases h1 : a' = σ.nextId + 1
    · subst h1
      simp at hp'
      subst hp'
      have hyx : y = x := hsep y ay py hy hpy hof.symm
      subst hyx
      exact ⟨⟨r, y, px.proj ++ [Proj.field i], m⟩, List.Mem.head _, rfl, by simp, rfl, rfl⟩
    · simp [h0, h1] at hp'
      obtain ⟨f, hf, h2, h3, h4, h5⟩ := hD y ay py hy hpy a' p' hp' hv hpend hl hof hne
      refine ⟨f, List.Mem.tail _ hf, h2, ?_, h4, h5⟩
      have hfr : f.r ≠ r := hnor f hf
      simp [hfr, h3]

/-! ### a run of core commands -/

/-- A straight-line run of the core's commands from `(F, E, σ)` to `(F', E', σ')`: each command its machine
    steps and the table's transfer of the facts. A merge or a loop's head is `describes_mono`. -/
inductive Run : List Fact → Env → State → List Fact → Env → State → Prop where
  | done (F : List Fact) (E : Env) (σ : State) : Run F E σ F E σ
  | read (F : List Fact) (E : Env) (σ : State) {F' E' σ'} :
      Run F E σ F' E' σ' → Run F E σ F' E' σ'
  | write (F : List Fact) (E : Env) (σ : State) (o : Identity) (removed added : List Token) {F' E' σ'} :
      Run F E (write σ o removed added) F' E' σ' → Run F E σ F' E' σ'
  | stop (F : List Fact) (E : Env) (σ : State) (r : Var) (b : Token) {F' E' σ'} :
      E.ref r = some b → Run (F.filter (fun f => f.r ≠ r)) E (dead σ b) F' E' σ' → Run F E σ F' E' σ'
  | borrow (F : List Fact) (E : Env) (σ : State) (ℓ : Thread) (r x : Var) (ax : Token) (px : PathRecord) (m : Mode)
      {F' E' σ'} :
      E.owner x = some ax → σ.paths ax = some px → (∀ v t, E.owner v = some t → t < σ.nextId) → (∀ f ∈ F, f.r ≠ r) →
      (∀ y ay py, E.owner y = some ay → σ.paths ay = some py → py.of = px.of → y = x) →
      Run (⟨r, x, px.proj, m⟩ :: F) { E with ref := fun v => if v = r then some σ.nextId else E.ref v }
        (borrow σ ℓ ax px m false) F' E' σ' →
      Run F E σ F' E' σ'
  | weaken (F F2 : List Fact) (E : Env) (σ : State) {F' E' σ'} :
      (∀ f ∈ F, f ∈ F2) → Run F2 E σ F' E' σ' → Run F E σ F' E' σ'

/-- The facts the table computes along a run describe the state the run reaches. -/
theorem run_describes {F E σ F' E' σ'} (h : Run F E σ F' E' σ') (hD : Describes F E σ) : Describes F' E' σ' := by
  induction h with
  | done => exact hD
  | read _ _ _ _ ih => exact ih hD
  | write F E σ o removed added _ ih => exact ih (row_write F E σ o removed added hD)
  | stop F E σ r b hb _ ih => exact ih (row_end F E σ r b hD hb)
  | borrow F E σ ℓ r x ax px m hx hpx hfo hnor hsep _ ih => exact ih (row_borrow F E σ ℓ r x ax px m hD hx hpx hfo hnor hsep)
  | weaken F F2 E σ hsub _ ih => exact ih (describes_mono F F2 E σ hD hsub)

/-- The theorem of D-0299's step 3 for the core: after any run from a state the facts describe, an access
    the table proves at the place it names passes `¬clash` — the machine admits it with no check, as a
    decided body's compiled form makes none. -/
theorem run_sound {F E σ F' E' σ'} (h : Run F E σ F' E' σ') (hD : Describes F E σ)
    (x : Var) (ax : Token) (px : PathRecord) (a : Token) (p : PathRecord) (π : List Proj) (m : Mode)
    (hx : E'.owner x = some ax) (hpx : σ'.paths ax = some px) (hp : σ'.paths a = some p) (hof : p.of = px.of)
    (hproj : p.proj = π) (hanc : a = ax ∨ ax ∈ ancestors σ' σ'.nextId a) (hpr : Proven F' x π m) :
    ¬ σ'.clash a m :=
  proven_no_clash F' E' σ' x ax px a p π m (run_describes h hD) hx hpx hp hof hproj hanc hpr

/-- A state with no reference binding and no path but the owners' roots is described by no facts: where a
    body begins, before it forms a reference. -/
theorem describes_start (E : Env) (σ : State)
    (honly : ∀ a' p', σ.paths a' = some p' → p'.valid = true → p'.pending = false → p'.lock = false →
      ∀ x ax px, E.owner x = some ax → σ.paths ax = some px → p'.of = px.of → a' = ax) :
    Describes [] E σ := by
  intro x ax px hx hpx a' p' hp' hv hpend hl hof hne
  exact absurd (honly a' p' hp' hv hpend hl x ax px hx hpx hof) hne

end Statics
end Cobalt
