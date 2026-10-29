import Cobalt.Step
/-
  Preservation: each predicate survives each step, by cases on the step.
-/
namespace Cobalt
open State

/-! ### helpers -/

theorem fresh_obj_none (σ : State) (hf : σ.Fresh) : σ.objects σ.nextId = none := by
  cases h : σ.objects σ.nextId with
  | none => rfl
  | some r => exact absurd (hf.2.1 _ _ h) (Nat.lt_irrefl _)

theorem obligation_ne_next (σ : State) (hf : σ.Fresh) (hR : σ.R) (o : Identity) (ho : σ.obligation o = true) :
    o ≠ σ.nextId := by
  intro heq
  obtain ⟨r, _, _, hr, _⟩ := hR o ho
  rw [heq, fresh_obj_none σ hf] at hr
  cases hr

theorem head_mem {α : Type} (l : List α) (d : α) (h : l ≠ []) : l.headD d ∈ l := by
  cases l with
  | nil => exact absurd rfl h
  | cons x xs => simp

theorem mem_tail_of_ne {α : Type} [DecidableEq α] (l : List α) (d x : α) (h : l ≠ []) (hm : x ∈ l) (hne : x ≠ l.headD d) : x ∈ l.tail := by
  cases l with
  | nil => exact absurd rfl h
  | cons y ys =>
    simp at hm hne
    rcases hm with rfl | hm
    · exact absurd rfl hne
    · simpa using hm

theorem frames_of (σ : State) (ℓ : Thread) (tr : ThreadRec) (h : σ.threads ℓ = some tr) : σ.frames ℓ = tr.frames := by
  simp [State.frames, h]

theorem scopes_of (σ : State) (ℓ : Thread) (tr : ThreadRec) (h : σ.threads ℓ = some tr) : σ.scopes ℓ = tr.scopes := by
  simp [State.scopes, h]

theorem Owner_mono (σ σ' : State) (o : Identity) (r : ObjectRecord) (tr : ThreadRec)
    (hob : σ.obligation tr.handle = true → σ'.obligation tr.handle = true) : Owner σ o r tr → Owner σ' o r tr := by
  intro h
  rcases h with h1 | h2 | ⟨res, hd, hm, hb⟩ | h4
  · exact Or.inl h1
  · exact Or.inr (Or.inl h2)
  · exact Or.inr (Or.inr (Or.inl ⟨res, hd, hm, hob hb⟩))
  · exact Or.inr (Or.inr (Or.inr h4))

/-! ### a transformation that keeps the path, content and liveness structure keeps T1, T2, A, Fresh -/

structure Core (σ σ' : State) : Prop where
  paths : σ'.paths = σ.paths
  contents : σ'.contents = σ.contents
  sync : σ'.sync = σ.sync
  nextId : σ'.nextId = σ.nextId
  alive : ∀ i, σ'.alive i ↔ σ.alive i
  dom : ∀ i r, σ'.objects i = some r → ∃ r', σ.objects i = some r'

theorem Core.trans {σ σ1 σ2 : State} (c1 : Core σ σ1) (c2 : Core σ1 σ2) : Core σ σ2 where
  paths := c2.paths.trans c1.paths
  contents := c2.contents.trans c1.contents
  sync := c2.sync.trans c1.sync
  nextId := c2.nextId.trans c1.nextId
  alive := fun i => (c2.alive i).trans (c1.alive i)
  dom := fun i r h => by
    obtain ⟨r1, h1⟩ := c2.dom i r h
    exact c1.dom i r1 h1

theorem Core.C1 {σ σ' : State} (c : Core σ σ') (h : σ.C1) : σ'.C1 := by
  intro o ℓ a hs
  rw [c.sync] at hs
  rw [c.paths]
  exact h o ℓ a hs

theorem Core.C2 {σ σ' : State} (c : Core σ σ') (h : σ.C2) : σ'.C2 := by
  intro a p hp hl hv
  rw [c.paths] at hp
  rw [c.sync]
  exact h a p hp hl hv

theorem Core.T1 {σ σ' : State} (c : Core σ σ') (h : σ.T1) : σ'.T1 := by
  intro a p hp hv
  rw [c.paths] at hp
  rw [c.alive]
  exact h a p hp hv

theorem Core.T2 {σ σ' : State} (c : Core σ σ') (h : σ.T2) : σ'.T2 := by
  intro a p hp o
  rw [c.paths] at hp
  rw [c.alive, c.contents]
  exact h a p hp o

theorem Core.A {σ σ' : State} (c : Core σ σ') (h : σ.A) : σ'.A := by
  intro a p b hp hb
  rw [c.paths] at hp
  rw [c.paths]
  exact h a p b hp hb

theorem Core.Fresh {σ σ' : State} (c : Core σ σ') (hf : σ.Fresh) : σ'.Fresh := by
  refine ⟨?_, ?_, ?_⟩
  · intro a p hp
    rw [c.paths] at hp
    rw [c.nextId]
    exact hf.1 a p hp
  · intro o r ho
    rw [c.nextId]
    obtain ⟨r', hr'⟩ := c.dom o r ho
    exact hf.2.1 o r' hr'
  · intro o t ht
    rw [c.contents] at ht
    rw [c.nextId]
    exact hf.2.2 o t ht

theorem core_restamp (σ : State) (sel : Identity → Bool) (fr : Option Frame) (sc : Option Scope) :
    Core σ (restamp σ sel fr sc) where
  paths := rfl
  contents := rfl
  sync := rfl
  nextId := rfl
  alive := by
    intro i
    constructor
    · intro ⟨r, hr, ha⟩
      by_cases hs : sel i = true
      · simp [restamp, hs] at hr
        obtain ⟨q, hq, rfl⟩ := hr
        exact ⟨q, hq, ha⟩
      · exact ⟨r, by simpa [restamp, hs] using hr, ha⟩
    · intro ⟨r, hr, ha⟩
      by_cases hs : sel i = true
      · exact ⟨{ r with frame := fr, scope := sc.getD r.scope }, by simp [restamp, hs, hr], ha⟩
      · exact ⟨r, by simp [restamp, hs, hr], ha⟩
  dom := by
    intro i r hr
    by_cases hs : sel i = true
    · simp [restamp, hs] at hr
      obtain ⟨q, hq, _⟩ := hr
      exact ⟨q, hq⟩
    · exact ⟨r, by simpa [restamp, hs] using hr⟩

theorem core_rekey (σ : State) (sel : Identity → Bool) (ℓ : Thread) : Core σ (rekey σ sel ℓ) where
  paths := rfl
  contents := rfl
  sync := rfl
  nextId := rfl
  alive := fun _ => Iff.rfl
  dom := fun _ r h => ⟨r, h⟩

theorem core_setThread (σ : State) (ℓ : Thread) (tr : Option ThreadRec) : Core σ (setThread σ ℓ tr) where
  paths := rfl
  contents := rfl
  sync := rfl
  nextId := rfl
  alive := fun _ => Iff.rfl
  dom := fun _ r h => ⟨r, h⟩

theorem core_mapThread (σ : State) (ℓ : Thread) (f : ThreadRec → ThreadRec) : Core σ (mapThread σ ℓ f) where
  paths := rfl
  contents := rfl
  sync := rfl
  nextId := rfl
  alive := fun _ => Iff.rfl
  dom := fun _ r h => ⟨r, h⟩

/-! ### C1 and C2 under the common path updates -/

-- A new path at `nextId`, not a lock path: (C) is about the old paths.
theorem C_newpath (σ : State) (p : PathRecord) (hl : p.lock = false) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) :
    (∀ o ℓ a, σ.sync o = SyncState.locked ℓ a →
      ∃ q, (if a = σ.nextId then some p else σ.paths a) = some q ∧ q.lock = true ∧ q.of = o ∧ q.thread = some ℓ ∧ q.mode = Mode.exclusive) ∧
    (∀ a q, (if a = σ.nextId then some p else σ.paths a) = some q → q.lock = true → q.valid = true →
      ∃ ℓ, q.thread = some ℓ ∧ σ.sync q.of = SyncState.locked ℓ a) := by
  constructor
  · intro o ℓ a hs
    obtain ⟨q, hq, rest⟩ := h1 o ℓ a hs
    have hne : a ≠ σ.nextId := Nat.ne_of_lt (hf.1 a q hq)
    exact ⟨q, by simp [hne, hq], rest⟩
  · intro a q hq hl' hv
    by_cases hae : a = σ.nextId
    · subst hae
      simp at hq
      subst hq
      rw [hl] at hl'
      cases hl'
    · simp [hae] at hq
      exact h2 a q hq hl' hv

-- A map over the paths that keeps `lock`, `of`, `thread`, `mode` and only lowers `valid`.
theorem C_pathmap (σ : State) (f : Token → PathRecord → PathRecord)
    (hkeep : ∀ t q, (f t q).lock = q.lock ∧ (f t q).of = q.of ∧ (f t q).thread = q.thread ∧ (f t q).mode = q.mode)
    (hv : ∀ t q, (f t q).valid = true → q.valid = true) (h1 : σ.C1) (h2 : σ.C2) :
    (∀ o ℓ a, σ.sync o = SyncState.locked ℓ a →
      ∃ q, (σ.paths a).map (f a) = some q ∧ q.lock = true ∧ q.of = o ∧ q.thread = some ℓ ∧ q.mode = Mode.exclusive) ∧
    (∀ a q, (σ.paths a).map (f a) = some q → q.lock = true → q.valid = true →
      ∃ ℓ, q.thread = some ℓ ∧ σ.sync q.of = SyncState.locked ℓ a) := by
  constructor
  · intro o ℓ a hs
    obtain ⟨q, hq, hl, ho, ht, hm⟩ := h1 o ℓ a hs
    obtain ⟨k1, k2, k3, k4⟩ := hkeep a q
    exact ⟨f a q, by simp [hq], by rw [k1, hl], by rw [k2, ho], by rw [k3, ht], by rw [k4, hm]⟩
  · intro a q hq hl hv'
    simp at hq
    obtain ⟨r, hr, rfl⟩ := hq
    obtain ⟨k1, k2, k3, _⟩ := hkeep a r
    rw [k2, k3]
    exact h2 a r hr (by rw [← k1]; exact hl) (hv a r hv')

theorem succ_not_lt (n : Nat) (h : n + 1 < n) : False :=
  absurd (Nat.lt_trans (Nat.lt_succ_self n) h) (Nat.lt_irrefl n)

/-! ### establish -/

theorem alive_establish_iff (σ : State) (ℓ : Thread) (r : Bool) (hf : σ.Fresh) (i : Identity) :
    (establish σ ℓ r).alive i ↔ (σ.alive i ∨ i = σ.nextId) := by
  constructor
  · intro ⟨q, hq, ha⟩
    by_cases hie : i = σ.nextId
    · exact Or.inr hie
    · exact Or.inl ⟨q, by simpa [establish, hie] using hq, ha⟩
  · intro h
    rcases h with ⟨q, hq, ha⟩ | rfl
    · have hne : i ≠ σ.nextId := by
        intro heq
        rw [heq, fresh_obj_none σ hf] at hq
        cases hq
      exact ⟨q, by simp [establish, hne, hq], ha⟩
    · exact ⟨{ alive := true, isResource := r, guard := false, reclaimed := false, frame := none, scope := σ.scope ℓ }, by simp [establish], rfl⟩

theorem T1_establish (σ : State) (ℓ : Thread) (r : Bool) (h : σ.T1) (hf : σ.Fresh) : (establish σ ℓ r).T1 := by
  intro a p hp hv
  have hp' : σ.paths a = some p := hp
  rw [alive_establish_iff σ ℓ r hf]
  exact Or.inl (h a p hp' hv)

theorem T2_establish (σ : State) (ℓ : Thread) (r : Bool) (h : σ.T2) (hf : σ.Fresh) : (establish σ ℓ r).T2 := by
  intro a p hp o
  have hp' : σ.paths a = some p := hp
  rw [alive_establish_iff σ ℓ r hf]
  by_cases hoe : o = σ.nextId
  · subst hoe
    have hnot : σ.nextId ∉ p.heldBy := by
      intro hm
      obtain ⟨⟨q, hq, _⟩, _⟩ := (h a p hp' σ.nextId).1 hm
      rw [fresh_obj_none σ hf] at hq
      cases hq
    simp [establish, hnot]
  · have hc : (establish σ ℓ r).contents o = σ.contents o := by simp [establish, hoe]
    rw [hc, h a p hp' o]
    constructor
    · intro ⟨ha, hm⟩
      exact ⟨Or.inl ha, hm⟩
    · intro ⟨ha, hm⟩
      rcases ha with ha | ha
      · exact ⟨ha, hm⟩
      · exact absurd ha hoe

theorem A_establish (σ : State) (ℓ : Thread) (r : Bool) (h : σ.A) : (establish σ ℓ r).A := by
  intro a p b hp hb
  exact h a p b hp hb

theorem Fresh_establish (σ : State) (ℓ : Thread) (r : Bool) (hf : σ.Fresh) : (establish σ ℓ r).Fresh := by
  refine ⟨?_, ?_, ?_⟩
  · intro a p hp
    have hp' : σ.paths a = some p := hp
    show a < σ.nextId + 1
    exact Nat.lt_succ_of_lt (hf.1 a p hp')
  · intro o rec ho
    by_cases hoe : o = σ.nextId
    · subst hoe
      show σ.nextId < σ.nextId + 1
      exact Nat.lt_succ_self _
    · have ho' : σ.objects o = some rec := by simpa [establish, hoe] using ho
      show o < σ.nextId + 1
      exact Nat.lt_succ_of_lt (hf.2.1 o rec ho')
  · intro o t ht
    by_cases hoe : o = σ.nextId
    · subst hoe
      simp [establish] at ht
    · have ht' : t ∈ σ.contents o := by simpa [establish, hoe] using ht
      show t < σ.nextId + 1
      exact Nat.lt_succ_of_lt (hf.2.2 o t ht')

theorem C1_establish (σ : State) (ℓ : Thread) (r : Bool) (h : σ.C1) : (establish σ ℓ r).C1 := h
theorem C2_establish (σ : State) (ℓ : Thread) (r : Bool) (h : σ.C2) : (establish σ ℓ r).C2 := h

theorem R_establish (σ : State) (ℓ : Thread) (r : Bool) (hrun : σ.running ℓ) (hs : σ.scopes ℓ ≠ [])
    (hf : σ.Fresh) (h : σ.R) : (establish σ ℓ r).R := by
  intro o ho
  obtain ⟨tr, htr, _⟩ := hrun
  by_cases hoe : o = σ.nextId
  · subst hoe
    have hr : r = true := by simpa [establish] using ho
    subst hr
    refine ⟨{ alive := true, isResource := true, guard := false, reclaimed := false, frame := none, scope := σ.scope ℓ }, ℓ, tr,
      by simp [establish], rfl, by simp [establish], htr, Or.inr (Or.inl ⟨rfl, ?_⟩)⟩
    show (σ.scopes ℓ).headD 0 ∈ tr.scopes
    rw [scopes_of σ ℓ tr htr] at hs ⊢
    exact head_mem _ _ hs
  · have ho' : σ.obligation o = true := by simpa [establish, hoe] using ho
    obtain ⟨rec, ℓ', tr', hr, ha, hau, htr', how⟩ := h o ho'
    refine ⟨rec, ℓ', tr', by simp [establish, hoe, hr], ha, by simp [establish, hoe, hau], htr', ?_⟩
    refine Owner_mono σ _ o rec tr' ?_ how
    intro hb
    have hne := obligation_ne_next σ hf h tr'.handle hb
    simp [establish, hne, hb]

/-! ### bind -/

theorem alive_bind_iff (σ : State) (ℓ : Thread) (o i : Identity) : (bind σ ℓ o).alive i ↔ σ.alive i := by
  constructor
  · intro ⟨r, hr, ha⟩
    by_cases hio : i = o
    · subst hio
      simp [bind] at hr
      obtain ⟨q, hq, rfl⟩ := hr
      exact ⟨q, hq, ha⟩
    · exact ⟨r, by simpa [bind, hio] using hr, ha⟩
  · intro ⟨r, hr, ha⟩
    by_cases hio : i = o
    · subst hio
      exact ⟨{ r with frame := some (σ.frame ℓ) }, by simp [bind, hr], ha⟩
    · exact ⟨r, by simp [bind, hio, hr], ha⟩

theorem T1_bind (σ : State) (ℓ : Thread) (o : Identity) (ho : σ.alive o) (h : σ.T1) : (bind σ ℓ o).T1 := by
  intro a p hp hv
  rw [alive_bind_iff]
  by_cases hae : a = σ.nextId
  · subst hae
    simp [bind] at hp
    subst hp
    exact ho
  · have hp' : σ.paths a = some p := by simpa [bind, hae] using hp
    exact h a p hp' hv

theorem T2_bind (σ : State) (ℓ : Thread) (o : Identity) (h : σ.T2) (hf : σ.Fresh) : (bind σ ℓ o).T2 := by
  intro a p hp i
  rw [alive_bind_iff σ ℓ o i]
  have hc : (bind σ ℓ o).contents i = σ.contents i := rfl
  rw [hc]
  by_cases hae : a = σ.nextId
  · subst hae
    simp [bind] at hp
    subst hp
    simp
    intro _ hm
    exact absurd (hf.2.2 i _ hm) (Nat.lt_irrefl _)
  · have hp' : σ.paths a = some p := by simpa [bind, hae] using hp
    exact h a p hp' i

theorem A_bind (σ : State) (ℓ : Thread) (o : Identity) (h : σ.A) (hf : σ.Fresh) : (bind σ ℓ o).A := by
  intro a p b hp hb
  by_cases hae : a = σ.nextId
  · subst hae
    simp [bind] at hp
    subst hp
    simp at hb
  · have hp' : σ.paths a = some p := by simpa [bind, hae] using hp
    obtain ⟨hlt, hne⟩ := h a p b hp' hb
    refine ⟨hlt, ?_⟩
    have hbe : b ≠ σ.nextId := by
      intro heq
      subst heq
      exact absurd (Nat.lt_trans hlt (hf.1 a p hp')) (Nat.lt_irrefl _)
    simpa [bind, hbe] using hne

theorem Fresh_bind (σ : State) (ℓ : Thread) (o : Identity) (hf : σ.Fresh) : (bind σ ℓ o).Fresh := by
  refine ⟨?_, ?_, ?_⟩
  · intro a p hp
    by_cases hae : a = σ.nextId
    · subst hae
      show σ.nextId < σ.nextId + 1
      exact Nat.lt_succ_self _
    · have hp' : σ.paths a = some p := by simpa [bind, hae] using hp
      show a < σ.nextId + 1
      exact Nat.lt_succ_of_lt (hf.1 a p hp')
  · intro i rec hi
    show i < σ.nextId + 1
    by_cases hio : i = o
    · subst hio
      simp [bind] at hi
      obtain ⟨r, hr, _⟩ := hi
      exact Nat.lt_succ_of_lt (hf.2.1 i r hr)
    · have hi' : σ.objects i = some rec := by simpa [bind, hio] using hi
      exact Nat.lt_succ_of_lt (hf.2.1 i rec hi')
  · intro i t ht
    show t < σ.nextId + 1
    exact Nat.lt_succ_of_lt (hf.2.2 i t ht)

theorem C1_bind (σ : State) (ℓ : Thread) (o : Identity) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (bind σ ℓ o).C1 :=
  (C_newpath σ _ rfl hf h1 h2).1
theorem C2_bind (σ : State) (ℓ : Thread) (o : Identity) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (bind σ ℓ o).C2 :=
  (C_newpath σ _ rfl hf h1 h2).2

theorem R_bind (σ : State) (ℓ : Thread) (o : Identity) (hfr : σ.frames ℓ ≠ [])
    (hau : ∀ ℓ', σ.authority o = some ℓ' → ℓ' = ℓ) (h : σ.R) : (bind σ ℓ o).R := by
  intro i hi
  have hi' : σ.obligation i = true := hi
  obtain ⟨rec, ℓ', tr, hr, ha, hau', htr, how⟩ := h i hi'
  by_cases hio : i = o
  · subst hio
    have hle : ℓ' = ℓ := hau ℓ' hau'
    subst hle
    refine ⟨{ rec with frame := some (σ.frame ℓ') }, ℓ', tr, by simp [bind, hr], ha, hau', htr, Or.inl ⟨σ.frame ℓ', rfl, ?_⟩⟩
    show (σ.frames ℓ').headD 0 ∈ tr.frames
    rw [frames_of σ ℓ' tr htr] at hfr ⊢
    exact head_mem _ _ hfr
  · exact ⟨rec, ℓ', tr, by simp [bind, hio, hr], ha, hau', htr, Owner_mono σ _ i rec tr (fun hb => hb) how⟩

/-! ### borrow and project: a new path, nothing else -/

theorem T1_newpath (σ : State) (a0 : Token) (p0 p : PathRecord) (hp0 : σ.paths a0 = some p0) (hof : p.of = p0.of)
    (htv : σ.temporallyValid a0) (h : σ.T1) :
    (∀ a q, (if a = σ.nextId then some p else σ.paths a) = some q → q.valid = true → σ.alive q.of) := by
  intro a q hq hv
  by_cases hae : a = σ.nextId
  · subst hae
    simp at hq
    subst hq
    obtain ⟨r, hr, _, hal⟩ := htv
    rw [hp0] at hr
    cases hr
    rw [hof]
    exact hal
  · simp [hae] at hq
    exact h a q hq hv

theorem T2_newpath (σ : State) (p : PathRecord) (hh : p.heldBy = []) (h : σ.T2) (hf : σ.Fresh) :
    (∀ a q, (if a = σ.nextId then some p else σ.paths a) = some q → ∀ o, o ∈ q.heldBy ↔ (σ.alive o ∧ a ∈ σ.contents o)) := by
  intro a q hq o
  by_cases hae : a = σ.nextId
  · subst hae
    simp at hq
    subst hq
    rw [hh]
    simp
    intro _ hm
    exact absurd (hf.2.2 o _ hm) (Nat.lt_irrefl _)
  · simp [hae] at hq
    exact h a q hq o

theorem A_newpath (σ : State) (a0 : Token) (p0 p : PathRecord) (hp0 : σ.paths a0 = some p0) (hb : p.base = some a0)
    (h : σ.A) (hf : σ.Fresh) :
    (∀ a q b, (if a = σ.nextId then some p else σ.paths a) = some q → q.base = some b →
      b < a ∧ (if b = σ.nextId then some p else σ.paths b) ≠ none) := by
  intro a q b hq hqb
  by_cases hae : a = σ.nextId
  · subst hae
    simp at hq
    subst hq
    rw [hb] at hqb
    cases hqb
    refine ⟨hf.1 a0 p0 hp0, ?_⟩
    have hne : a0 ≠ σ.nextId := Nat.ne_of_lt (hf.1 a0 p0 hp0)
    simp [hne, hp0]
  · simp [hae] at hq
    obtain ⟨hlt, hne⟩ := h a q b hq hqb
    refine ⟨hlt, ?_⟩
    have hbe : b ≠ σ.nextId := by
      intro heq
      subst heq
      exact absurd (Nat.lt_trans hlt (hf.1 a q hq)) (Nat.lt_irrefl _)
    simpa [hbe] using hne

theorem Fresh_newpath (σ : State) (p : PathRecord) (hf : σ.Fresh) :
    (∀ a q, (if a = σ.nextId then some p else σ.paths a) = some q → a < σ.nextId + 1) ∧
    (∀ o r, σ.objects o = some r → o < σ.nextId + 1) ∧
    (∀ o t, t ∈ σ.contents o → t < σ.nextId + 1) := by
  refine ⟨?_, ?_, ?_⟩
  · intro a q hq
    by_cases hae : a = σ.nextId
    · subst hae
      exact Nat.lt_succ_self _
    · simp [hae] at hq
      exact Nat.lt_succ_of_lt (hf.1 a q hq)
  · intro o r ho
    exact Nat.lt_succ_of_lt (hf.2.1 o r ho)
  · intro o t ht
    exact Nat.lt_succ_of_lt (hf.2.2 o t ht)

theorem T1_borrow (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (m : Mode) (pend : Bool) (hp0 : σ.paths a0 = some p0)
    (htv : σ.temporallyValid a0) (h : σ.T1) : (borrow σ ℓ a0 p0 m pend).T1 :=
  T1_newpath σ a0 p0 _ hp0 rfl htv h
theorem T2_borrow (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (m : Mode) (pend : Bool) (h : σ.T2) (hf : σ.Fresh) : (borrow σ ℓ a0 p0 m pend).T2 :=
  T2_newpath σ _ rfl h hf
theorem A_borrow (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (m : Mode) (pend : Bool) (hp0 : σ.paths a0 = some p0)
    (h : σ.A) (hf : σ.Fresh) : (borrow σ ℓ a0 p0 m pend).A :=
  A_newpath σ a0 p0 _ hp0 rfl h hf
theorem Fresh_borrow (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (m : Mode) (pend : Bool) (hf : σ.Fresh) : (borrow σ ℓ a0 p0 m pend).Fresh :=
  Fresh_newpath σ _ hf
theorem R_borrow (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (m : Mode) (pend : Bool) (h : σ.R) : (borrow σ ℓ a0 p0 m pend).R := h
theorem C1_borrow (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (m : Mode) (pend : Bool) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (borrow σ ℓ a0 p0 m pend).C1 :=
  (C_newpath σ _ rfl hf h1 h2).1
theorem C2_borrow (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (m : Mode) (pend : Bool) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (borrow σ ℓ a0 p0 m pend).C2 :=
  (C_newpath σ _ rfl hf h1 h2).2

theorem T1_project (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (f : Proj) (hp0 : σ.paths a0 = some p0)
    (htv : σ.temporallyValid a0) (h : σ.T1) : (project σ ℓ a0 p0 f).T1 :=
  T1_newpath σ a0 p0 _ hp0 rfl htv h
theorem T2_project (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (f : Proj) (h : σ.T2) (hf : σ.Fresh) : (project σ ℓ a0 p0 f).T2 :=
  T2_newpath σ _ rfl h hf
theorem A_project (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (f : Proj) (hp0 : σ.paths a0 = some p0)
    (h : σ.A) (hf : σ.Fresh) : (project σ ℓ a0 p0 f).A :=
  A_newpath σ a0 p0 _ hp0 rfl h hf
theorem Fresh_project (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (f : Proj) (hf : σ.Fresh) : (project σ ℓ a0 p0 f).Fresh :=
  Fresh_newpath σ _ hf
theorem R_project (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (f : Proj) (h : σ.R) : (project σ ℓ a0 p0 f).R := h
theorem C1_project (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (f : Proj) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (project σ ℓ a0 p0 f).C1 :=
  (C_newpath σ _ rfl hf h1 h2).1
theorem C2_project (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (f : Proj) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (project σ ℓ a0 p0 f).C2 :=
  (C_newpath σ _ rfl hf h1 h2).2

/-! ### write (the contents change; `held-by` follows them) -/

theorem alive_writeWith_iff (σ : State) (o : Identity) (refs : List Token) (i : Identity) :
    (writeWith σ o refs).alive i ↔ σ.alive i := Iff.rfl

theorem T1_writeWith (σ : State) (o : Identity) (refs : List Token) (h : σ.T1) : (writeWith σ o refs).T1 := by
  intro a p hp hv
  simp [writeWith] at hp
  obtain ⟨q, hq, rfl⟩ := hp
  exact h a q hq hv

theorem T2_writeWith (σ : State) (o : Identity) (refs : List Token) (ho : σ.alive o) (h : σ.T2) :
    (writeWith σ o refs).T2 := by
  intro a p hp i
  rw [alive_writeWith_iff]
  simp [writeWith] at hp
  obtain ⟨q, hq, rfl⟩ := hp
  have hq2 := h a q hq i
  simp only []
  by_cases hio : i = o
  · subst hio
    by_cases har : a ∈ refs
    · simp [writeWith, har]
      refine ⟨fun _ => ho, fun _ => ?_⟩
      by_cases hoq : i ∈ q.heldBy
      · simp [hoq]
      · simp [hoq]
    · simp [writeWith, har, List.mem_filter]
  · simp [writeWith, hio]
    by_cases har : a ∈ refs
    · simp [har]
      rw [← hq2]
      by_cases hoq : o ∈ q.heldBy
      · simp [hoq]
      · simp [hoq, hio]
    · simp [har, List.mem_filter, hio]
      exact hq2

theorem A_writeWith (σ : State) (o : Identity) (refs : List Token) (h : σ.A) : (writeWith σ o refs).A := by
  intro a p b hp hb
  simp [writeWith] at hp
  obtain ⟨q, hq, rfl⟩ := hp
  simp at hb
  obtain ⟨hlt, hne⟩ := h a q b hq hb
  refine ⟨hlt, ?_⟩
  simp [writeWith]
  exact hne

theorem Fresh_writeWith (σ : State) (o : Identity) (refs : List Token) (hrefs : ∀ t ∈ refs, t < σ.nextId)
    (hf : σ.Fresh) : (writeWith σ o refs).Fresh := by
  refine ⟨?_, ?_, ?_⟩
  · intro a p hp
    simp [writeWith] at hp
    obtain ⟨q, hq, _⟩ := hp
    exact hf.1 a q hq
  · intro i rec hi
    exact hf.2.1 i rec hi
  · intro i t ht
    by_cases hio : i = o
    · subst hio
      have ht' : t ∈ refs := by simpa [writeWith] using ht
      exact hrefs t ht'
    · have ht' : t ∈ σ.contents i := by simpa [writeWith, hio] using ht
      exact hf.2.2 i t ht'

theorem T1_write (σ : State) (o : Identity) (removed added : List Token) (h : σ.T1) : (write σ o removed added).T1 :=
  T1_writeWith σ o _ h
theorem T2_write (σ : State) (o : Identity) (removed added : List Token) (ho : σ.alive o) (h : σ.T2) :
    (write σ o removed added).T2 :=
  T2_writeWith σ o _ ho h
theorem A_write (σ : State) (o : Identity) (removed added : List Token) (h : σ.A) : (write σ o removed added).A :=
  A_writeWith σ o _ h
theorem Fresh_write (σ : State) (o : Identity) (removed added : List Token) (hadded : ∀ t ∈ added, σ.paths t ≠ none)
    (hf : σ.Fresh) : (write σ o removed added).Fresh := by
  apply Fresh_writeWith σ o _ _ hf
  intro t ht
  simp [List.mem_filter] at ht
  rcases ht with ⟨hold, _⟩ | hnew
  · exact hf.2.2 o t hold
  · have hne := hadded t hnew
    cases hpt : σ.paths t with
    | none => exact absurd hpt hne
    | some q => exact hf.1 t q hpt
theorem R_write (σ : State) (o : Identity) (removed added : List Token) (h : σ.R) : (write σ o removed added).R := h
theorem C1_writeWith (σ : State) (o : Identity) (refs : List Token) (h1 : σ.C1) (h2 : σ.C2) : (writeWith σ o refs).C1 :=
  (C_pathmap σ (fun t p => { p with heldBy := if t ∈ refs then (if o ∈ p.heldBy then p.heldBy else o :: p.heldBy) else p.heldBy.filter (· ≠ o) })
    (fun _ _ => ⟨rfl, rfl, rfl, rfl⟩) (fun _ _ h => h) h1 h2).1
theorem C2_writeWith (σ : State) (o : Identity) (refs : List Token) (h1 : σ.C1) (h2 : σ.C2) : (writeWith σ o refs).C2 :=
  (C_pathmap σ (fun t p => { p with heldBy := if t ∈ refs then (if o ∈ p.heldBy then p.heldBy else o :: p.heldBy) else p.heldBy.filter (· ≠ o) })
    (fun _ _ => ⟨rfl, rfl, rfl, rfl⟩) (fun _ _ h => h) h1 h2).2
theorem C1_write (σ : State) (o : Identity) (removed added : List Token) (h1 : σ.C1) (h2 : σ.C2) : (write σ o removed added).C1 :=
  C1_writeWith σ o _ h1 h2
theorem C2_write (σ : State) (o : Identity) (removed added : List Token) (h1 : σ.C1) (h2 : σ.C2) : (write σ o removed added).C2 :=
  C2_writeWith σ o _ h1 h2

/-! ### endObjects -/

theorem alive_endObjects_iff (σ : State) (dead : Identity → Bool) (i : Identity) :
    (endObjects σ dead).alive i ↔ (σ.alive i ∧ dead i = false) := by
  constructor
  · intro ⟨r, hr, ha⟩
    by_cases hd : dead i = true
    · simp [endObjects, hd] at hr
      obtain ⟨q, _, rfl⟩ := hr
      simp at ha
    · have hd' : dead i = false := by simpa using hd
      exact ⟨⟨r, by simpa [endObjects, hd'] using hr, ha⟩, hd'⟩
  · intro ⟨⟨r, hr, ha⟩, hd⟩
    exact ⟨r, by simp [endObjects, hd, hr], ha⟩

theorem T1_endObjects (σ : State) (dead : Identity → Bool) (h : σ.T1) : (endObjects σ dead).T1 := by
  intro a p hp hv
  rw [alive_endObjects_iff]
  simp [endObjects] at hp
  obtain ⟨q, hq, rfl⟩ := hp
  simp at hv
  obtain ⟨hne, hqv⟩ := hv
  exact ⟨h a q hq hqv, by simpa using hne⟩

theorem T2_endObjects (σ : State) (dead : Identity → Bool) (h : σ.T2) : (endObjects σ dead).T2 := by
  intro a p hp i
  rw [alive_endObjects_iff]
  simp [endObjects] at hp
  obtain ⟨q, hq, rfl⟩ := hp
  have hc : (endObjects σ dead).contents i = σ.contents i := rfl
  rw [hc]
  simp [List.mem_filter]
  rw [h a q hq i]
  constructor
  · intro ⟨⟨ha, hm⟩, hd⟩
    exact ⟨⟨ha, hd⟩, hm⟩
  · intro ⟨⟨ha, hd⟩, hm⟩
    exact ⟨⟨ha, hm⟩, hd⟩

theorem A_endObjects (σ : State) (dead : Identity → Bool) (h : σ.A) : (endObjects σ dead).A := by
  intro a p b hp hb
  simp [endObjects] at hp
  obtain ⟨q, hq, rfl⟩ := hp
  simp at hb
  obtain ⟨hlt, hne⟩ := h a q b hq hb
  refine ⟨hlt, ?_⟩
  simp [endObjects]
  exact hne

theorem Fresh_endObjects (σ : State) (dead : Identity → Bool) (hf : σ.Fresh) : (endObjects σ dead).Fresh := by
  refine ⟨?_, ?_, ?_⟩
  · intro a p hp
    simp [endObjects] at hp
    obtain ⟨q, hq, _⟩ := hp
    exact hf.1 a q hq
  · intro i rec hi
    simp [endObjects] at hi
    split at hi
    · simp at hi
      obtain ⟨q, hq, _⟩ := hi
      exact hf.2.1 _ q hq
    · exact hf.2.1 i rec hi
  · intro i t ht
    exact hf.2.2 i t ht

theorem obligation_endObjects (σ : State) (dead : Identity → Bool) (o : Identity)
    (ho : (endObjects σ dead).obligation o = true) : dead o = false ∧ σ.obligation o = true := by
  simp [endObjects] at ho
  exact ⟨by simpa using ho.1, ho.2⟩

theorem objects_endObjects_live (σ : State) (dead : Identity → Bool) (o : Identity) (hnd : dead o = false) :
    (endObjects σ dead).objects o = σ.objects o := by
  simp [endObjects, hnd]

theorem endObjects_valid (dead : Identity → Bool) (q : PathRecord)
    (hv : ({ q with valid := if dead q.of then false else q.valid, heldBy := q.heldBy.filter (fun o => !dead o) } : PathRecord).valid = true) :
    q.valid = true := by
  simp at hv
  exact hv.2

theorem C1_endObjects (σ : State) (dead : Identity → Bool) (h1 : σ.C1) (h2 : σ.C2) : (endObjects σ dead).C1 :=
  (C_pathmap σ (fun _ p => { p with valid := if dead p.of then false else p.valid, heldBy := p.heldBy.filter (fun o => !dead o) })
    (fun _ _ => ⟨rfl, rfl, rfl, rfl⟩) (fun _ q hv => endObjects_valid dead q hv) h1 h2).1
theorem C2_endObjects (σ : State) (dead : Identity → Bool) (h1 : σ.C1) (h2 : σ.C2) : (endObjects σ dead).C2 :=
  (C_pathmap σ (fun _ p => { p with valid := if dead p.of then false else p.valid, heldBy := p.heldBy.filter (fun o => !dead o) })
    (fun _ _ => ⟨rfl, rfl, rfl, rfl⟩) (fun _ q hv => endObjects_valid dead q hv) h1 h2).2

theorem R_endObjects (σ : State) (dead : Identity → Bool) (hnh : σ.noHandle dead) (h : σ.R) : (endObjects σ dead).R := by
  intro o ho
  obtain ⟨hnd, ho'⟩ := obligation_endObjects σ dead o ho
  obtain ⟨rec, ℓ, tr, hr, ha, hau, htr, how⟩ := h o ho'
  refine ⟨rec, ℓ, tr, by simp [endObjects, hnd, hr], ha, hau, htr, Owner_mono σ _ o rec tr ?_ how⟩
  intro hb
  have hd := hnh ℓ tr htr
  simp [endObjects, hd, hb]

/-! ### the thread-local stacks -/

theorem R_blockEnter (σ : State) (ℓ : Thread) (f : Frame) (h : σ.R) : (blockEnter σ ℓ f).R := by
  intro o ho
  obtain ⟨rec, ℓ', tr, hr, ha, hau, htr, how⟩ := h o ho
  by_cases hle : ℓ' = ℓ
  · subst hle
    refine ⟨rec, ℓ', { tr with frames := f :: tr.frames }, hr, ha, hau, by simp [blockEnter, mapThread, htr], ?_⟩
    rcases how with ⟨g, hg, hm⟩ | h2 | h3 | h4
    · exact Or.inl ⟨g, hg, by simp [hm]⟩
    · exact Or.inr (Or.inl h2)
    · exact Or.inr (Or.inr (Or.inl h3))
    · exact Or.inr (Or.inr (Or.inr h4))
  · exact ⟨rec, ℓ', tr, hr, ha, hau, by simp [blockEnter, mapThread, hle, htr], how⟩

theorem R_stmtEnter (σ : State) (ℓ : Thread) (t : Scope) (h : σ.R) : (stmtEnter σ ℓ t).R := by
  intro o ho
  obtain ⟨rec, ℓ', tr, hr, ha, hau, htr, how⟩ := h o ho
  by_cases hle : ℓ' = ℓ
  · subst hle
    refine ⟨rec, ℓ', { tr with scopes := t :: tr.scopes }, hr, ha, hau, by simp [stmtEnter, mapThread, htr], ?_⟩
    rcases how with h1 | ⟨hf, hm⟩ | h3 | h4
    · exact Or.inl h1
    · exact Or.inr (Or.inl ⟨hf, by simp [hm]⟩)
    · exact Or.inr (Or.inr (Or.inl h3))
    · exact Or.inr (Or.inr (Or.inr h4))
  · exact ⟨rec, ℓ', tr, hr, ha, hau, by simp [stmtEnter, mapThread, hle, htr], how⟩

theorem R_blockExit (σ : State) (ℓ : Thread) (hfr : σ.frames ℓ ≠ []) (hnh : σ.noHandle (blockDead σ ℓ)) (h : σ.R) :
    (blockExit σ ℓ).R := by
  have h1 := R_endObjects σ (blockDead σ ℓ) hnh h
  intro o ho
  have ho1 : (endObjects σ (blockDead σ ℓ)).obligation o = true := ho
  obtain ⟨rec, ℓ', tr, hr, ha, hau, htr, how⟩ := h1 o ho1
  obtain ⟨hnd, _⟩ := obligation_endObjects σ _ o ho1
  have hr0 : σ.objects o = some rec := by rwa [objects_endObjects_live σ _ o hnd] at hr
  have htr0 : σ.threads ℓ' = some tr := htr
  by_cases hle : ℓ' = ℓ
  · subst hle
    refine ⟨rec, ℓ', { tr with frames := tr.frames.tail }, hr, ha, hau, by simp [blockExit, mapThread, htr], ?_⟩
    rcases how with ⟨g, hg, hm⟩ | h2 | h3 | h4
    · refine Or.inl ⟨g, hg, ?_⟩
      have hne : g ≠ σ.frame ℓ' := by
        intro heq
        simp [blockDead, hr0, hg, heq] at hnd
      show g ∈ tr.frames.tail
      rw [frames_of σ ℓ' tr htr0] at hfr
      have hfe : σ.frame ℓ' = tr.frames.headD 0 := by simp [State.frame, frames_of σ ℓ' tr htr0]
      rw [hfe] at hne
      exact mem_tail_of_ne tr.frames 0 g hfr hm hne
    · exact Or.inr (Or.inl h2)
    · exact Or.inr (Or.inr (Or.inl h3))
    · exact Or.inr (Or.inr (Or.inr h4))
  · exact ⟨rec, ℓ', tr, hr, ha, hau, by simp [blockExit, mapThread, hle, htr], how⟩

theorem R_stmtExit (σ : State) (ℓ : Thread) (hs : σ.scopes ℓ ≠ []) (hnh : σ.noHandle (stmtDead σ ℓ)) (h : σ.R) :
    (stmtExit σ ℓ).R := by
  have h1 := R_endObjects σ (stmtDead σ ℓ) hnh h
  intro o ho
  have ho1 : (endObjects σ (stmtDead σ ℓ)).obligation o = true := ho
  obtain ⟨rec, ℓ', tr, hr, ha, hau, htr, how⟩ := h1 o ho1
  obtain ⟨hnd, _⟩ := obligation_endObjects σ _ o ho1
  have hr0 : σ.objects o = some rec := by rwa [objects_endObjects_live σ _ o hnd] at hr
  have htr0 : σ.threads ℓ' = some tr := htr
  by_cases hle : ℓ' = ℓ
  · subst hle
    refine ⟨rec, ℓ', { tr with scopes := tr.scopes.tail }, hr, ha, hau, by simp [stmtExit, mapThread, invalidateUnheld, htr], ?_⟩
    rcases how with h1 | ⟨hf, hm⟩ | h3 | h4
    · exact Or.inl h1
    · refine Or.inr (Or.inl ⟨hf, ?_⟩)
      have hne : rec.scope ≠ σ.scope ℓ' := by
        intro heq
        simp [stmtDead, hr0, hf, heq] at hnd
      show rec.scope ∈ tr.scopes.tail
      rw [scopes_of σ ℓ' tr htr0] at hs
      have hse : σ.scope ℓ' = tr.scopes.headD 0 := by simp [State.scope, scopes_of σ ℓ' tr htr0]
      rw [hse] at hne
      exact mem_tail_of_ne tr.scopes 0 rec.scope hs hm hne
    · exact Or.inr (Or.inr (Or.inl h3))
    · exact Or.inr (Or.inr (Or.inr h4))
  · exact ⟨rec, ℓ', tr, hr, ha, hau, by simp [stmtExit, mapThread, invalidateUnheld, hle, htr], how⟩

-- `invalidateUnheld` only lowers validity.
theorem T1_invalidateUnheld (σ : State) (t : Scope) (h : σ.T1) : (invalidateUnheld σ t).T1 := by
  intro a p hp hv
  simp [invalidateUnheld] at hp
  obtain ⟨q, hq, rfl⟩ := hp
  simp at hv
  obtain ⟨_, hqv⟩ := hv
  exact h a q hq hqv

theorem T2_invalidateUnheld (σ : State) (t : Scope) (h : σ.T2) : (invalidateUnheld σ t).T2 := by
  intro a p hp i
  simp [invalidateUnheld] at hp
  obtain ⟨q, hq, rfl⟩ := hp
  simp
  exact h a q hq i

theorem A_invalidateUnheld (σ : State) (t : Scope) (h : σ.A) : (invalidateUnheld σ t).A := by
  intro a p b hp hb
  simp [invalidateUnheld] at hp
  obtain ⟨q, hq, rfl⟩ := hp
  simp at hb
  obtain ⟨hlt, hne⟩ := h a q b hq hb
  refine ⟨hlt, ?_⟩
  simp [invalidateUnheld]
  exact hne

theorem Fresh_invalidateUnheld (σ : State) (t : Scope) (hf : σ.Fresh) : (invalidateUnheld σ t).Fresh := by
  refine ⟨?_, ?_, ?_⟩
  · intro a p hp
    simp [invalidateUnheld] at hp
    obtain ⟨q, hq, _⟩ := hp
    exact hf.1 a q hq
  · intro i rec hi
    exact hf.2.1 i rec hi
  · intro i t ht
    exact hf.2.2 i t ht

theorem invalidateUnheld_valid (σ : State) (t : Scope) (a : Token) (q : PathRecord)
    (hv : ({ q with valid := if q.scope = t ∧ q.heldBy = [] ∧ σ.holder q.of ≠ some a then false else q.valid } : PathRecord).valid = true) :
    q.valid = true := by
  simp at hv
  exact hv.2

theorem C1_invalidateUnheld (σ : State) (t : Scope) (h1 : σ.C1) (h2 : σ.C2) : (invalidateUnheld σ t).C1 :=
  (C_pathmap σ (fun a p => { p with valid := if p.scope = t ∧ p.heldBy = [] ∧ σ.holder p.of ≠ some a then false else p.valid })
    (fun _ _ => ⟨rfl, rfl, rfl, rfl⟩) (fun a q hv => invalidateUnheld_valid σ t a q hv) h1 h2).1
theorem C2_invalidateUnheld (σ : State) (t : Scope) (h1 : σ.C1) (h2 : σ.C2) : (invalidateUnheld σ t).C2 :=
  (C_pathmap σ (fun a p => { p with valid := if p.scope = t ∧ p.heldBy = [] ∧ σ.holder p.of ≠ some a then false else p.valid })
    (fun _ _ => ⟨rfl, rfl, rfl, rfl⟩) (fun a q hv => invalidateUnheld_valid σ t a q hv) h1 h2).2

/-! ### transfer -/

theorem transfer_paths (σ : State) (a1 a2 : Token) (ℓ2 : Thread) (f2 : Frame) (o : Identity) (a : Token) :
    (transfer σ a1 a2 ℓ2 f2 o).paths a = (if a = a1 then (σ.paths a).map (fun p => { p with valid := false }) else σ.paths a) := rfl

theorem transfer_core (σ : State) (ℓ2 : Thread) (f2 : Frame) (o : Identity) :
    Core σ (restamp (rekey σ (fun i => decide (i = o)) ℓ2) (fun i => decide (i = o)) (some f2) none) :=
  (core_rekey σ _ ℓ2).trans (core_restamp _ _ _ _)

theorem T1_transfer (σ : State) (a1 a2 : Token) (ℓ2 : Thread) (f2 : Frame) (o : Identity) (h : σ.T1) : (transfer σ a1 a2 ℓ2 f2 o).T1 := by
  intro a p hp hv
  rw [transfer_paths] at hp
  show (restamp (rekey σ (fun i => decide (i = o)) ℓ2) (fun i => decide (i = o)) (some f2) none).alive p.of
  rw [(transfer_core σ ℓ2 f2 o).alive]
  by_cases hae : a = a1
  · subst hae
    simp at hp
    obtain ⟨q, _, rfl⟩ := hp
    simp at hv
  · simp [hae] at hp
    exact h a p hp hv

theorem T2_transfer (σ : State) (a1 a2 : Token) (ℓ2 : Thread) (f2 : Frame) (o : Identity) (h : σ.T2) : (transfer σ a1 a2 ℓ2 f2 o).T2 := by
  intro a p hp i
  rw [transfer_paths] at hp
  show i ∈ p.heldBy ↔ ((restamp (rekey σ (fun i => decide (i = o)) ℓ2) (fun i => decide (i = o)) (some f2) none).alive i ∧ a ∈ σ.contents i)
  rw [(transfer_core σ ℓ2 f2 o).alive]
  by_cases hae : a = a1
  · subst hae
    simp at hp
    obtain ⟨q, hq, rfl⟩ := hp
    simp
    exact h a q hq i
  · simp [hae] at hp
    exact h a p hp i

theorem A_transfer (σ : State) (a1 a2 : Token) (ℓ2 : Thread) (f2 : Frame) (o : Identity) (h : σ.A) : (transfer σ a1 a2 ℓ2 f2 o).A := by
  intro a p b hp hb
  rw [transfer_paths] at hp
  rw [transfer_paths]
  by_cases hae : a = a1
  · subst hae
    simp at hp
    obtain ⟨q, hq, rfl⟩ := hp
    simp at hb
    obtain ⟨hlt, hne⟩ := h a q b hq hb
    refine ⟨hlt, ?_⟩
    by_cases hba : b = a
    · subst hba
      simp [hq]
    · simpa [hba] using hne
  · simp [hae] at hp
    obtain ⟨hlt, hne⟩ := h a p b hp hb
    refine ⟨hlt, ?_⟩
    by_cases hba : b = a1
    · subst hba
      simp
      exact hne
    · simpa [hba] using hne

theorem Fresh_transfer (σ : State) (a1 a2 : Token) (ℓ2 : Thread) (f2 : Frame) (o : Identity) (hf : σ.Fresh) : (transfer σ a1 a2 ℓ2 f2 o).Fresh := by
  have c := transfer_core σ ℓ2 f2 o
  refine ⟨?_, ?_, ?_⟩
  · intro a p hp
    rw [transfer_paths] at hp
    show a < σ.nextId
    by_cases hae : a = a1
    · subst hae
      simp at hp
      obtain ⟨q, hq, _⟩ := hp
      exact hf.1 a q hq
    · simp [hae] at hp
      exact hf.1 a p hp
  · intro i rec hi
    show i < σ.nextId
    obtain ⟨r', hr'⟩ := c.dom i rec hi
    exact hf.2.1 i r' hr'
  · intro i t ht
    show t < σ.nextId
    exact hf.2.2 i t ht

theorem C1_transfer (σ : State) (a1 a2 : Token) (ℓ2 : Thread) (f2 : Frame) (o : Identity) (h1 : σ.C1) : (transfer σ a1 a2 ℓ2 f2 o).C1 := by
  intro o' ℓ a hs
  have hs' : σ.sync o' = SyncState.locked ℓ a := hs
  obtain ⟨q, hq, rest⟩ := h1 o' ℓ a hs'
  rw [transfer_paths]
  by_cases hae : a = a1
  · subst hae
    exact ⟨{ q with valid := false }, by simp [hq], rest⟩
  · exact ⟨q, by simp [hae, hq], rest⟩

theorem C2_transfer (σ : State) (a1 a2 : Token) (ℓ2 : Thread) (f2 : Frame) (o : Identity) (h2 : σ.C2) : (transfer σ a1 a2 ℓ2 f2 o).C2 := by
  intro a p hp hl hv
  rw [transfer_paths] at hp
  show ∃ ℓ, p.thread = some ℓ ∧ σ.sync p.of = SyncState.locked ℓ a
  by_cases hae : a = a1
  · subst hae
    simp at hp
    obtain ⟨q, _, rfl⟩ := hp
    simp at hv
  · simp [hae] at hp
    exact h2 a p hp hl hv

theorem R_transfer (σ : State) (ℓ ℓ2 : Thread) (a1 a2 : Token) (p1 : PathRecord) (f2 : Frame)
    (_hau1 : σ.authority p1.of = some ℓ) (hrun2 : σ.running ℓ2) (hfm : f2 ∈ σ.frames ℓ2)
    (h : σ.R) : (transfer σ a1 a2 ℓ2 f2 p1.of).R := by
  intro i hi
  have hi' : σ.obligation i = true := hi
  obtain ⟨rec, ℓ'', tr'', hr, ha, hau, htr'', how⟩ := h i hi'
  obtain ⟨tr2, htr2, _⟩ := hrun2
  by_cases hio : i = p1.of
  · subst hio
    refine ⟨{ rec with frame := some f2, scope := rec.scope }, ℓ2, tr2,
      by simp [transfer, restamp, rekey, hr], ha, by simp [transfer, restamp, rekey], htr2, Or.inl ⟨f2, rfl, ?_⟩⟩
    rw [frames_of σ ℓ2 tr2 htr2] at hfm
    exact hfm
  · exact ⟨rec, ℓ'', tr'', by simp [transfer, restamp, rekey, hio, hr], ha, by simp [transfer, restamp, rekey, hio, hau], htr'',
      Owner_mono σ _ i rec tr'' (fun hb => hb) how⟩

/-! ### spawn -/

theorem R_spawn (σ : State) (ℓ ℓ' : Thread) (f' : Frame) (hrun : σ.running ℓ) (hs : σ.scopes ℓ ≠ [])
    (hnone : σ.threads ℓ' = none) (hf : σ.Fresh) (h : σ.R) : (spawn σ ℓ ℓ' f').R := by
  have h1 := R_establish σ ℓ true hrun hs hf h
  intro o ho
  have ho1 : (establish σ ℓ true).obligation o = true := ho
  obtain ⟨rec, ℓ'', tr, hr, ha, hau, htr, how⟩ := h1 o ho1
  have hne : ℓ'' ≠ ℓ' := by
    intro heq
    subst heq
    have : σ.threads ℓ'' = some tr := htr
    rw [hnone] at this
    cases this
  exact ⟨rec, ℓ'', tr, hr, ha, hau, by simp [spawn, setThread, hne, htr], Owner_mono _ _ o rec tr (fun hb => hb) how⟩

/-! ### threadDone -/

theorem R_threadDone (σ : State) (ℓ' : Thread) (tr : ThreadRec) (f' : Frame) (res : List Identity)
    (htr : σ.threads ℓ' = some tr) (hdone : tr.done = none) (hfr : tr.frames = [f']) (hsc : tr.scopes = [])
    (hob : σ.obligation tr.handle = true)
    (hres : ∀ o ∈ res, σ.obligation o = true → σ.authority o = some ℓ')
    (hnh : σ.noHandle (fun i => blockDead σ ℓ' i && !decide (i ∈ res))) (h : σ.R) : (threadDone σ ℓ' res).R := by
  have h1 := R_endObjects σ _ hnh h
  intro o ho
  have ho1 : (endObjects σ (fun i => blockDead σ ℓ' i && !decide (i ∈ res))).obligation o = true := ho
  obtain ⟨rec, ℓ'', tr'', hr, ha, hau, htr'', how⟩ := h1 o ho1
  obtain ⟨hnd, ho0⟩ := obligation_endObjects σ _ o ho1
  have hr0 : σ.objects o = some rec := by rwa [objects_endObjects_live σ _ o hnd] at hr
  have hau0 : σ.authority o = some ℓ'' := hau
  have htr0 : σ.threads ℓ'' = some tr'' := htr''
  have hhd : (fun i => blockDead σ ℓ' i && !decide (i ∈ res)) tr.handle = false := hnh ℓ' tr htr
  by_cases hmem : o ∈ res
  · have hle : ℓ'' = ℓ' := by
      have := hres o hmem ho0
      rw [hau0] at this
      cases this
      rfl
    subst hle
    have htt : tr'' = tr := by
      rw [htr0] at htr
      cases htr
      rfl
    subst htt
    refine ⟨{ rec with frame := none, scope := rec.scope }, ℓ'', { tr'' with frames := [], scopes := [], done := some res }, ?_, ha, hau, ?_, Or.inr (Or.inr (Or.inl ⟨res, rfl, hmem, ?_⟩))⟩
    · simp [threadDone, mapThread, restamp, hmem, hr]
    · simp [threadDone, mapThread, restamp, htr'']
    · have hhd' : (blockDead σ ℓ'' tr''.handle && !decide (tr''.handle ∈ res)) = false := hhd
      have hor : blockDead σ ℓ'' tr''.handle = false ∨ tr''.handle ∈ res := by
        by_cases hin : tr''.handle ∈ res
        · exact Or.inr hin
        · left
          simpa [hin] using hhd'
      simp [threadDone, mapThread, restamp, endObjects, hob]
      exact hor
  · by_cases hle : ℓ'' = ℓ'
    · subst hle
      have htt : tr'' = tr := by
        rw [htr0] at htr
        cases htr
        rfl
      subst htt
      rcases how with ⟨g, hg, hm⟩ | ⟨_, hm⟩ | ⟨res', hd', _, _⟩ | h4
      · exfalso
        rw [hfr] at hm
        simp at hm
        subst hm
        have hfe : σ.frame ℓ'' = g := by simp [State.frame, frames_of σ ℓ'' tr'' htr0, hfr]
        simp [blockDead, hr0, hg, hfe, hmem] at hnd
      · exfalso
        rw [hsc] at hm
        simp at hm
      · exfalso
        rw [hdone] at hd'
        cases hd'
      · exact ⟨rec, ℓ'', { tr'' with frames := [], scopes := [], done := some res }, by simp [threadDone, mapThread, restamp, hmem, hr], ha, hau,
          by simp [threadDone, mapThread, restamp, htr''], Or.inr (Or.inr (Or.inr h4))⟩
    · refine ⟨rec, ℓ'', tr'', by simp [threadDone, mapThread, restamp, hmem, hr], ha, hau, by simp [threadDone, mapThread, restamp, hle, htr''], ?_⟩
      exact Owner_mono _ _ o rec tr'' (fun hb => hb) how

/-! ### join -/

theorem R_join (σ : State) (ℓ ℓ' : Thread) (tr : ThreadRec) (res : List Identity)
    (hrun : σ.running ℓ) (hne : ℓ ≠ ℓ') (hs : σ.scopes ℓ ≠ [])
    (htr : σ.threads ℓ' = some tr) (hdone : tr.done = some res) (hfr : tr.frames = []) (hsc : tr.scopes = [])
    (hdist : ∀ ℓ'' tr'', ℓ'' ≠ ℓ' → σ.threads ℓ'' = some tr'' → tr''.handle ≠ tr.handle)
    (hnorecl : ∀ o r, σ.objects o = some r → r.reclaimed = true → σ.obligation o = true → σ.authority o ≠ some ℓ')
    (h : σ.R) : (join σ ℓ ℓ' tr.handle res).R := by
  intro o ho
  have ho1 : (endObjects σ (fun i => decide (i = tr.handle))).obligation o = true := ho
  obtain ⟨hnd, ho0⟩ := obligation_endObjects σ _ o ho1
  have hoh : o ≠ tr.handle := by simpa using hnd
  obtain ⟨rec, ℓ'', tr'', hr, ha, hau, htr'', how⟩ := h o ho0
  obtain ⟨trl, htrl, _⟩ := hrun
  by_cases hmem : o ∈ res
  · refine ⟨{ rec with frame := none, scope := σ.scope ℓ }, ℓ, trl, ?_, ha, ?_, ?_, Or.inr (Or.inl ⟨rfl, ?_⟩)⟩
    · simp [join, setThread, rekey, restamp, endObjects, hmem, hoh, hr]
    · simp [join, setThread, rekey, restamp, hmem]
    · simp [join, setThread, rekey, restamp, endObjects, hne, htrl]
    · show (σ.scopes ℓ).headD 0 ∈ trl.scopes
      rw [scopes_of σ ℓ trl htrl] at hs ⊢
      exact head_mem _ _ hs
  · by_cases hle : ℓ'' = ℓ'
    · subst hle
      have htt : tr'' = tr := by
        rw [htr''] at htr
        cases htr
        rfl
      subst htt
      exfalso
      rcases how with ⟨g, _, hm⟩ | ⟨_, hm⟩ | ⟨res', hd', hm', _⟩ | h4
      · rw [hfr] at hm
        simp at hm
      · rw [hsc] at hm
        simp at hm
      · rw [hdone] at hd'
        cases hd'
        exact hmem hm'
      · exact hnorecl o rec hr h4 ho0 hau
    · refine ⟨rec, ℓ'', tr'', by simp [join, setThread, rekey, restamp, endObjects, hmem, hoh, hr], ha,
        by simp [join, setThread, rekey, restamp, endObjects, hmem, hau], by simp [join, setThread, rekey, restamp, endObjects, hle, htr''], ?_⟩
      refine Owner_mono σ _ o rec tr'' ?_ how
      intro hb
      have hh := hdist ℓ'' tr'' hle htr''
      simp [join, setThread, rekey, restamp, endObjects, hh, hb]

/-! ### handleDestroy -/

theorem R_handleDestroy (σ : State) (ℓ' : Thread) (tr : ThreadRec) (res : List Identity)
    (htr : σ.threads ℓ' = some tr) (hdone : tr.done = some res) (hfr : tr.frames = []) (hsc : tr.scopes = [])
    (hdist : ∀ ℓ'' tr'', ℓ'' ≠ ℓ' → σ.threads ℓ'' = some tr'' → tr''.handle ≠ tr.handle ∧ tr''.handle ∉ res)
    (hnorecl : ∀ o r, σ.objects o = some r → r.reclaimed = true → σ.obligation o = true → σ.authority o ≠ some ℓ')
    (h : σ.R) : (handleDestroy σ ℓ' tr.handle res).R := by
  intro o ho
  have ho1 : (endObjects σ (fun i => decide (i = tr.handle) || decide (i ∈ res))).obligation o = true := ho
  obtain ⟨hnd, ho0⟩ := obligation_endObjects σ _ o ho1
  have hoh : o ≠ tr.handle ∧ o ∉ res := by simpa using hnd
  obtain ⟨rec, ℓ'', tr'', hr, ha, hau, htr'', how⟩ := h o ho0
  by_cases hle : ℓ'' = ℓ'
  · subst hle
    have htt : tr'' = tr := by
      rw [htr''] at htr
      cases htr
      rfl
    subst htt
    exfalso
    rcases how with ⟨g, _, hm⟩ | ⟨_, hm⟩ | ⟨res', hd', hm', _⟩ | h4
    · rw [hfr] at hm
      simp at hm
    · rw [hsc] at hm
      simp at hm
    · rw [hdone] at hd'
      cases hd'
      exact hoh.2 hm'
    · exact hnorecl o rec hr h4 ho0 hau
  · refine ⟨rec, ℓ'', tr'', by simp [handleDestroy, setThread, endObjects, hoh.1, hoh.2, hr], ha, hau,
      by simp [handleDestroy, setThread, endObjects, hle, htr''], ?_⟩
    refine Owner_mono σ _ o rec tr'' ?_ how
    intro hb
    obtain ⟨hh1, hh2⟩ := hdist ℓ'' tr'' hle htr''
    simp [handleDestroy, setThread, endObjects, hh1, hh2, hb]

/-! ### lock -/

theorem alive_lock_iff (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) (hf : σ.Fresh) (i : Identity) :
    (lock σ ℓ am pm).alive i ↔ (σ.alive i ∨ i = σ.nextId + 1) := by
  constructor
  · intro ⟨q, hq, ha⟩
    by_cases hie : i = σ.nextId + 1
    · exact Or.inr hie
    · exact Or.inl ⟨q, by simpa [lock, hie] using hq, ha⟩
  · intro h
    rcases h with ⟨q, hq, ha⟩ | rfl
    · have hne : i ≠ σ.nextId + 1 := by
        intro heq
        have := hf.2.1 i q hq
        rw [heq] at this
        exact succ_not_lt _ this
      exact ⟨q, by simp [lock, hne, hq], ha⟩
    · exact ⟨{ alive := true, isResource := true, guard := true, reclaimed := false, frame := none, scope := σ.scope ℓ }, by simp [lock], rfl⟩

theorem lock_paths (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) (a : Token) :
    (lock σ ℓ am pm).paths a = (if a = σ.nextId then some { pm with mode := Mode.exclusive, valid := true, base := some am, thread := some ℓ, lock := true, taken := false, pending := false, frame := σ.frame ℓ, scope := σ.scope ℓ, heldBy := [σ.nextId + 1] } else σ.paths a) := rfl

theorem T1_lock (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) (hpm : σ.paths am = some pm)
    (htv : σ.temporallyValid am) (hf : σ.Fresh) (h : σ.T1) : (lock σ ℓ am pm).T1 := by
  intro a p hp hv
  rw [lock_paths] at hp
  rw [alive_lock_iff σ ℓ am pm hf]
  by_cases hae : a = σ.nextId
  · subst hae
    simp at hp
    subst hp
    obtain ⟨q, hq, _, hal⟩ := htv
    rw [hpm] at hq
    cases hq
    exact Or.inl hal
  · simp [hae] at hp
    exact Or.inl (h a p hp hv)

theorem T2_lock (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) (hf : σ.Fresh) (h : σ.T2) : (lock σ ℓ am pm).T2 := by
  intro a p hp o
  rw [lock_paths] at hp
  rw [alive_lock_iff σ ℓ am pm hf]
  have hc : (lock σ ℓ am pm).contents o = (if o = σ.nextId + 1 then [σ.nextId] else σ.contents o) := rfl
  rw [hc]
  by_cases hae : a = σ.nextId
  · subst hae
    simp at hp
    subst hp
    by_cases hog : o = σ.nextId + 1
    · subst hog
      simp
    · simp [hog]
      intro hal hm
      exact absurd (hf.2.2 o _ hm) (Nat.lt_irrefl _)
  · simp [hae] at hp
    by_cases hog : o = σ.nextId + 1
    · subst hog
      have hnot : σ.nextId + 1 ∉ p.heldBy := by
        intro hm
        obtain ⟨⟨q, hq, _⟩, _⟩ := (h a p hp (σ.nextId + 1)).1 hm
        exact succ_not_lt _ (hf.2.1 _ q hq)
      simp [hnot, hae]
    · simp [hog]
      rw [h a p hp o]

theorem A_lock (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) (hpm : σ.paths am = some pm)
    (hf : σ.Fresh) (h : σ.A) : (lock σ ℓ am pm).A := by
  intro a p b hp hb
  rw [lock_paths] at hp
  rw [lock_paths]
  by_cases hae : a = σ.nextId
  · subst hae
    simp at hp
    subst hp
    simp at hb
    subst hb
    have hlt := hf.1 am pm hpm
    refine ⟨hlt, ?_⟩
    have hne : am ≠ σ.nextId := Nat.ne_of_lt hlt
    simp [hne, hpm]
  · simp [hae] at hp
    obtain ⟨hlt, hne⟩ := h a p b hp hb
    refine ⟨hlt, ?_⟩
    have hbe : b ≠ σ.nextId := by
      intro heq
      subst heq
      exact absurd (Nat.lt_trans hlt (hf.1 a p hp)) (Nat.lt_irrefl _)
    simpa [hbe] using hne

theorem Fresh_lock (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) (hf : σ.Fresh) : (lock σ ℓ am pm).Fresh := by
  refine ⟨?_, ?_, ?_⟩
  · intro a p hp
    rw [lock_paths] at hp
    show a < σ.nextId + 1 + 1
    by_cases hae : a = σ.nextId
    · subst hae
      exact Nat.lt_succ_of_lt (Nat.lt_succ_self _)
    · simp [hae] at hp
      exact Nat.lt_succ_of_lt (Nat.lt_succ_of_lt (hf.1 a p hp))
  · intro i rec hi
    show i < σ.nextId + 1 + 1
    by_cases hig : i = σ.nextId + 1
    · subst hig
      exact Nat.lt_succ_self _
    · have hi' : σ.objects i = some rec := by simpa [lock, hig] using hi
      exact Nat.lt_succ_of_lt (Nat.lt_succ_of_lt (hf.2.1 i rec hi'))
  · intro i t ht
    show t < σ.nextId + 1 + 1
    by_cases hig : i = σ.nextId + 1
    · subst hig
      have ht' : t = σ.nextId := by simpa [lock] using ht
      subst ht'
      exact Nat.lt_succ_of_lt (Nat.lt_succ_self _)
    · have ht' : t ∈ σ.contents i := by simpa [lock, hig] using ht
      exact Nat.lt_succ_of_lt (Nat.lt_succ_of_lt (hf.2.2 i t ht'))

theorem R_lock (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) (hrun : σ.running ℓ) (hs : σ.scopes ℓ ≠ [])
    (hf : σ.Fresh) (h : σ.R) : (lock σ ℓ am pm).R := by
  intro o ho
  obtain ⟨tr, htr, _⟩ := hrun
  by_cases hog : o = σ.nextId + 1
  · subst hog
    refine ⟨{ alive := true, isResource := true, guard := true, reclaimed := false, frame := none, scope := σ.scope ℓ }, ℓ, tr,
      by simp [lock], rfl, by simp [lock], htr, Or.inr (Or.inl ⟨rfl, ?_⟩)⟩
    show (σ.scopes ℓ).headD 0 ∈ tr.scopes
    rw [scopes_of σ ℓ tr htr] at hs ⊢
    exact head_mem _ _ hs
  · have ho' : σ.obligation o = true := by simpa [lock, hog] using ho
    obtain ⟨rec, ℓ', tr', hr, ha, hau, htr', how⟩ := h o ho'
    refine ⟨rec, ℓ', tr', by simp [lock, hog, hr], ha, by simp [lock, hog, hau], htr', ?_⟩
    refine Owner_mono σ _ o rec tr' ?_ how
    intro hb
    have hne : tr'.handle ≠ σ.nextId + 1 := by
      intro heq
      obtain ⟨q, _, _, hq, _⟩ := h tr'.handle hb
      have := hf.2.1 _ q hq
      rw [heq] at this
      exact succ_not_lt _ this
    simp [lock, hne, hb]

theorem C1_lock (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) (hf : σ.Fresh) (h1 : σ.C1) : (lock σ ℓ am pm).C1 := by
  intro o ℓ' a hs
  have hs' : (if o = pm.of then SyncState.locked ℓ σ.nextId else σ.sync o) = SyncState.locked ℓ' a := hs
  rw [lock_paths]
  by_cases hoe : o = pm.of
  · subst hoe
    simp at hs'
    obtain ⟨rfl, rfl⟩ := hs'
    simp
  · simp [hoe] at hs'
    obtain ⟨q, hq, rest⟩ := h1 o ℓ' a hs'
    have hne : a ≠ σ.nextId := Nat.ne_of_lt (hf.1 a q hq)
    exact ⟨q, by simp [hne, hq], rest⟩

theorem C2_lock (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) (hunl : σ.sync pm.of = SyncState.unlocked)
    (h2 : σ.C2) : (lock σ ℓ am pm).C2 := by
  intro a p hp hl hv
  rw [lock_paths] at hp
  show ∃ ℓ', p.thread = some ℓ' ∧ (if p.of = pm.of then SyncState.locked ℓ σ.nextId else σ.sync p.of) = SyncState.locked ℓ' a
  by_cases hae : a = σ.nextId
  · subst hae
    simp at hp
    subst hp
    exact ⟨ℓ, rfl, by simp⟩
  · simp [hae] at hp
    obtain ⟨ℓ', ht, hs⟩ := h2 a p hp hl hv
    have hne : p.of ≠ pm.of := by
      intro heq
      rw [heq, hunl] at hs
      cases hs
    exact ⟨ℓ', ht, by simp [hne, hs]⟩

/-! ### guardDrop -/

theorem guardDrop_paths (σ : State) (g : Identity) (ag : Token) (o : Identity) (a : Token) :
    (guardDrop σ g ag o).paths a =
      (if a = ag then ((endObjects σ (fun i => decide (i = g))).paths a).map (fun p => { p with valid := false })
       else (endObjects σ (fun i => decide (i = g))).paths a) := rfl

theorem T1_guardDrop (σ : State) (g : Identity) (ag : Token) (o : Identity) (h : σ.T1) : (guardDrop σ g ag o).T1 := by
  intro a p hp hv
  rw [guardDrop_paths] at hp
  show (endObjects σ (fun i => decide (i = g))).alive p.of
  by_cases hae : a = ag
  · subst hae
    simp at hp
    obtain ⟨q, _, rfl⟩ := hp
    simp at hv
  · simp [hae] at hp
    exact T1_endObjects σ _ h a p hp hv

theorem T2_guardDrop (σ : State) (g : Identity) (ag : Token) (o : Identity) (h : σ.T2) : (guardDrop σ g ag o).T2 := by
  intro a p hp i
  rw [guardDrop_paths] at hp
  show i ∈ p.heldBy ↔ ((endObjects σ (fun i => decide (i = g))).alive i ∧ a ∈ (endObjects σ (fun i => decide (i = g))).contents i)
  by_cases hae : a = ag
  · subst hae
    simp at hp
    obtain ⟨q, hq, rfl⟩ := hp
    simp
    exact T2_endObjects σ _ h a q hq i
  · simp [hae] at hp
    exact T2_endObjects σ _ h a p hp i

theorem A_guardDrop (σ : State) (g : Identity) (ag : Token) (o : Identity) (h : σ.A) : (guardDrop σ g ag o).A := by
  intro a p b hp hb
  rw [guardDrop_paths] at hp
  rw [guardDrop_paths]
  have hA := A_endObjects σ (fun i => decide (i = g)) h
  by_cases hae : a = ag
  · subst hae
    simp at hp
    obtain ⟨q, hq, rfl⟩ := hp
    simp at hb
    obtain ⟨hlt, hne⟩ := hA a q b hq hb
    refine ⟨hlt, ?_⟩
    by_cases hba : b = a
    · subst hba
      simp [hq]
    · simpa [hba] using hne
  · simp [hae] at hp
    obtain ⟨hlt, hne⟩ := hA a p b hp hb
    refine ⟨hlt, ?_⟩
    by_cases hba : b = ag
    · subst hba
      simp
      exact hne
    · simpa [hba] using hne

theorem Fresh_guardDrop (σ : State) (g : Identity) (ag : Token) (o : Identity) (hf : σ.Fresh) : (guardDrop σ g ag o).Fresh := by
  have h4 := Fresh_endObjects σ (fun i => decide (i = g)) hf
  refine ⟨?_, ?_, ?_⟩
  · intro a p hp
    rw [guardDrop_paths] at hp
    show a < σ.nextId
    by_cases hae : a = ag
    · subst hae
      simp at hp
      obtain ⟨q, hq, _⟩ := hp
      exact h4.1 a q hq
    · simp [hae] at hp
      exact h4.1 a p hp
  · intro i rec hi
    exact h4.2.1 i rec hi
  · intro i t ht
    exact h4.2.2 i t ht

theorem R_guardDrop (σ : State) (g : Identity) (ag : Token) (o : Identity) (hnh : σ.noHandle (fun i => decide (i = g)))
    (h : σ.R) : (guardDrop σ g ag o).R := by
  have hR := R_endObjects σ _ hnh h
  intro i hi
  have hi' : (endObjects σ (fun i => decide (i = g))).obligation i = true := hi
  obtain ⟨rec, ℓ, tr, hr, ha, hau, htr, how⟩ := hR i hi'
  exact ⟨rec, ℓ, tr, hr, ha, hau, htr, Owner_mono _ _ i rec tr (fun hb => hb) how⟩

theorem C1_guardDrop (σ : State) (g : Identity) (ag : Token) (o : Identity) (h1 : σ.C1) : (guardDrop σ g ag o).C1 := by
  intro o' ℓ a hs
  have hs' : (if o' = o then SyncState.unlocked else σ.sync o') = SyncState.locked ℓ a := hs
  by_cases hoe : o' = o
  · simp [hoe] at hs'
  · simp [hoe] at hs'
    obtain ⟨q, hq, hl, hof, ht, hm⟩ := h1 o' ℓ a hs'
    rw [guardDrop_paths]
    by_cases hae : a = ag
    · subst hae
      exact ⟨{ q with valid := false, heldBy := q.heldBy.filter (fun x => !decide (x = g)) }, by simp [endObjects, hq], hl, hof, ht, hm⟩
    · exact ⟨{ q with valid := if decide (q.of = g) then false else q.valid, heldBy := q.heldBy.filter (fun x => !decide (x = g)) }, by simp [hae, endObjects, hq], hl, hof, ht, hm⟩

theorem C2_guardDrop (σ : State) (g : Identity) (ag : Token) (pg : PathRecord) (hpg : σ.paths ag = some pg)
    (hlg : pg.lock = true) (hvg : pg.valid = true) (h2 : σ.C2) : (guardDrop σ g ag pg.of).C2 := by
  intro a p hp hl hv
  rw [guardDrop_paths] at hp
  show ∃ ℓ, p.thread = some ℓ ∧ (if p.of = pg.of then SyncState.unlocked else σ.sync p.of) = SyncState.locked ℓ a
  by_cases hae : a = ag
  · subst hae
    simp at hp
    obtain ⟨q, _, rfl⟩ := hp
    simp at hv
  · simp [hae, endObjects] at hp
    obtain ⟨q, hq, rfl⟩ := hp
    simp at hv hl
    obtain ⟨_, hqv⟩ := hv
    obtain ⟨ℓ, ht, hs⟩ := h2 a q hq hl hqv
    have hne : q.of ≠ pg.of := by
      intro heq
      obtain ⟨ℓg, _, hsg⟩ := h2 ag pg hpg hlg hvg
      rw [heq, hsg] at hs
      exact hae (SyncState.locked.inj hs).2.symm
    exact ⟨ℓ, ht, by simp [hne]; exact hs⟩

/-! ### lend and takePath -/

theorem T1_lend (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (hp0 : σ.paths a0 = some p0)
    (htv : σ.temporallyValid a0) (h : σ.T1) : (lend σ ℓ a0 p0).T1 :=
  T1_newpath σ a0 p0 _ hp0 rfl htv h
theorem T2_lend (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (h : σ.T2) (hf : σ.Fresh) : (lend σ ℓ a0 p0).T2 :=
  T2_newpath σ _ rfl h hf
theorem A_lend (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (hp0 : σ.paths a0 = some p0)
    (h : σ.A) (hf : σ.Fresh) : (lend σ ℓ a0 p0).A :=
  A_newpath σ a0 p0 _ hp0 rfl h hf
theorem Fresh_lend (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (hf : σ.Fresh) : (lend σ ℓ a0 p0).Fresh :=
  Fresh_newpath σ _ hf
theorem R_lend (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (h : σ.R) : (lend σ ℓ a0 p0).R := h
theorem C1_lend (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (lend σ ℓ a0 p0).C1 :=
  (C_newpath σ _ rfl hf h1 h2).1
theorem C2_lend (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (lend σ ℓ a0 p0).C2 :=
  (C_newpath σ _ rfl hf h1 h2).2

theorem takePath_paths (σ : State) (ℓ : Thread) (a t : Token) :
    (takePath σ ℓ a).paths t = (if t = a then (σ.paths t).map (fun p => { p with thread := some ℓ, taken := true }) else σ.paths t) := rfl

theorem takePath_core_fields (σ : State) (ℓ : Thread) (a : Token) :
    (takePath σ ℓ a).contents = σ.contents ∧ (takePath σ ℓ a).sync = σ.sync ∧ (takePath σ ℓ a).nextId = σ.nextId ∧
    (∀ i, (takePath σ ℓ a).alive i ↔ σ.alive i) := ⟨rfl, rfl, rfl, fun _ => Iff.rfl⟩

theorem T1_takePath (σ : State) (ℓ : Thread) (a : Token) (h : σ.T1) : (takePath σ ℓ a).T1 := by
  intro t p hp hv
  rw [takePath_paths] at hp
  show σ.alive p.of
  by_cases hta : t = a
  · subst hta
    simp at hp
    obtain ⟨q, hq, rfl⟩ := hp
    exact h t q hq hv
  · simp [hta] at hp
    exact h t p hp hv

theorem T2_takePath (σ : State) (ℓ : Thread) (a : Token) (h : σ.T2) : (takePath σ ℓ a).T2 := by
  intro t p hp i
  rw [takePath_paths] at hp
  show i ∈ p.heldBy ↔ (σ.alive i ∧ t ∈ σ.contents i)
  by_cases hta : t = a
  · subst hta
    simp at hp
    obtain ⟨q, hq, rfl⟩ := hp
    exact h t q hq i
  · simp [hta] at hp
    exact h t p hp i

theorem A_takePath (σ : State) (ℓ : Thread) (a : Token) (h : σ.A) : (takePath σ ℓ a).A := by
  intro t p b hp hb
  rw [takePath_paths] at hp
  rw [takePath_paths]
  by_cases hta : t = a
  · subst hta
    simp at hp
    obtain ⟨q, hq, rfl⟩ := hp
    simp at hb
    obtain ⟨hlt, hne⟩ := h t q b hq hb
    refine ⟨hlt, ?_⟩
    by_cases hbt : b = t
    · subst hbt
      simp [hq]
    · simpa [hbt] using hne
  · simp [hta] at hp
    obtain ⟨hlt, hne⟩ := h t p b hp hb
    refine ⟨hlt, ?_⟩
    by_cases hba : b = a
    · subst hba
      simp
      exact hne
    · simpa [hba] using hne

theorem Fresh_takePath (σ : State) (ℓ : Thread) (a : Token) (hf : σ.Fresh) : (takePath σ ℓ a).Fresh := by
  refine ⟨?_, ?_, ?_⟩
  · intro t p hp
    rw [takePath_paths] at hp
    show t < σ.nextId
    by_cases hta : t = a
    · subst hta
      simp at hp
      obtain ⟨q, hq, _⟩ := hp
      exact hf.1 t q hq
    · simp [hta] at hp
      exact hf.1 t p hp
  · intro i rec hi
    exact hf.2.1 i rec hi
  · intro i t ht
    exact hf.2.2 i t ht

theorem R_takePath (σ : State) (ℓ : Thread) (a : Token) (h : σ.R) : (takePath σ ℓ a).R := h

-- The taken path is not a lock path (`[Thread-Lend]` never lends one, D-0240): (C) is untouched.
theorem C1_takePath (σ : State) (ℓ : Thread) (a : Token) (p : PathRecord) (hp : σ.paths a = some p) (hl : p.lock = false)
    (h1 : σ.C1) : (takePath σ ℓ a).C1 := by
  intro o ℓ' t hs
  have hs' : σ.sync o = SyncState.locked ℓ' t := hs
  obtain ⟨q, hq, hql, rest⟩ := h1 o ℓ' t hs'
  rw [takePath_paths]
  have hta : t ≠ a := by
    intro heq
    subst heq
    rw [hp] at hq
    cases hq
    rw [hl] at hql
    cases hql
  exact ⟨q, by simp [hta, hq], hql, rest⟩

theorem C2_takePath (σ : State) (ℓ : Thread) (a : Token) (p : PathRecord) (hp : σ.paths a = some p) (hl : p.lock = false)
    (h2 : σ.C2) : (takePath σ ℓ a).C2 := by
  intro t q hq hql hv
  rw [takePath_paths] at hq
  show ∃ ℓ', q.thread = some ℓ' ∧ σ.sync q.of = SyncState.locked ℓ' t
  by_cases hta : t = a
  · subst hta
    simp at hq
    obtain ⟨r, hr, rfl⟩ := hq
    rw [hp] at hr
    cases hr
    simp at hql
    rw [hl] at hql
    cases hql
  · simp [hta] at hq
    exact h2 t q hq hql hv

/-! ### faultContain -/

theorem fcMid_paths (σ : State) (ℓ' : Thread) (tr : ThreadRec) (a : Token) :
    (fcMid σ ℓ' tr).paths a = ((endObjects σ (fcDead σ tr)).paths a).map
      (fun p => { p with valid := if p.lock = true ∧ p.thread = some ℓ' then false else p.valid }) := rfl

theorem fcMid_sync (σ : State) (ℓ' : Thread) (tr : ThreadRec) (o : Identity) :
    (fcMid σ ℓ' tr).sync o = (match σ.sync o with
      | SyncState.locked ℓ _ => if ℓ = ℓ' then SyncState.poisoned else σ.sync o
      | s => s) := rfl

theorem T1_fcMid (σ : State) (ℓ' : Thread) (tr : ThreadRec) (h : σ.T1) : (fcMid σ ℓ' tr).T1 := by
  intro a p hp hv
  rw [fcMid_paths] at hp
  simp at hp
  obtain ⟨q, hq, rfl⟩ := hp
  simp at hv
  show (endObjects σ (fcDead σ tr)).alive q.of
  exact T1_endObjects σ _ h a q hq hv.2

theorem T2_fcMid (σ : State) (ℓ' : Thread) (tr : ThreadRec) (h : σ.T2) : (fcMid σ ℓ' tr).T2 := by
  intro a p hp i
  rw [fcMid_paths] at hp
  simp at hp
  obtain ⟨q, hq, rfl⟩ := hp
  show i ∈ q.heldBy ↔ ((endObjects σ (fcDead σ tr)).alive i ∧ a ∈ (endObjects σ (fcDead σ tr)).contents i)
  exact T2_endObjects σ _ h a q hq i

theorem A_fcMid (σ : State) (ℓ' : Thread) (tr : ThreadRec) (h : σ.A) : (fcMid σ ℓ' tr).A := by
  intro a p b hp hb
  rw [fcMid_paths] at hp
  rw [fcMid_paths]
  simp at hp
  obtain ⟨q, hq, rfl⟩ := hp
  simp at hb
  obtain ⟨hlt, hne⟩ := A_endObjects σ _ h a q b hq hb
  refine ⟨hlt, ?_⟩
  simp
  exact hne

theorem Fresh_fcMid (σ : State) (ℓ' : Thread) (tr : ThreadRec) (hf : σ.Fresh) : (fcMid σ ℓ' tr).Fresh := by
  have h4 := Fresh_endObjects σ (fcDead σ tr) hf
  refine ⟨?_, ?_, ?_⟩
  · intro a p hp
    rw [fcMid_paths] at hp
    simp at hp
    obtain ⟨q, hq, _⟩ := hp
    exact h4.1 a q hq
  · intro i rec hi
    exact h4.2.1 i rec hi
  · intro i t ht
    exact h4.2.2 i t ht

theorem R_fcMid (σ : State) (ℓ' : Thread) (tr : ThreadRec) (hnh : σ.noHandle (fcDead σ tr)) (h : σ.R) : (fcMid σ ℓ' tr).R :=
  R_endObjects σ _ hnh h

theorem C1_fcMid (σ : State) (ℓ' : Thread) (tr : ThreadRec) (h1 : σ.C1) : (fcMid σ ℓ' tr).C1 := by
  intro o ℓ a hs
  rw [fcMid_sync] at hs
  have hs' : σ.sync o = SyncState.locked ℓ a := by
    cases hs0 : σ.sync o with
    | unlocked => rw [hs0] at hs; cases hs
    | poisoned => rw [hs0] at hs; cases hs
    | locked ℓ0 a0 =>
      rw [hs0] at hs
      by_cases he : ℓ0 = ℓ'
      · simp [he] at hs
      · simp [he] at hs
        obtain ⟨rfl, rfl⟩ := hs
        rfl
  obtain ⟨q, hq, hl, ho, ht, hm⟩ := h1 o ℓ a hs'
  rw [fcMid_paths]
  simp [endObjects, hq, hl, ho, ht, hm]

theorem C2_fcMid (σ : State) (ℓ' : Thread) (tr : ThreadRec) (h2 : σ.C2) : (fcMid σ ℓ' tr).C2 := by
  intro a p hp hl hv
  rw [fcMid_paths] at hp
  simp [endObjects] at hp
  obtain ⟨q, hq, rfl⟩ := hp
  simp at hl hv
  obtain ⟨hnot, _, hqv⟩ := hv
  obtain ⟨ℓq, ht, hs⟩ := h2 a q hq hl hqv
  have hne : ℓq ≠ ℓ' := by
    intro heq
    subst heq
    rcases hnot with h | h
    · rw [hl] at h
      cases h
    · exact h ht
  refine ⟨ℓq, ht, ?_⟩
  show (match σ.sync q.of with
      | SyncState.locked ℓ _ => if ℓ = ℓ' then SyncState.poisoned else σ.sync q.of
      | s => s) = SyncState.locked ℓq a
  rw [hs]
  simp [hne]

theorem R_faultContain (σ : State) (ℓ' : Thread) (tr : ThreadRec) (htr : σ.threads ℓ' = some tr) (hdone : tr.done = none)
    (hnh : σ.noHandle (fcDead σ tr)) (h : σ.R) : (faultContain σ ℓ' tr).R := by
  have hR := R_fcMid σ ℓ' tr hnh h
  intro o ho
  have ho1 : (fcMid σ ℓ' tr).obligation o = true := ho
  obtain ⟨rec, ℓ'', tr'', hr, ha, hau, htr'', how⟩ := hR o ho1
  have hoe : (endObjects σ (fcDead σ tr)).obligation o = true := ho1
  obtain ⟨hnd, _⟩ := obligation_endObjects σ _ o hoe
  have hr0 : σ.objects o = some rec := by
    have hr1 : (endObjects σ (fcDead σ tr)).objects o = some rec := hr
    rwa [objects_endObjects_live σ _ o hnd] at hr1
  have htr0 : σ.threads ℓ'' = some tr'' := htr''
  by_cases hle : ℓ'' = ℓ'
  · subst hle
    have htt : tr'' = tr := by
      rw [htr0] at htr
      cases htr
      rfl
    subst htt
    rcases how with ⟨g, hg, hm⟩ | ⟨hf, hm⟩ | ⟨res', hd', _, _⟩ | h4
    · exfalso
      simp [fcDead, hr0, hg, hm] at hnd
    · exfalso
      simp [fcDead, hr0, hf, hm] at hnd
    · exfalso
      rw [hdone] at hd'
      cases hd'
    · exact ⟨rec, ℓ'', { tr'' with frames := [], scopes := [], done := some [] }, hr, ha, hau,
        by simp [faultContain, mapThread, htr''], Or.inr (Or.inr (Or.inr h4))⟩
  · refine ⟨rec, ℓ'', tr'', hr, ha, hau, by simp [faultContain, mapThread, hle, htr''], ?_⟩
    exact Owner_mono _ _ o rec tr'' (fun hb => hb) how

/-! ### the theorem -/


/-! ### (P): a pending path is held by nothing; (RS): a sub-obligation's container is under obligation -/

theorem Core.P {σ σ' : State} (c : Core σ σ') (h : σ.P) : σ'.P := by
  intro a p hp hpend
  rw [c.paths] at hp
  exact h a p hp hpend

-- A map over the paths that keeps `pending` and an empty `heldBy` keeps P.
theorem P_pathmap (σ : State) (f : Token → PathRecord → PathRecord)
    (hkeep : ∀ t q, (f t q).pending = q.pending ∧ (q.heldBy = [] → (f t q).heldBy = [])) (h : σ.P) :
    (∀ a q, (σ.paths a).map (f a) = some q → q.pending = true → q.heldBy = []) := by
  intro a q hq hpend
  simp at hq
  obtain ⟨r, hr, rfl⟩ := hq
  obtain ⟨k1, k2⟩ := hkeep a r
  rw [k1] at hpend
  exact k2 (h a r hr hpend)

theorem P_newpath (σ : State) (p : PathRecord) (hp : p.pending = true → p.heldBy = []) (h : σ.P) :
    (∀ a q, (if a = σ.nextId then some p else σ.paths a) = some q → q.pending = true → q.heldBy = []) := by
  intro a q hq hpend
  by_cases hae : a = σ.nextId
  · subst hae
    simp at hq
    subst hq
    exact hp hpend
  · simp [hae] at hq
    exact h a q hq hpend

theorem P_establish (σ : State) (ℓ : Thread) (r : Bool) (h : σ.P) : (establish σ ℓ r).P := h
theorem P_bind (σ : State) (ℓ : Thread) (o : Identity) (h : σ.P) : (bind σ ℓ o).P := P_newpath σ _ (fun _ => rfl) h
theorem P_borrow (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (m : Mode) (pend : Bool) (h : σ.P) :
    (borrow σ ℓ a0 p0 m pend).P := P_newpath σ _ (fun _ => rfl) h
theorem P_lend (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (h : σ.P) : (lend σ ℓ a0 p0).P :=
  P_newpath σ _ (fun _ => rfl) h
theorem P_project (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (f : Proj) (h : σ.P) : (project σ ℓ a0 p0 f).P :=
  P_newpath σ _ (fun _ => rfl) h
theorem P_tempRoot (σ : State) (ℓ : Thread) (o : Identity) (h : σ.P) : (tempRoot σ ℓ o).P := P_newpath σ _ (fun _ => rfl) h

theorem P_writeWith (σ : State) (o : Identity) (refs : List Token) (ho : σ.alive o) (h2 : σ.T2)
    (hrefs : ∀ t ∈ refs, ∀ q, σ.paths t = some q → q.pending = false) (h : σ.P) : (writeWith σ o refs).P := by
  intro a p hp hpend
  simp [writeWith] at hp
  obtain ⟨q, hq, rfl⟩ := hp
  have hpend' : q.pending = true := hpend
  have hq0 : q.heldBy = [] := h a q hq hpend'
  by_cases har : a ∈ refs
  · have := hrefs a har q hq
    rw [this] at hpend'
    cases hpend'
  · simp [har, hq0]

-- The references an object's contents hold are held, so not pending (T2 with P).
theorem contents_not_pending (σ : State) (o : Identity) (ho : σ.alive o) (h2 : σ.T2) (h : σ.P)
    (t : Token) (ht : t ∈ σ.contents o) (q : PathRecord) (hq : σ.paths t = some q) : q.pending = false := by
  by_cases hpend : q.pending = true
  · have hm : o ∈ q.heldBy := (h2 t q hq o).2 ⟨ho, ht⟩
    rw [h t q hq hpend] at hm
    cases hm
  · simpa using hpend

theorem P_write (σ : State) (o : Identity) (removed added : List Token) (ho : σ.alive o) (h2 : σ.T2)
    (hadded : ∀ t ∈ added, ∃ q, σ.paths t = some q ∧ q.pending = false) (h : σ.P) : (write σ o removed added).P := by
  refine P_writeWith σ o _ ho h2 ?_ h
  intro t ht q hq
  rw [List.mem_append] at ht
  rcases ht with hc | ha
  · exact contents_not_pending σ o ho h2 h t (List.mem_filter.1 hc).1 q hq
  · obtain ⟨q', hq', hpq⟩ := hadded t ha
    rw [hq] at hq'
    cases hq'
    exact hpq

theorem P_endObjects (σ : State) (dead : Identity → Bool) (h : σ.P) : (endObjects σ dead).P :=
  P_pathmap σ (fun _ p => { p with valid := if dead p.of then false else p.valid, heldBy := p.heldBy.filter (fun o => !dead o) })
    (fun _ q => ⟨rfl, fun hq => by simp [hq]⟩) h
theorem P_invalidateUnheld (σ : State) (t : Scope) (h : σ.P) : (invalidateUnheld σ t).P :=
  P_pathmap σ (fun a p => { p with valid := if p.scope = t ∧ p.heldBy = [] ∧ σ.holder p.of ≠ some a then false else p.valid })
    (fun _ _ => ⟨rfl, fun hq => hq⟩) h
theorem P_transfer (σ : State) (a1 a2 : Token) (ℓ2 : Thread) (f2 : Frame) (o : Identity) (h : σ.P) : (transfer σ a1 a2 ℓ2 f2 o).P := by
  intro a p hp hpend
  rw [transfer_paths] at hp
  by_cases hae : a = a1
  · subst hae
    simp at hp
    obtain ⟨q, hq, rfl⟩ := hp
    exact h a q hq hpend
  · simp [hae] at hp
    exact h a p hp hpend
theorem P_takePath (σ : State) (ℓ : Thread) (a : Token) (h : σ.P) : (takePath σ ℓ a).P := by
  intro t p hp hpend
  rw [takePath_paths] at hp
  by_cases hte : t = a
  · subst hte
    simp at hp
    obtain ⟨q, hq, rfl⟩ := hp
    exact h t q hq hpend
  · simp [hte] at hp
    exact h t p hp hpend
theorem P_settle (σ : State) (a : Token) (h : σ.P) : (settle σ a).P := by
  intro t p hp hpend
  by_cases hte : t = a
  · subst hte
    simp [settle] at hp
    obtain ⟨q, _, rfl⟩ := hp
    cases hpend
  · simp [settle, hte] at hp
    exact h t p hp hpend
theorem P_fcMid (σ : State) (ℓ' : Thread) (tr : ThreadRec) (h : σ.P) : (fcMid σ ℓ' tr).P := by
  intro a p hp hpend
  rw [fcMid_paths] at hp
  simp at hp
  obtain ⟨q, hq, rfl⟩ := hp
  exact P_endObjects σ _ h a q hq hpend
theorem P_guardDrop (σ : State) (g : Identity) (ag : Token) (o : Identity) (h : σ.P) : (guardDrop σ g ag o).P := by
  intro a p hp hpend
  rw [guardDrop_paths] at hp
  have he := P_endObjects σ (fun i => decide (i = g)) h
  by_cases hae : a = ag
  · subst hae
    simp at hp
    obtain ⟨q, hq, rfl⟩ := hp
    exact he a q hq hpend
  · simp [hae] at hp
    exact he a p hp hpend
theorem P_lock (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) (h : σ.P) : (lock σ ℓ am pm).P := by
  intro a p hp hpend
  rw [lock_paths] at hp
  by_cases hae : a = σ.nextId
  · subst hae
    simp at hp
    subst hp
    cases hpend
  · simp [hae] at hp
    exact h a p hp hpend

theorem RS_restamp (σ : State) (sel : Identity → Bool) (fr : Option Frame) (sc : Option Scope) (h : σ.RS) :
    (restamp σ sel fr sc).RS := h
theorem RS_setThread (σ : State) (ℓ : Thread) (tr : Option ThreadRec) (h : σ.RS) : (setThread σ ℓ tr).RS := h
theorem RS_mapThread (σ : State) (ℓ : Thread) (f : ThreadRec → ThreadRec) (h : σ.RS) : (mapThread σ ℓ f).RS := h
theorem RS_rekey (σ : State) (sel : Identity → Bool) (ℓ : Thread) (h : σ.RS) : (rekey σ sel ℓ).RS := by
  intro o q ℓ' hs
  by_cases hsel : sel o = true
  · simp [rekey, hsel] at hs
    obtain ⟨⟨ℓ0, hs0⟩, rfl⟩ := hs
    obtain ⟨hq, hob, _⟩ := h o q ℓ0 hs0
    exact ⟨hq, hob, by simp [rekey, hsel]⟩
  · simp [rekey, hsel] at hs
    obtain ⟨hq, hob, hau⟩ := h o q ℓ' hs
    exact ⟨hq, hob, by simp [rekey, hsel, hau]⟩
theorem RS_endObjects (σ : State) (dead : Identity → Bool) (h : σ.RS) : (endObjects σ dead).RS := by
  intro o q ℓ hs
  by_cases hd : dead o = true
  · simp [endObjects, hd] at hs
  · simp [endObjects, hd] at hs
    obtain ⟨hq, hob, hau⟩ := h o q ℓ hs
    exact ⟨hq, by simp [endObjects, hd, hob], hau⟩
theorem RS_establish (σ : State) (ℓ : Thread) (r : Bool) (h : σ.RS) : (establish σ ℓ r).RS := by
  intro o q ℓ' hs
  by_cases hoe : o = σ.nextId
  · simp [establish, hoe] at hs
  · simp [establish, hoe] at hs
    obtain ⟨hq, hob, hau⟩ := h o q ℓ' hs
    exact ⟨hq, by simp [establish, hoe, hob], by simp [establish, hoe, hau]⟩
theorem RS_bind (σ : State) (ℓ : Thread) (o : Identity) (h : σ.RS) : (bind σ ℓ o).RS := h
theorem RS_borrow (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (m : Mode) (pend : Bool) (h : σ.RS) :
    (borrow σ ℓ a0 p0 m pend).RS := h
theorem RS_lend (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (h : σ.RS) : (lend σ ℓ a0 p0).RS := h
theorem RS_project (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (f : Proj) (h : σ.RS) : (project σ ℓ a0 p0 f).RS := h
theorem RS_tempRoot (σ : State) (ℓ : Thread) (o : Identity) (h : σ.RS) : (tempRoot σ ℓ o).RS := h
theorem RS_writeWith (σ : State) (o : Identity) (refs : List Token) (h : σ.RS) : (writeWith σ o refs).RS := h
theorem RS_write (σ : State) (o : Identity) (removed added : List Token) (h : σ.RS) : (write σ o removed added).RS := h
theorem RS_invalidateUnheld (σ : State) (t : Scope) (h : σ.RS) : (invalidateUnheld σ t).RS := h
theorem RS_takePath (σ : State) (ℓ : Thread) (a : Token) (h : σ.RS) : (takePath σ ℓ a).RS := h
theorem RS_settle (σ : State) (a : Token) (h : σ.RS) : (settle σ a).RS := h
theorem RS_transfer (σ : State) (a1 a2 : Token) (ℓ2 : Thread) (f2 : Frame) (o : Identity) (h : σ.RS) :
    (transfer σ a1 a2 ℓ2 f2 o).RS := RS_restamp _ (fun i => decide (i = o)) (some f2) none (RS_rekey σ (fun i => decide (i = o)) ℓ2 h)
theorem RS_blockEnter (σ : State) (ℓ : Thread) (f : Frame) (h : σ.RS) : (blockEnter σ ℓ f).RS := h
theorem RS_blockExit (σ : State) (ℓ : Thread) (h : σ.RS) : (blockExit σ ℓ).RS := RS_mapThread _ ℓ _ (RS_endObjects σ _ h)
theorem RS_stmtEnter (σ : State) (ℓ : Thread) (t : Scope) (h : σ.RS) : (stmtEnter σ ℓ t).RS := h
theorem RS_stmtExit (σ : State) (ℓ : Thread) (h : σ.RS) : (stmtExit σ ℓ).RS :=
  RS_mapThread _ ℓ _ (RS_invalidateUnheld _ _ (RS_endObjects σ _ h))
theorem RS_spawn (σ : State) (ℓ ℓ' : Thread) (f' : Frame) (h : σ.RS) : (spawn σ ℓ ℓ' f').RS :=
  RS_setThread _ ℓ' _ (RS_establish σ ℓ true h)
theorem RS_threadDone (σ : State) (ℓ' : Thread) (res : List Identity) (h : σ.RS) : (threadDone σ ℓ' res).RS :=
  RS_mapThread _ ℓ' _ (RS_restamp _ (fun i => decide (i ∈ res)) none none (RS_endObjects σ _ h))
theorem RS_join (σ : State) (ℓ ℓ' : Thread) (hd : Identity) (res : List Identity) (h : σ.RS) : (join σ ℓ ℓ' hd res).RS :=
  RS_setThread _ ℓ' _ (RS_rekey _ (fun i => decide (i ∈ res)) ℓ (RS_restamp _ (fun i => decide (i ∈ res)) none (some (σ.scope ℓ)) (RS_endObjects σ _ h)))
theorem RS_handleDestroy (σ : State) (ℓ' : Thread) (hd : Identity) (res : List Identity) (h : σ.RS) :
    (handleDestroy σ ℓ' hd res).RS := RS_setThread _ ℓ' _ (RS_endObjects σ _ h)
theorem RS_fcMid (σ : State) (ℓ' : Thread) (tr : ThreadRec) (h : σ.RS) : (fcMid σ ℓ' tr).RS := RS_endObjects σ _ h
theorem RS_faultContain (σ : State) (ℓ' : Thread) (tr : ThreadRec) (h : σ.RS) : (faultContain σ ℓ' tr).RS :=
  RS_mapThread _ ℓ' _ (RS_fcMid σ ℓ' tr h)
theorem RS_lock (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) (h : σ.RS) : (lock σ ℓ am pm).RS := by
  intro o q ℓ' hs
  by_cases hog : o = σ.nextId + 1
  · simp [lock, hog] at hs
  · simp [lock, hog] at hs
    obtain ⟨hq, hob, hau⟩ := h o q ℓ' hs
    exact ⟨hq, by simp [lock, hog, hob], by simp [lock, hog, hau]⟩
theorem RS_guardDrop (σ : State) (g : Identity) (ag : Token) (o : Identity) (h : σ.RS) : (guardDrop σ g ag o).RS := by
  intro i q ℓ hs
  have hs' : (endObjects σ (fun j => decide (j = g))).subs i q = some ℓ := hs
  obtain ⟨hq, hob, hau⟩ := RS_endObjects σ _ h i q ℓ hs'
  exact ⟨hq, hob, hau⟩
theorem RS_destroyComposite (σ : State) (o : Identity) (h : σ.RS) : (destroyComposite σ o).RS := by
  intro i q ℓ hs
  by_cases hio : i = o
  · simp [destroyComposite, hio] at hs
  · simp [destroyComposite, hio] at hs
    exact h i q ℓ hs


/-! ### temp-root: a root path for a temporary, nothing else -/

theorem tempRoot_paths (σ : State) (ℓ : Thread) (o : Identity) (a : Token) :
    (tempRoot σ ℓ o).paths a = (if a = σ.nextId then some { of := o, proj := [], mode := Mode.exclusive, valid := true, base := none, thread := some ℓ, lock := false, taken := false, pending := false, frame := σ.frame ℓ, scope := σ.scope ℓ, heldBy := [] } else σ.paths a) := rfl

theorem T1_tempRoot (σ : State) (ℓ : Thread) (o : Identity) (ho : σ.alive o) (h : σ.T1) : (tempRoot σ ℓ o).T1 := by
  intro a p hp hv
  rw [tempRoot_paths] at hp
  show σ.alive p.of
  by_cases hae : a = σ.nextId
  · subst hae
    simp at hp
    subst hp
    exact ho
  · simp [hae] at hp
    exact h a p hp hv
theorem T2_tempRoot (σ : State) (ℓ : Thread) (o : Identity) (h : σ.T2) (hf : σ.Fresh) : (tempRoot σ ℓ o).T2 :=
  T2_newpath σ _ rfl h hf
theorem A_tempRoot (σ : State) (ℓ : Thread) (o : Identity) (h : σ.A) (hf : σ.Fresh) : (tempRoot σ ℓ o).A := by
  intro a p b hp hb
  rw [tempRoot_paths] at hp
  rw [tempRoot_paths]
  by_cases hae : a = σ.nextId
  · subst hae
    simp at hp
    subst hp
    simp at hb
  · simp [hae] at hp
    obtain ⟨hlt, hne⟩ := h a p b hp hb
    refine ⟨hlt, ?_⟩
    have hbe : b ≠ σ.nextId := by
      intro heq
      subst heq
      exact absurd (Nat.lt_trans hlt (hf.1 a p hp)) (Nat.lt_irrefl _)
    simpa [hbe] using hne
theorem Fresh_tempRoot (σ : State) (ℓ : Thread) (o : Identity) (hf : σ.Fresh) : (tempRoot σ ℓ o).Fresh := Fresh_newpath σ _ hf
theorem R_tempRoot (σ : State) (ℓ : Thread) (o : Identity) (h : σ.R) : (tempRoot σ ℓ o).R := h
theorem C1_tempRoot (σ : State) (ℓ : Thread) (o : Identity) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (tempRoot σ ℓ o).C1 :=
  (C_newpath σ _ rfl hf h1 h2).1
theorem C2_tempRoot (σ : State) (ℓ : Thread) (o : Identity) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (tempRoot σ ℓ o).C2 :=
  (C_newpath σ _ rfl hf h1 h2).2

/-! ### the parameter binding of a call: a parameter object holding the argument's path -/

theorem bindParam_paths (σ : State) (ℓ : Thread) (a t : Token) :
    (bindParam σ ℓ a).paths t = (if t = σ.nextId + 1 then some { of := σ.nextId, proj := [], mode := Mode.exclusive, valid := true, base := none, thread := some ℓ, lock := false, taken := false, pending := false, frame := σ.frame ℓ, scope := σ.scope ℓ, heldBy := [] }
      else if t = a then (σ.paths t).map (fun p => { p with pending := false, heldBy := σ.nextId :: p.heldBy }) else σ.paths t) := rfl

theorem alive_bindParam_iff (σ : State) (ℓ : Thread) (a : Token) (hf : σ.Fresh) (i : Identity) :
    (bindParam σ ℓ a).alive i ↔ (σ.alive i ∨ i = σ.nextId) := by
  constructor
  · intro ⟨q, hq, ha⟩
    by_cases hie : i = σ.nextId
    · exact Or.inr hie
    · exact Or.inl ⟨q, by simpa [bindParam, hie] using hq, ha⟩
  · intro h
    rcases h with ⟨q, hq, ha⟩ | rfl
    · have hne : i ≠ σ.nextId := by
        intro heq
        rw [heq, fresh_obj_none σ hf] at hq
        cases hq
      exact ⟨q, by simp [bindParam, hne, hq], ha⟩
    · exact ⟨{ alive := true, isResource := false, guard := false, reclaimed := false, frame := some (σ.frame ℓ), scope := σ.scope ℓ }, by simp [bindParam], rfl⟩

theorem T1_bindParam (σ : State) (ℓ : Thread) (a : Token) (hf : σ.Fresh) (h : σ.T1) : (bindParam σ ℓ a).T1 := by
  intro t p hp hv
  rw [bindParam_paths] at hp
  rw [alive_bindParam_iff σ ℓ a hf]
  by_cases hte : t = σ.nextId + 1
  · subst hte
    simp at hp
    subst hp
    exact Or.inr rfl
  · by_cases hta : t = a
    · subst hta
      simp [hte] at hp
      obtain ⟨q, hq, rfl⟩ := hp
      exact Or.inl (h t q hq hv)
    · simp [hte, hta] at hp
      exact Or.inl (h t p hp hv)

theorem T2_bindParam (σ : State) (ℓ : Thread) (a : Token) (p0 : PathRecord) (hp0 : σ.paths a = some p0) (hf : σ.Fresh) (h : σ.T2) :
    (bindParam σ ℓ a).T2 := by
  intro t p hp i
  rw [bindParam_paths] at hp
  rw [alive_bindParam_iff σ ℓ a hf]
  have hc : (bindParam σ ℓ a).contents i = (if i = σ.nextId then [a] else σ.contents i) := rfl
  rw [hc]
  by_cases hte : t = σ.nextId + 1
  · subst hte
    simp at hp
    subst hp
    simp only [List.not_mem_nil, false_iff]
    intro ⟨_, hm⟩
    by_cases hie : i = σ.nextId
    · subst hie
      simp at hm
      have := hf.1 a p0 hp0
      subst hm
      exact succ_not_lt _ this
    · simp [hie] at hm
      exact succ_not_lt _ (hf.2.2 i _ hm)
  · by_cases hta : t = a
    · subst hta
      simp [hte] at hp
      obtain ⟨q, hq, rfl⟩ := hp
      show i ∈ σ.nextId :: q.heldBy ↔ _
      by_cases hie : i = σ.nextId
      · subst hie
        simp
      · simp [hie]
        exact h t q hq i
    · simp [hte, hta] at hp
      by_cases hie : i = σ.nextId
      · subst hie
        simp [hta]
        intro hm
        obtain ⟨⟨r, hr, _⟩, _⟩ := (h t p hp σ.nextId).1 hm
        rw [fresh_obj_none σ hf] at hr
        cases hr
      · simp [hie]
        exact h t p hp i

theorem A_bindParam (σ : State) (ℓ : Thread) (a : Token) (hf : σ.Fresh) (h : σ.A) : (bindParam σ ℓ a).A := by
  intro t p b hp hb
  rw [bindParam_paths] at hp
  rw [bindParam_paths]
  by_cases hte : t = σ.nextId + 1
  · subst hte
    simp at hp
    subst hp
    simp at hb
  · have hp' : ∃ q, σ.paths t = some q ∧ q.base = p.base := by
      by_cases hta : t = a
      · subst hta
        simp [hte] at hp
        obtain ⟨q, hq, rfl⟩ := hp
        exact ⟨q, hq, rfl⟩
      · simp [hte, hta] at hp
        exact ⟨p, hp, rfl⟩
    obtain ⟨q, hq, hqb⟩ := hp'
    rw [← hqb] at hb
    obtain ⟨hlt, hne⟩ := h t q b hq hb
    refine ⟨hlt, ?_⟩
    have hb1 : b ≠ σ.nextId + 1 := by
      intro heq
      subst heq
      exact succ_not_lt _ (Nat.lt_trans hlt (hf.1 t q hq))
    by_cases hba : b = a
    · subst hba
      simp [hb1]
      exact hne
    · simpa [hb1, hba] using hne

theorem Fresh_bindParam (σ : State) (ℓ : Thread) (a : Token) (p0 : PathRecord) (hp0 : σ.paths a = some p0) (hf : σ.Fresh) :
    (bindParam σ ℓ a).Fresh := by
  refine ⟨?_, ?_, ?_⟩
  · intro t p hp
    rw [bindParam_paths] at hp
    show t < σ.nextId + 1 + 1
    by_cases hte : t = σ.nextId + 1
    · subst hte
      exact Nat.lt_succ_self _
    · by_cases hta : t = a
      · subst hta
        exact Nat.lt_succ_of_lt (Nat.lt_succ_of_lt (hf.1 t p0 hp0))
      · simp [hte, hta] at hp
        exact Nat.lt_succ_of_lt (Nat.lt_succ_of_lt (hf.1 t p hp))
  · intro i rec hi
    show i < σ.nextId + 1 + 1
    by_cases hie : i = σ.nextId
    · subst hie
      exact Nat.lt_succ_of_lt (Nat.lt_succ_self _)
    · have hi' : σ.objects i = some rec := by simpa [bindParam, hie] using hi
      exact Nat.lt_succ_of_lt (Nat.lt_succ_of_lt (hf.2.1 i rec hi'))
  · intro i t ht
    show t < σ.nextId + 1 + 1
    by_cases hie : i = σ.nextId
    · subst hie
      simp [bindParam] at ht
      subst ht
      exact Nat.lt_succ_of_lt (Nat.lt_succ_of_lt (hf.1 _ p0 hp0))
    · have ht' : t ∈ σ.contents i := by simpa [bindParam, hie] using ht
      exact Nat.lt_succ_of_lt (Nat.lt_succ_of_lt (hf.2.2 i t ht'))

theorem R_bindParam (σ : State) (ℓ : Thread) (a : Token) (hf : σ.Fresh) (h : σ.R) : (bindParam σ ℓ a).R := by
  intro i hi
  have hne : i ≠ σ.nextId := by
    intro heq
    subst heq
    simp [bindParam] at hi
  have hi' : σ.obligation i = true := by simpa [bindParam, hne] using hi
  obtain ⟨rec, ℓ', tr, hr, ha, hau, htr, how⟩ := h i hi'
  refine ⟨rec, ℓ', tr, by simp [bindParam, hne, hr], ha, by simp [bindParam, hne, hau], htr, ?_⟩
  refine Owner_mono σ _ i rec tr ?_ how
  intro hb
  have hhn : tr.handle ≠ σ.nextId := obligation_ne_next σ hf h tr.handle hb
  simp [bindParam, hhn, hb]

theorem C1_bindParam (σ : State) (ℓ : Thread) (a : Token) (hf : σ.Fresh) (h1 : σ.C1) : (bindParam σ ℓ a).C1 := by
  intro o ℓ' t hs
  have hs' : σ.sync o = SyncState.locked ℓ' t := hs
  obtain ⟨q, hq, hql, hqo, hqt, hqm⟩ := h1 o ℓ' t hs'
  rw [bindParam_paths]
  have hte : t ≠ σ.nextId + 1 := by
    intro heq
    subst heq
    exact succ_not_lt _ (hf.1 _ q hq)
  by_cases hta : t = a
  · subst hta
    exact ⟨{ q with pending := false, heldBy := σ.nextId :: q.heldBy }, by simp [hte, hq], hql, hqo, hqt, hqm⟩
  · exact ⟨q, by simp [hte, hta, hq], hql, hqo, hqt, hqm⟩

theorem C2_bindParam (σ : State) (ℓ : Thread) (a : Token) (h2 : σ.C2) : (bindParam σ ℓ a).C2 := by
  intro t q hq hql hv
  rw [bindParam_paths] at hq
  show ∃ ℓ', q.thread = some ℓ' ∧ σ.sync q.of = SyncState.locked ℓ' t
  by_cases hte : t = σ.nextId + 1
  · subst hte
    simp at hq
    subst hq
    cases hql
  · by_cases hta : t = a
    · subst hta
      simp [hte] at hq
      obtain ⟨r, hr, rfl⟩ := hq
      exact h2 t r hr hql hv
    · simp [hte, hta] at hq
      exact h2 t q hq hql hv

theorem P_bindParam (σ : State) (ℓ : Thread) (a : Token) (h : σ.P) : (bindParam σ ℓ a).P := by
  intro t q hq hpend
  rw [bindParam_paths] at hq
  by_cases hte : t = σ.nextId + 1
  · subst hte
    simp at hq
    subst hq
    cases hpend
  · by_cases hta : t = a
    · subst hta
      simp [hte] at hq
      obtain ⟨r, _, rfl⟩ := hq
      cases hpend
    · simp [hte, hta] at hq
      exact h t q hq hpend

theorem RS_bindParam (σ : State) (ℓ : Thread) (a : Token) (h : σ.RS) : (bindParam σ ℓ a).RS := by
  intro i q ℓ' hs
  by_cases hie : i = σ.nextId
  · simp [bindParam, hie] at hs
  · simp [bindParam, hie] at hs
    obtain ⟨hq, hob, hau⟩ := h i q ℓ' hs
    exact ⟨hq, by simp [bindParam, hie, hob], by simp [bindParam, hie, hau]⟩

/-! ### settle: the one-access path ends -/

theorem settle_paths (σ : State) (a t : Token) :
    (settle σ a).paths t = (if t = a then (σ.paths t).map (fun p => { p with pending := false, valid := false }) else σ.paths t) := rfl

theorem T1_settle (σ : State) (a : Token) (h : σ.T1) : (settle σ a).T1 := by
  intro t p hp hv
  rw [settle_paths] at hp
  show σ.alive p.of
  by_cases hta : t = a
  · subst hta
    simp at hp
    obtain ⟨q, _, rfl⟩ := hp
    cases hv
  · simp [hta] at hp
    exact h t p hp hv

theorem T2_settle (σ : State) (a : Token) (h : σ.T2) : (settle σ a).T2 := by
  intro t p hp i
  rw [settle_paths] at hp
  show i ∈ p.heldBy ↔ (σ.alive i ∧ t ∈ σ.contents i)
  by_cases hta : t = a
  · subst hta
    simp at hp
    obtain ⟨q, hq, rfl⟩ := hp
    exact h t q hq i
  · simp [hta] at hp
    exact h t p hp i

theorem A_settle (σ : State) (a : Token) (h : σ.A) : (settle σ a).A := by
  intro t p b hp hb
  rw [settle_paths] at hp
  rw [settle_paths]
  by_cases hta : t = a
  · subst hta
    simp at hp
    obtain ⟨q, hq, rfl⟩ := hp
    simp at hb
    obtain ⟨hlt, hne⟩ := h t q b hq hb
    refine ⟨hlt, ?_⟩
    by_cases hbt : b = t
    · subst hbt
      simp [hq]
    · simpa [hbt] using hne
  · simp [hta] at hp
    obtain ⟨hlt, hne⟩ := h t p b hp hb
    refine ⟨hlt, ?_⟩
    by_cases hba : b = a
    · subst hba
      simp
      exact hne
    · simpa [hba] using hne

theorem Fresh_settle (σ : State) (a : Token) (hf : σ.Fresh) : (settle σ a).Fresh := by
  refine ⟨?_, ?_, ?_⟩
  · intro t p hp
    rw [settle_paths] at hp
    show t < σ.nextId
    by_cases hta : t = a
    · subst hta
      simp at hp
      obtain ⟨q, hq, _⟩ := hp
      exact hf.1 t q hq
    · simp [hta] at hp
      exact hf.1 t p hp
  · intro i rec hi
    exact hf.2.1 i rec hi
  · intro i t ht
    exact hf.2.2 i t ht

theorem R_settle (σ : State) (a : Token) (h : σ.R) : (settle σ a).R := h

theorem C1_settle (σ : State) (a : Token) (h1 : σ.C1) : (settle σ a).C1 := by
  intro o ℓ' t hs
  have hs' : σ.sync o = SyncState.locked ℓ' t := hs
  obtain ⟨q, hq, hql, hqo, hqt, hqm⟩ := h1 o ℓ' t hs'
  rw [settle_paths]
  by_cases hta : t = a
  · subst hta
    exact ⟨{ q with pending := false, valid := false }, by simp [hq], hql, hqo, hqt, hqm⟩
  · exact ⟨q, by simp [hta, hq], hql, hqo, hqt, hqm⟩

theorem C2_settle (σ : State) (a : Token) (h2 : σ.C2) : (settle σ a).C2 := by
  intro t q hq hql hv
  rw [settle_paths] at hq
  show ∃ ℓ', q.thread = some ℓ' ∧ σ.sync q.of = SyncState.locked ℓ' t
  by_cases hta : t = a
  · subst hta
    simp at hq
    obtain ⟨r, _, rfl⟩ := hq
    cases hv
  · simp [hta] at hq
    exact h2 t q hq hql hv

/-! ### destroy-composite: the sub-obligations discharged, nothing else -/

theorem core_destroyComposite (σ : State) (o : Identity) : Core σ (destroyComposite σ o) where
  paths := rfl
  contents := rfl
  sync := rfl
  nextId := rfl
  alive := fun _ => Iff.rfl
  dom := fun i r h => ⟨r, h⟩

theorem R_destroyComposite (σ : State) (o : Identity) (h : σ.R) : (destroyComposite σ o).R := h

/-! ### relocate-in: a write of the moved references into the container, the sub-obligations re-keyed, the source ended -/

theorem relocateIn_core (σ : State) (oS o : Identity) (path : List Proj) :
    Core (writeWith σ o (σ.contents o ++ σ.contents oS))
      { writeWith σ o (σ.contents o ++ σ.contents oS) with subs := relocInSubs σ oS o path } where
  paths := rfl
  contents := rfl
  sync := rfl
  nextId := rfl
  alive := fun _ => Iff.rfl
  dom := fun i r h => ⟨r, h⟩

theorem RS_relocInSubs (σ : State) (ℓ : Thread) (oS o : Identity) (path : List Proj) (hpath : path ≠ [])
    (hob : σ.obligation o = true) (hau : σ.authority o = some ℓ) (hauS : σ.obligation oS = true → σ.authority oS = some ℓ)
    (h : σ.RS) :
    ({ writeWith σ o (σ.contents o ++ σ.contents oS) with subs := relocInSubs σ oS o path } : State).RS := by
  intro i q ℓ' hs
  have hs' : relocInSubs σ oS o path i q = some ℓ' := hs
  show q ≠ [] ∧ σ.obligation i = true ∧ σ.authority i = some ℓ'
  unfold relocInSubs at hs'
  by_cases hio : i = o
  · rw [hio] at hs' ⊢
    rw [if_pos rfl] at hs'
    by_cases hpre : path.isPrefixOf q = true
    · rw [if_pos hpre] at hs'
      have hq : q ≠ [] := by
        intro hq
        subst hq
        cases path with
        | nil => exact hpath rfl
        | cons x xs => simp [List.isPrefixOf] at hpre
      by_cases hqp : q = path
      · rw [if_pos hqp] at hs'
        by_cases hobS : σ.obligation oS = true
        · rw [if_pos hobS, hauS hobS] at hs'
          cases hs'
          exact ⟨hq, hob, hau⟩
        · rw [if_neg hobS] at hs'
          cases hs'
      · rw [if_neg hqp] at hs'
        obtain ⟨_, hobS, hauS'⟩ := h oS _ ℓ' hs'
        rw [hauS hobS] at hauS'
        cases hauS'
        exact ⟨hq, hob, hau⟩
    · rw [if_neg hpre] at hs'
      exact h o q ℓ' hs'
  · rw [if_neg hio] at hs'
    exact h i q ℓ' hs'

theorem relocateIn_refs_lt (σ : State) (oS o : Identity) (hf : σ.Fresh) :
    ∀ t ∈ σ.contents o ++ σ.contents oS, t < σ.nextId := by
  intro t ht
  rw [List.mem_append] at ht
  rcases ht with h1 | h1
  · exact hf.2.2 o t h1
  · exact hf.2.2 oS t h1

theorem relocateIn_refs_bound (σ : State) (oS o : Identity) (hoS : σ.alive oS) (ho : σ.alive o) (h2 : σ.T2) (h8 : σ.P) :
    ∀ t ∈ σ.contents o ++ σ.contents oS, ∀ q, σ.paths t = some q → q.pending = false := by
  intro t ht q hq
  rw [List.mem_append] at ht
  rcases ht with h1 | h1
  · exact contents_not_pending σ o ho h2 h8 t h1 q hq
  · exact contents_not_pending σ oS hoS h2 h8 t h1 q hq

theorem T1_relocateIn (σ : State) (oS o : Identity) (path : List Proj) (h : σ.T1) : (relocateIn σ oS o path).T1 :=
  T1_endObjects _ _ ((relocateIn_core σ oS o path).T1 (T1_writeWith σ o _ h))
theorem T2_relocateIn (σ : State) (oS o : Identity) (path : List Proj) (ho : σ.alive o) (h : σ.T2) : (relocateIn σ oS o path).T2 :=
  T2_endObjects _ _ ((relocateIn_core σ oS o path).T2 (T2_writeWith σ o _ ho h))
theorem A_relocateIn (σ : State) (oS o : Identity) (path : List Proj) (h : σ.A) : (relocateIn σ oS o path).A :=
  A_endObjects _ _ ((relocateIn_core σ oS o path).A (A_writeWith σ o _ h))
theorem Fresh_relocateIn (σ : State) (oS o : Identity) (path : List Proj) (hf : σ.Fresh) : (relocateIn σ oS o path).Fresh :=
  Fresh_endObjects _ _ ((relocateIn_core σ oS o path).Fresh (Fresh_writeWith σ o _ (relocateIn_refs_lt σ oS o hf) hf))
theorem R_relocateIn (σ : State) (oS o : Identity) (path : List Proj) (hnh : σ.noHandle (fun i => decide (i = oS))) (h : σ.R) :
    (relocateIn σ oS o path).R :=
  R_endObjects _ _ hnh h
theorem C1_relocateIn (σ : State) (oS o : Identity) (path : List Proj) (h1 : σ.C1) (h2 : σ.C2) : (relocateIn σ oS o path).C1 :=
  C1_endObjects _ _ ((relocateIn_core σ oS o path).C1 (C1_writeWith σ o _ h1 h2)) ((relocateIn_core σ oS o path).C2 (C2_writeWith σ o _ h1 h2))
theorem C2_relocateIn (σ : State) (oS o : Identity) (path : List Proj) (h1 : σ.C1) (h2 : σ.C2) : (relocateIn σ oS o path).C2 :=
  C2_endObjects _ _ ((relocateIn_core σ oS o path).C1 (C1_writeWith σ o _ h1 h2)) ((relocateIn_core σ oS o path).C2 (C2_writeWith σ o _ h1 h2))
theorem P_relocateIn (σ : State) (oS o : Identity) (path : List Proj) (hoS : σ.alive oS) (ho : σ.alive o) (h2 : σ.T2) (h8 : σ.P) :
    (relocateIn σ oS o path).P :=
  P_endObjects _ _ ((relocateIn_core σ oS o path).P (P_writeWith σ o _ ho h2 (relocateIn_refs_bound σ oS o hoS ho h2 h8) h8))
theorem RS_relocateIn (σ : State) (ℓ : Thread) (oS o : Identity) (path : List Proj) (hpath : path ≠ [])
    (hob : σ.obligation o = true) (hau : σ.authority o = some ℓ) (hauS : σ.obligation oS = true → σ.authority oS = some ℓ)
    (h : σ.RS) : (relocateIn σ oS o path).RS :=
  RS_endObjects _ _ (RS_relocInSubs σ ℓ oS o path hpath hob hau hauS h)

/-! ### relocate-out: a fresh temporary established, the moved references written into it, the sub-obligations re-keyed -/

theorem relocateOut_core (σ : State) (ℓ : Thread) (o : Identity) (path : List Proj) (moved : List Token) :
    Core (writeWith (writeWith (establish σ ℓ true) o ((σ.contents o).filter (· ∉ moved))) σ.nextId moved)
      (relocateOut σ ℓ o path moved) where
  paths := rfl
  contents := rfl
  sync := rfl
  nextId := rfl
  alive := fun _ => Iff.rfl
  dom := fun i r h => ⟨r, h⟩

theorem relocateOut_alive1 (σ : State) (ℓ : Thread) (o : Identity) (ho : σ.alive o) (hf : σ.Fresh) : (establish σ ℓ true).alive o :=
  (alive_establish_iff σ ℓ true hf o).2 (Or.inl ho)
theorem relocateOut_alive2 (σ : State) (ℓ : Thread) (o : Identity) (moved : List Token) (hf : σ.Fresh) :
    (writeWith (establish σ ℓ true) o ((σ.contents o).filter (· ∉ moved))).alive σ.nextId :=
  (alive_writeWith_iff _ _ _ _).2 ((alive_establish_iff σ ℓ true hf _).2 (Or.inr rfl))

theorem T1_relocateOut (σ : State) (ℓ : Thread) (o : Identity) (path : List Proj) (moved : List Token) (hf : σ.Fresh) (h : σ.T1) :
    (relocateOut σ ℓ o path moved).T1 :=
  (relocateOut_core σ ℓ o path moved).T1 (T1_writeWith _ _ _ (T1_writeWith _ _ _ (T1_establish σ ℓ true h hf)))
theorem T2_relocateOut (σ : State) (ℓ : Thread) (o : Identity) (path : List Proj) (moved : List Token) (ho : σ.alive o) (hf : σ.Fresh) (h : σ.T2) :
    (relocateOut σ ℓ o path moved).T2 :=
  (relocateOut_core σ ℓ o path moved).T2 (T2_writeWith _ _ _ (relocateOut_alive2 σ ℓ o moved hf)
    (T2_writeWith _ _ _ (relocateOut_alive1 σ ℓ o ho hf) (T2_establish σ ℓ true h hf)))
theorem A_relocateOut (σ : State) (ℓ : Thread) (o : Identity) (path : List Proj) (moved : List Token) (h : σ.A) :
    (relocateOut σ ℓ o path moved).A :=
  (relocateOut_core σ ℓ o path moved).A (A_writeWith _ _ _ (A_writeWith _ _ _ (A_establish σ ℓ true h)))
theorem Fresh_relocateOut (σ : State) (ℓ : Thread) (o : Identity) (path : List Proj) (moved : List Token)
    (hmoved : ∀ t ∈ moved, t ∈ σ.contents o) (hf : σ.Fresh) : (relocateOut σ ℓ o path moved).Fresh :=
  (relocateOut_core σ ℓ o path moved).Fresh (Fresh_writeWith _ _ _
    (fun t ht => show t < σ.nextId + 1 from Nat.lt_succ_of_lt (hf.2.2 o t (hmoved t ht)))
    (Fresh_writeWith _ _ _ (fun t ht => show t < σ.nextId + 1 from Nat.lt_succ_of_lt (hf.2.2 o t (List.mem_filter.1 ht).1))
      (Fresh_establish σ ℓ true hf)))
theorem R_relocateOut (σ : State) (ℓ : Thread) (o : Identity) (path : List Proj) (moved : List Token)
    (hrun : σ.running ℓ) (hs : σ.scopes ℓ ≠ []) (hf : σ.Fresh) (h : σ.R) : (relocateOut σ ℓ o path moved).R :=
  R_establish σ ℓ true hrun hs hf h
theorem C1_relocateOut (σ : State) (ℓ : Thread) (o : Identity) (path : List Proj) (moved : List Token) (h1 : σ.C1) (h2 : σ.C2) :
    (relocateOut σ ℓ o path moved).C1 :=
  (relocateOut_core σ ℓ o path moved).C1 (C1_writeWith _ _ _ (C1_writeWith _ _ _ (C1_establish σ ℓ true h1) (C2_establish σ ℓ true h2))
    (C2_writeWith _ _ _ (C1_establish σ ℓ true h1) (C2_establish σ ℓ true h2)))
theorem C2_relocateOut (σ : State) (ℓ : Thread) (o : Identity) (path : List Proj) (moved : List Token) (h1 : σ.C1) (h2 : σ.C2) :
    (relocateOut σ ℓ o path moved).C2 :=
  (relocateOut_core σ ℓ o path moved).C2 (C2_writeWith _ _ _ (C1_writeWith _ _ _ (C1_establish σ ℓ true h1) (C2_establish σ ℓ true h2))
    (C2_writeWith _ _ _ (C1_establish σ ℓ true h1) (C2_establish σ ℓ true h2)))
theorem P_relocateOut (σ : State) (ℓ : Thread) (o : Identity) (path : List Proj) (moved : List Token)
    (ho : σ.alive o) (hmoved : ∀ t ∈ moved, t ∈ σ.contents o) (hf : σ.Fresh) (h2 : σ.T2) (h8 : σ.P) :
    (relocateOut σ ℓ o path moved).P := by
  have e2 := T2_establish σ ℓ true h2 hf
  have e8 : (establish σ ℓ true).P := h8
  have w2 := T2_writeWith _ o ((σ.contents o).filter (· ∉ moved)) (relocateOut_alive1 σ ℓ o ho hf) e2
  have w8 : (writeWith (establish σ ℓ true) o ((σ.contents o).filter (· ∉ moved))).P := by
    refine P_writeWith _ o _ (relocateOut_alive1 σ ℓ o ho hf) e2 ?_ e8
    intro t ht q hq
    exact contents_not_pending σ o ho h2 h8 t (List.mem_filter.1 ht).1 q hq
  refine (relocateOut_core σ ℓ o path moved).P (P_writeWith _ σ.nextId moved (relocateOut_alive2 σ ℓ o moved hf) w2 ?_ w8)
  intro t ht q hq
  simp [writeWith, establish] at hq
  obtain ⟨q0, hq0, rfl⟩ := hq
  exact contents_not_pending σ o ho h2 h8 t (hmoved t ht) q0 hq0

theorem RS_relocateOut (σ : State) (ℓ : Thread) (o : Identity) (path : List Proj) (moved : List Token)
    (ho : σ.alive o) (hf : σ.Fresh) (hsub : σ.subs o path = some ℓ) (h : σ.RS) : (relocateOut σ ℓ o path moved).RS := by
  intro i q ℓ' hs
  have hs' : relocOutSubs σ o path i q = some ℓ' := hs
  have hob : (relocateOut σ ℓ o path moved).obligation i = (if i = σ.nextId then true else σ.obligation i) := rfl
  have hau : (relocateOut σ ℓ o path moved).authority i = (if i = σ.nextId then some ℓ else σ.authority i) := rfl
  rw [hob, hau]
  unfold relocOutSubs at hs'
  obtain ⟨_, _, hauo⟩ := h o path ℓ hsub
  by_cases hin : i = σ.nextId
  · rw [if_pos hin] at hs'
    rw [if_pos hin, if_pos hin]
    by_cases hq : q = []
    · rw [if_pos hq] at hs'
      cases hs'
    · rw [if_neg hq] at hs'
      obtain ⟨_, _, hauo'⟩ := h o _ ℓ' hs'
      rw [hauo] at hauo'
      cases hauo'
      exact ⟨hq, rfl, rfl⟩
  · rw [if_neg hin] at hs'
    rw [if_neg hin, if_neg hin]
    by_cases hio : i = o
    · rw [if_pos hio] at hs'
      by_cases hpre : path.isPrefixOf q = true
      · rw [if_pos hpre] at hs'
        cases hs'
      · rw [if_neg hpre] at hs'
        rw [hio]
        exact h o q ℓ' hs'
    · rw [if_neg hio] at hs'
      exact h i q ℓ' hs'


/-! ### raw storage: reclaim, re-attachment, release, and the moves between fresh and reclaimed storage -/

theorem Owner_flag (σ : State) (o : Identity) (r : ObjectRecord) (tr : ThreadRec) (h : Owner σ o r tr) :
    Owner σ o { r with reclaimed := true } tr := by
  rcases h with ⟨g, hg, hm⟩ | ⟨hf, hm⟩ | h3 | _
  · exact Or.inl ⟨g, hg, hm⟩
  · exact Or.inr (Or.inl ⟨hf, hm⟩)
  · exact Or.inr (Or.inr (Or.inl h3))
  · exact Or.inr (Or.inr (Or.inr rfl))

theorem core_setReclaimed (σ : State) (o : Identity) : Core σ (setReclaimed σ o) where
  paths := rfl
  contents := rfl
  sync := rfl
  nextId := rfl
  alive := fun i => by
    constructor
    · intro ⟨q, hq, ha⟩
      by_cases hio : i = o
      · subst hio
        simp [setReclaimed] at hq
        obtain ⟨r, hr, rfl⟩ := hq
        exact ⟨r, hr, ha⟩
      · exact ⟨q, by simpa [setReclaimed, hio] using hq, ha⟩
    · intro ⟨q, hq, ha⟩
      by_cases hio : i = o
      · subst hio
        exact ⟨{ q with reclaimed := true }, by simp [setReclaimed, hq], ha⟩
      · exact ⟨q, by simp [setReclaimed, hio, hq], ha⟩
  dom := fun i r h => by
    by_cases hio : i = o
    · subst hio
      simp [setReclaimed] at h
      obtain ⟨r', hr', _⟩ := h
      exact ⟨r', hr'⟩
    · exact ⟨r, by simpa [setReclaimed, hio] using h⟩

theorem R_setReclaimed (σ : State) (o : Identity) (h : σ.R) : (setReclaimed σ o).R := by
  intro i hi
  have hi' : σ.obligation i = true := hi
  obtain ⟨rec, ℓ', tr, hr, ha, hau, htr, how⟩ := h i hi'
  by_cases hio : i = o
  · subst hio
    exact ⟨{ rec with reclaimed := true }, ℓ', tr, by simp [setReclaimed, hr], ha, hau, htr, Owner_flag σ i rec tr how⟩
  · exact ⟨rec, ℓ', tr, by simp [setReclaimed, hio, hr], ha, hau, htr, how⟩

theorem RS_setReclaimed (σ : State) (o : Identity) (h : σ.RS) : (setReclaimed σ o).RS := h

theorem reclaim_alive (σ : State) (ℓ : Thread) (r : Bool) (hf : σ.Fresh) : (setReclaimed (establish σ ℓ r) σ.nextId).alive σ.nextId :=
  ((core_setReclaimed _ _).alive _).2 ((alive_establish_iff σ ℓ r hf _).2 (Or.inr rfl))

theorem T1_reclaim (σ : State) (ℓ : Thread) (r : Bool) (hf : σ.Fresh) (h : σ.T1) : (reclaim σ ℓ r).T1 :=
  T1_tempRoot _ ℓ σ.nextId (reclaim_alive σ ℓ r hf) ((core_setReclaimed _ _).T1 (T1_establish σ ℓ r h hf))
theorem T2_reclaim (σ : State) (ℓ : Thread) (r : Bool) (hf : σ.Fresh) (h : σ.T2) : (reclaim σ ℓ r).T2 :=
  T2_tempRoot _ ℓ σ.nextId ((core_setReclaimed _ _).T2 (T2_establish σ ℓ r h hf)) ((core_setReclaimed _ _).Fresh (Fresh_establish σ ℓ r hf))
theorem A_reclaim (σ : State) (ℓ : Thread) (r : Bool) (hf : σ.Fresh) (h : σ.A) : (reclaim σ ℓ r).A :=
  A_tempRoot _ ℓ σ.nextId ((core_setReclaimed _ _).A (A_establish σ ℓ r h)) ((core_setReclaimed _ _).Fresh (Fresh_establish σ ℓ r hf))
theorem Fresh_reclaim (σ : State) (ℓ : Thread) (r : Bool) (hf : σ.Fresh) : (reclaim σ ℓ r).Fresh :=
  Fresh_tempRoot _ ℓ σ.nextId ((core_setReclaimed _ _).Fresh (Fresh_establish σ ℓ r hf))
theorem R_reclaim (σ : State) (ℓ : Thread) (r : Bool) (hrun : σ.running ℓ) (hs : σ.scopes ℓ ≠ []) (hf : σ.Fresh) (h : σ.R) :
    (reclaim σ ℓ r).R :=
  R_tempRoot _ ℓ σ.nextId (R_setReclaimed _ _ (R_establish σ ℓ r hrun hs hf h))
theorem C1_reclaim (σ : State) (ℓ : Thread) (r : Bool) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (reclaim σ ℓ r).C1 :=
  C1_tempRoot _ ℓ σ.nextId ((core_setReclaimed _ _).Fresh (Fresh_establish σ ℓ r hf)) ((core_setReclaimed _ _).C1 (C1_establish σ ℓ r h1))
    ((core_setReclaimed _ _).C2 (C2_establish σ ℓ r h2))
theorem C2_reclaim (σ : State) (ℓ : Thread) (r : Bool) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (reclaim σ ℓ r).C2 :=
  C2_tempRoot _ ℓ σ.nextId ((core_setReclaimed _ _).Fresh (Fresh_establish σ ℓ r hf)) ((core_setReclaimed _ _).C1 (C1_establish σ ℓ r h1))
    ((core_setReclaimed _ _).C2 (C2_establish σ ℓ r h2))
theorem P_reclaim (σ : State) (ℓ : Thread) (r : Bool) (h : σ.P) : (reclaim σ ℓ r).P :=
  P_tempRoot _ ℓ σ.nextId ((core_setReclaimed _ _).P (P_establish σ ℓ r h))
theorem RS_reclaim (σ : State) (ℓ : Thread) (r : Bool) (h : σ.RS) : (reclaim σ ℓ r).RS :=
  RS_tempRoot _ ℓ σ.nextId (RS_setReclaimed _ _ (RS_establish σ ℓ r h))

theorem R_rekey_reclaimed (σ : State) (ℓ : Thread) (o : Identity) (r : ObjectRecord) (hrun : σ.running ℓ)
    (hr : σ.objects o = some r) (hrecl : r.reclaimed = true) (h : σ.R) : (rekey σ (fun i => decide (i = o)) ℓ).R := by
  intro i hi
  have hi' : σ.obligation i = true := hi
  obtain ⟨rec, ℓ', tr', hr', ha, hau, htr', how⟩ := h i hi'
  by_cases hio : i = o
  · subst hio
    obtain ⟨tr, htr, _⟩ := hrun
    have hrr : rec = r := by
      rw [hr] at hr'
      cases hr'
      rfl
    refine ⟨rec, ℓ, tr, hr', ha, by simp [rekey], htr, Or.inr (Or.inr (Or.inr ?_))⟩
    rw [hrr]
    exact hrecl
  · exact ⟨rec, ℓ', tr', hr', ha, by simp [rekey, hio, hau], htr', Owner_mono σ _ i rec tr' (fun hb => hb) how⟩

theorem T1_reattach (σ : State) (ℓ : Thread) (o : Identity) (r : ObjectRecord) (hr : σ.objects o = some r) (hal : r.alive = true)
    (h : σ.T1) : (reattach σ ℓ o).T1 :=
  T1_tempRoot _ ℓ o (((core_rekey σ (fun i => decide (i = o)) ℓ).alive o).2 ⟨r, hr, hal⟩) ((core_rekey σ _ ℓ).T1 h)
theorem T2_reattach (σ : State) (ℓ : Thread) (o : Identity) (h : σ.T2) (hf : σ.Fresh) : (reattach σ ℓ o).T2 :=
  T2_tempRoot _ ℓ o ((core_rekey σ (fun i => decide (i = o)) ℓ).T2 h) ((core_rekey σ _ ℓ).Fresh hf)
theorem A_reattach (σ : State) (ℓ : Thread) (o : Identity) (h : σ.A) (hf : σ.Fresh) : (reattach σ ℓ o).A :=
  A_tempRoot _ ℓ o ((core_rekey σ (fun i => decide (i = o)) ℓ).A h) ((core_rekey σ _ ℓ).Fresh hf)
theorem Fresh_reattach (σ : State) (ℓ : Thread) (o : Identity) (hf : σ.Fresh) : (reattach σ ℓ o).Fresh :=
  Fresh_tempRoot _ ℓ o ((core_rekey σ (fun i => decide (i = o)) ℓ).Fresh hf)
theorem R_reattach (σ : State) (ℓ : Thread) (o : Identity) (r : ObjectRecord) (hrun : σ.running ℓ) (hr : σ.objects o = some r)
    (hrecl : r.reclaimed = true) (h : σ.R) : (reattach σ ℓ o).R :=
  R_tempRoot _ ℓ o (R_rekey_reclaimed σ ℓ o r hrun hr hrecl h)
theorem C1_reattach (σ : State) (ℓ : Thread) (o : Identity) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (reattach σ ℓ o).C1 :=
  C1_tempRoot _ ℓ o ((core_rekey σ (fun i => decide (i = o)) ℓ).Fresh hf) ((core_rekey σ _ ℓ).C1 h1) ((core_rekey σ _ ℓ).C2 h2)
theorem C2_reattach (σ : State) (ℓ : Thread) (o : Identity) (hf : σ.Fresh) (h1 : σ.C1) (h2 : σ.C2) : (reattach σ ℓ o).C2 :=
  C2_tempRoot _ ℓ o ((core_rekey σ (fun i => decide (i = o)) ℓ).Fresh hf) ((core_rekey σ _ ℓ).C1 h1) ((core_rekey σ _ ℓ).C2 h2)
theorem P_reattach (σ : State) (ℓ : Thread) (o : Identity) (h : σ.P) : (reattach σ ℓ o).P :=
  P_tempRoot _ ℓ o ((core_rekey σ (fun i => decide (i = o)) ℓ).P h)
theorem RS_reattach (σ : State) (ℓ : Thread) (o : Identity) (h : σ.RS) : (reattach σ ℓ o).RS :=
  RS_tempRoot _ ℓ o (RS_rekey σ (fun i => decide (i = o)) ℓ h)

theorem mw_core (σ : State) (ℓ : Thread) (oS : Identity) (recl : Bool) : Core (establish σ ℓ (mwRes σ oS)) (mwMid σ ℓ oS recl) where
  paths := rfl
  contents := rfl
  sync := rfl
  nextId := rfl
  alive := fun i => by
    constructor
    · intro ⟨q, hq, ha⟩
      by_cases hie : i = σ.nextId
      · subst hie
        exact ⟨{ alive := true, isResource := mwRes σ oS, guard := false, reclaimed := false, frame := none, scope := σ.scope ℓ }, by simp [establish], rfl⟩
      · exact ⟨q, by simpa [mwMid, establish, hie] using hq, ha⟩
    · intro ⟨q, hq, ha⟩
      by_cases hie : i = σ.nextId
      · subst hie
        exact ⟨{ alive := true, isResource := mwRes σ oS, guard := false, reclaimed := recl, frame := none, scope := σ.scope ℓ }, by simp [mwMid, establish], rfl⟩
      · exact ⟨q, by simpa [mwMid, establish, hie] using hq, ha⟩
  dom := fun i r h => by
    by_cases hie : i = σ.nextId
    · subst hie
      exact ⟨{ alive := true, isResource := mwRes σ oS, guard := false, reclaimed := false, frame := none, scope := σ.scope ℓ }, by simp [establish]⟩
    · exact ⟨r, by simpa [mwMid, establish, hie] using h⟩

theorem mw_alive (σ : State) (ℓ : Thread) (oS : Identity) (recl : Bool) : (mwMid σ ℓ oS recl).alive σ.nextId :=
  ⟨{ alive := true, isResource := mwRes σ oS, guard := false, reclaimed := recl, frame := none, scope := σ.scope ℓ }, by simp [mwMid, establish], rfl⟩

theorem R_mwMid (σ : State) (ℓ : Thread) (oS : Identity) (recl : Bool) (hrun : σ.running ℓ) (hs : recl = false → σ.scopes ℓ ≠ [])
    (hf : σ.Fresh) (h : σ.R) : (mwMid σ ℓ oS recl).R := by
  intro i hi
  obtain ⟨tr, htr, _⟩ := hrun
  by_cases hie : i = σ.nextId
  · subst hie
    have hob : σ.obligation oS = true := by simpa [mwMid, establish] using hi
    refine ⟨{ alive := true, isResource := mwRes σ oS, guard := false, reclaimed := recl, frame := none, scope := σ.scope ℓ }, ℓ, tr,
      by simp [mwMid, establish], rfl, by simp [mwMid, establish, hob], htr, ?_⟩
    cases recl with
    | true => exact Or.inr (Or.inr (Or.inr rfl))
    | false =>
      refine Or.inr (Or.inl ⟨rfl, ?_⟩)
      show (σ.scopes ℓ).headD 0 ∈ tr.scopes
      have hs' := hs rfl
      rw [scopes_of σ ℓ tr htr] at hs' ⊢
      exact head_mem _ _ hs'
  · have hi' : σ.obligation i = true := by simpa [mwMid, establish, hie] using hi
    obtain ⟨rec, ℓ', tr', hr, ha, hau, htr', how⟩ := h i hi'
    refine ⟨rec, ℓ', tr', by simp [mwMid, establish, hie, hr], ha, by simp [mwMid, establish, hie, hau], htr', ?_⟩
    refine Owner_mono σ _ i rec tr' ?_ how
    intro hb
    have hhn : tr'.handle ≠ σ.nextId := obligation_ne_next σ hf h tr'.handle hb
    simp [mwMid, establish, hhn, hb]

theorem RS_mwMid (σ : State) (ℓ : Thread) (oS : Identity) (recl : Bool) (h : σ.RS) : (mwMid σ ℓ oS recl).RS := by
  intro i q ℓ' hs
  by_cases hie : i = σ.nextId
  · subst hie
    simp [mwMid, establish] at hs
    obtain ⟨⟨ℓ0, hs0⟩, rfl⟩ := hs
    obtain ⟨hq, hob, _⟩ := h oS q ℓ0 hs0
    exact ⟨hq, by simp [mwMid, establish, hob], by simp [mwMid, establish, hob]⟩
  · simp [mwMid, establish, hie] at hs
    obtain ⟨hq, hob, hau⟩ := h i q ℓ' hs
    exact ⟨hq, by simp [mwMid, establish, hie, hob], by simp [mwMid, establish, hie, hau]⟩

theorem T1_moveWhole (σ : State) (ℓ : Thread) (oS : Identity) (recl : Bool) (hf : σ.Fresh) (h : σ.T1) : (moveWhole σ ℓ oS recl).T1 :=
  T1_endObjects _ _ (T1_writeWith _ _ _ ((mw_core σ ℓ oS recl).T1 (T1_establish σ ℓ _ h hf)))
theorem T2_moveWhole (σ : State) (ℓ : Thread) (oS : Identity) (recl : Bool) (hf : σ.Fresh) (h : σ.T2) : (moveWhole σ ℓ oS recl).T2 :=
  T2_endObjects _ _ (T2_writeWith _ _ _ (mw_alive σ ℓ oS recl) ((mw_core σ ℓ oS recl).T2 (T2_establish σ ℓ _ h hf)))
theorem A_moveWhole (σ : State) (ℓ : Thread) (oS : Identity) (recl : Bool) (h : σ.A) : (moveWhole σ ℓ oS recl).A :=
  A_endObjects _ _ (A_writeWith _ _ _ ((mw_core σ ℓ oS recl).A (A_establish σ ℓ _ h)))
theorem Fresh_moveWhole (σ : State) (ℓ : Thread) (oS : Identity) (recl : Bool) (hf : σ.Fresh) : (moveWhole σ ℓ oS recl).Fresh :=
  Fresh_endObjects _ _ (Fresh_writeWith _ _ _ (fun t ht => show t < σ.nextId + 1 from Nat.lt_succ_of_lt (hf.2.2 oS t ht))
    ((mw_core σ ℓ oS recl).Fresh (Fresh_establish σ ℓ _ hf)))
theorem R_moveWhole (σ : State) (ℓ : Thread) (oS : Identity) (recl : Bool) (hrun : σ.running ℓ) (hs : recl = false → σ.scopes ℓ ≠ [])
    (hf : σ.Fresh) (hnh : σ.noHandle (fun i => decide (i = oS))) (h : σ.R) : (moveWhole σ ℓ oS recl).R :=
  R_endObjects _ _ hnh (R_mwMid σ ℓ oS recl hrun hs hf h)
theorem C1_moveWhole (σ : State) (ℓ : Thread) (oS : Identity) (recl : Bool) (h1 : σ.C1) (h2 : σ.C2) : (moveWhole σ ℓ oS recl).C1 :=
  C1_endObjects _ _ (C1_writeWith _ _ _ ((mw_core σ ℓ oS recl).C1 (C1_establish σ ℓ _ h1)) ((mw_core σ ℓ oS recl).C2 (C2_establish σ ℓ _ h2)))
    (C2_writeWith _ _ _ ((mw_core σ ℓ oS recl).C1 (C1_establish σ ℓ _ h1)) ((mw_core σ ℓ oS recl).C2 (C2_establish σ ℓ _ h2)))
theorem C2_moveWhole (σ : State) (ℓ : Thread) (oS : Identity) (recl : Bool) (h1 : σ.C1) (h2 : σ.C2) : (moveWhole σ ℓ oS recl).C2 :=
  C2_endObjects _ _ (C1_writeWith _ _ _ ((mw_core σ ℓ oS recl).C1 (C1_establish σ ℓ _ h1)) ((mw_core σ ℓ oS recl).C2 (C2_establish σ ℓ _ h2)))
    (C2_writeWith _ _ _ ((mw_core σ ℓ oS recl).C1 (C1_establish σ ℓ _ h1)) ((mw_core σ ℓ oS recl).C2 (C2_establish σ ℓ _ h2)))
theorem P_moveWhole (σ : State) (ℓ : Thread) (oS : Identity) (recl : Bool) (hoS : σ.alive oS) (hf : σ.Fresh) (h2 : σ.T2) (h8 : σ.P) :
    (moveWhole σ ℓ oS recl).P :=
  P_endObjects _ _ (P_writeWith _ _ _ (mw_alive σ ℓ oS recl) ((mw_core σ ℓ oS recl).T2 (T2_establish σ ℓ _ h2 hf))
    (fun t ht q hq => contents_not_pending σ oS hoS h2 h8 t ht q hq) ((mw_core σ ℓ oS recl).P (P_establish σ ℓ _ h8)))
theorem RS_moveWhole (σ : State) (ℓ : Thread) (oS : Identity) (recl : Bool) (h : σ.RS) : (moveWhole σ ℓ oS recl).RS :=
  RS_endObjects _ _ (RS_writeWith _ _ _ (RS_mwMid σ ℓ oS recl h))

theorem core6 {σ σ' : State} (c : Core σ σ') (h1 : σ.T1) (h2 : σ.T2) (h3 : σ.A) (h4 : σ.Fresh) (h6 : σ.C1) (h7 : σ.C2) :
    σ'.T1 ∧ σ'.T2 ∧ σ'.A ∧ σ'.Fresh ∧ σ'.C1 ∧ σ'.C2 :=
  ⟨c.T1 h1, c.T2 h2, c.A h3, c.Fresh h4, c.C1 h6, c.C2 h7⟩

theorem end6 (σ : State) (dead : Identity → Bool) (h1 : σ.T1) (h2 : σ.T2) (h3 : σ.A) (h4 : σ.Fresh) (h6 : σ.C1) (h7 : σ.C2) :
    (endObjects σ dead).T1 ∧ (endObjects σ dead).T2 ∧ (endObjects σ dead).A ∧ (endObjects σ dead).Fresh ∧
    (endObjects σ dead).C1 ∧ (endObjects σ dead).C2 :=
  ⟨T1_endObjects σ dead h1, T2_endObjects σ dead h2, A_endObjects σ dead h3, Fresh_endObjects σ dead h4,
   C1_endObjects σ dead h6 h7, C2_endObjects σ dead h6 h7⟩

theorem preservation (σ σ' : State) (hinv : σ.Invariants) (hs : Step σ σ') : σ'.Invariants := by
  obtain ⟨h1, h2, h3, h4, h5, h6, h7, h8, h9⟩ := hinv
  cases hs with
  | read _ _ _ _ _ _ _ => exact ⟨h1, h2, h3, h4, h5, h6, h7, h8, h9⟩
  | lend ℓ a0 p0 _ hp0 htv _ _ _ _ =>
    exact ⟨T1_lend σ ℓ a0 p0 hp0 htv h1, T2_lend σ ℓ a0 p0 h2 h4, A_lend σ ℓ a0 p0 hp0 h3 h4, Fresh_lend σ ℓ a0 p0 h4,
      R_lend σ ℓ a0 p0 h5, C1_lend σ ℓ a0 p0 h4 h6 h7, C2_lend σ ℓ a0 p0 h4 h6 h7, P_lend σ ℓ a0 p0 h8, RS_lend σ ℓ a0 p0 h9⟩
  | takePath ℓ a p _ hp _ hl _ =>
    exact ⟨T1_takePath σ ℓ a h1, T2_takePath σ ℓ a h2, A_takePath σ ℓ a h3, Fresh_takePath σ ℓ a h4, R_takePath σ ℓ a h5,
      C1_takePath σ ℓ a p hp hl h6, C2_takePath σ ℓ a p hp hl h7, P_takePath σ ℓ a h8, RS_takePath σ ℓ a h9⟩
  | write ℓ a p removed added _ hp htv _ _ _ hadded =>
    have ho : σ.alive p.of := by
      obtain ⟨q, hq, _, hal⟩ := htv
      rw [hp] at hq
      cases hq
      exact hal
    have hadded' : ∀ t ∈ added, σ.paths t ≠ none := by
      intro t ht
      obtain ⟨q, hq, _⟩ := hadded t ht
      rw [hq]
      exact Option.some_ne_none q
    exact ⟨T1_write σ p.of removed added h1, T2_write σ p.of removed added ho h2, A_write σ p.of removed added h3,
      Fresh_write σ p.of removed added hadded' h4, R_write σ p.of removed added h5,
      C1_write σ p.of removed added h6 h7, C2_write σ p.of removed added h6 h7,
      P_write σ p.of removed added ho h2 hadded h8, RS_write σ p.of removed added h9⟩
  | establish ℓ r hrun hsc =>
    exact ⟨T1_establish σ ℓ r h1 h4, T2_establish σ ℓ r h2 h4, A_establish σ ℓ r h3, Fresh_establish σ ℓ r h4,
      R_establish σ ℓ r hrun hsc h4 h5, C1_establish σ ℓ r h6, C2_establish σ ℓ r h7, P_establish σ ℓ r h8, RS_establish σ ℓ r h9⟩
  | bind ℓ o _ ho hfr hau _ _ =>
    exact ⟨T1_bind σ ℓ o ho h1, T2_bind σ ℓ o h2 h4, A_bind σ ℓ o h3 h4, Fresh_bind σ ℓ o h4, R_bind σ ℓ o hfr hau h5,
      C1_bind σ ℓ o h4 h6 h7, C2_bind σ ℓ o h4 h6 h7, P_bind σ ℓ o h8, RS_bind σ ℓ o h9⟩
  | borrow ℓ a0 p0 m pend _ hp0 htv _ _ _ =>
    exact ⟨T1_borrow σ ℓ a0 p0 m pend hp0 htv h1, T2_borrow σ ℓ a0 p0 m pend h2 h4, A_borrow σ ℓ a0 p0 m pend hp0 h3 h4,
      Fresh_borrow σ ℓ a0 p0 m pend h4, R_borrow σ ℓ a0 p0 m pend h5, C1_borrow σ ℓ a0 p0 m pend h4 h6 h7,
      C2_borrow σ ℓ a0 p0 m pend h4 h6 h7, P_borrow σ ℓ a0 p0 m pend h8, RS_borrow σ ℓ a0 p0 m pend h9⟩
  | project ℓ a0 p0 f _ hp0 htv =>
    exact ⟨T1_project σ ℓ a0 p0 f hp0 htv h1, T2_project σ ℓ a0 p0 f h2 h4, A_project σ ℓ a0 p0 f hp0 h3 h4,
      Fresh_project σ ℓ a0 p0 f h4, R_project σ ℓ a0 p0 f h5, C1_project σ ℓ a0 p0 f h4 h6 h7, C2_project σ ℓ a0 p0 f h4 h6 h7,
      P_project σ ℓ a0 p0 f h8, RS_project σ ℓ a0 p0 f h9⟩
  | tempRoot ℓ o _ ho _ _ =>
    exact ⟨T1_tempRoot σ ℓ o ho h1, T2_tempRoot σ ℓ o h2 h4, A_tempRoot σ ℓ o h3 h4, Fresh_tempRoot σ ℓ o h4, R_tempRoot σ ℓ o h5,
      C1_tempRoot σ ℓ o h4 h6 h7, C2_tempRoot σ ℓ o h4 h6 h7, P_tempRoot σ ℓ o h8, RS_tempRoot σ ℓ o h9⟩
  | bindParam ℓ a p _ _ hp _ _ =>
    exact ⟨T1_bindParam σ ℓ a h4 h1, T2_bindParam σ ℓ a p hp h4 h2, A_bindParam σ ℓ a h4 h3, Fresh_bindParam σ ℓ a p hp h4,
      R_bindParam σ ℓ a h4 h5, C1_bindParam σ ℓ a h4 h6, C2_bindParam σ ℓ a h7, P_bindParam σ ℓ a h8, RS_bindParam σ ℓ a h9⟩
  | settle ℓ a p _ _ _ =>
    exact ⟨T1_settle σ a h1, T2_settle σ a h2, A_settle σ a h3, Fresh_settle σ a h4, R_settle σ a h5, C1_settle σ a h6,
      C2_settle σ a h7, P_settle σ a h8, RS_settle σ a h9⟩
  | relocateIn ℓ oS o path _ hoS ho _ hpath hob hau hauS _ _ hnh _ _ =>
    exact ⟨T1_relocateIn σ oS o path h1, T2_relocateIn σ oS o path ho h2, A_relocateIn σ oS o path h3, Fresh_relocateIn σ oS o path h4,
      R_relocateIn σ oS o path hnh h5, C1_relocateIn σ oS o path h6 h7, C2_relocateIn σ oS o path h6 h7,
      P_relocateIn σ oS o path hoS ho h2 h8, RS_relocateIn σ ℓ oS o path hpath hob hau hauS h9⟩
  | relocateOut ℓ o path moved hrun hsc ho hsub hmoved =>
    exact ⟨T1_relocateOut σ ℓ o path moved h4 h1, T2_relocateOut σ ℓ o path moved ho h4 h2, A_relocateOut σ ℓ o path moved h3,
      Fresh_relocateOut σ ℓ o path moved hmoved h4, R_relocateOut σ ℓ o path moved hrun hsc h4 h5,
      C1_relocateOut σ ℓ o path moved h6 h7, C2_relocateOut σ ℓ o path moved h6 h7,
      P_relocateOut σ ℓ o path moved ho hmoved h4 h2 h8, RS_relocateOut σ ℓ o path moved ho h4 hsub h9⟩
  | destroyComposite ℓ o _ _ _ =>
    obtain ⟨c1, c2, c3, c4, c6, c7⟩ := core6 (core_destroyComposite σ o) h1 h2 h3 h4 h6 h7
    exact ⟨c1, c2, c3, c4, R_destroyComposite σ o h5, c6, c7, (core_destroyComposite σ o).P h8, RS_destroyComposite σ o h9⟩
  | destroy ℓ o _ _ _ _ hnh _ _ =>
    obtain ⟨c1, c2, c3, c4, c6, c7⟩ := end6 σ _ h1 h2 h3 h4 h6 h7
    exact ⟨c1, c2, c3, c4, R_endObjects σ _ hnh h5, c6, c7, P_endObjects σ _ h8, RS_endObjects σ _ h9⟩
  | release ℓ dead _ _ _ hnh _ =>
    obtain ⟨c1, c2, c3, c4, c6, c7⟩ := end6 σ dead h1 h2 h3 h4 h6 h7
    exact ⟨c1, c2, c3, c4, R_endObjects σ dead hnh h5, c6, c7, P_endObjects σ dead h8, RS_endObjects σ dead h9⟩
  | reclaim ℓ r hrun hsc =>
    exact ⟨T1_reclaim σ ℓ r h4 h1, T2_reclaim σ ℓ r h4 h2, A_reclaim σ ℓ r h4 h3, Fresh_reclaim σ ℓ r h4,
      R_reclaim σ ℓ r hrun hsc h4 h5, C1_reclaim σ ℓ r h4 h6 h7, C2_reclaim σ ℓ r h4 h6 h7, P_reclaim σ ℓ r h8, RS_reclaim σ ℓ r h9⟩
  | reattach ℓ o r hrun hr hal hrecl _ =>
    exact ⟨T1_reattach σ ℓ o r hr hal h1, T2_reattach σ ℓ o h2 h4, A_reattach σ ℓ o h3 h4, Fresh_reattach σ ℓ o h4,
      R_reattach σ ℓ o r hrun hr hrecl h5, C1_reattach σ ℓ o h4 h6 h7, C2_reattach σ ℓ o h4 h6 h7, P_reattach σ ℓ o h8,
      RS_reattach σ ℓ o h9⟩
  | rawptrMoveIn ℓ oS hrun hoS _ _ hnh _ _ =>
    exact ⟨T1_moveWhole σ ℓ oS true h4 h1, T2_moveWhole σ ℓ oS true h4 h2, A_moveWhole σ ℓ oS true h3, Fresh_moveWhole σ ℓ oS true h4,
      R_moveWhole σ ℓ oS true hrun (fun hc => by cases hc) h4 hnh h5, C1_moveWhole σ ℓ oS true h6 h7, C2_moveWhole σ ℓ oS true h6 h7,
      P_moveWhole σ ℓ oS true hoS h4 h2 h8, RS_moveWhole σ ℓ oS true h9⟩
  | rawptrMoveOut ℓ oS r hrun hsc hr hal _ _ hnh _ =>
    exact ⟨T1_moveWhole σ ℓ oS false h4 h1, T2_moveWhole σ ℓ oS false h4 h2, A_moveWhole σ ℓ oS false h3, Fresh_moveWhole σ ℓ oS false h4,
      R_moveWhole σ ℓ oS false hrun (fun _ => hsc) h4 hnh h5, C1_moveWhole σ ℓ oS false h6 h7, C2_moveWhole σ ℓ oS false h6 h7,
      P_moveWhole σ ℓ oS false ⟨r, hr, hal⟩ h4 h2 h8, RS_moveWhole σ ℓ oS false h9⟩
  | endPlain ℓ o _ _ _ hnh _ _ _ =>
    obtain ⟨c1, c2, c3, c4, c6, c7⟩ := end6 σ _ h1 h2 h3 h4 h6 h7
    exact ⟨c1, c2, c3, c4, R_endObjects σ _ hnh h5, c6, c7, P_endObjects σ _ h8, RS_endObjects σ _ h9⟩
  | transfer ℓ ℓ2 a1 p1 _ hp1 htv1 hau1 _ hrun2 hfr _ =>
    have ho : σ.alive p1.of := by
      obtain ⟨q, hq, _, hal⟩ := htv1
      rw [hp1] at hq
      cases hq
      exact hal
    have hfm : σ.frame ℓ2 ∈ (tempRoot σ ℓ2 p1.of).frames ℓ2 := head_mem _ 0 hfr
    exact ⟨T1_transfer _ a1 σ.nextId ℓ2 (σ.frame ℓ2) p1.of (T1_tempRoot σ ℓ2 p1.of ho h1),
      T2_transfer _ a1 σ.nextId ℓ2 (σ.frame ℓ2) p1.of (T2_tempRoot σ ℓ2 p1.of h2 h4),
      A_transfer _ a1 σ.nextId ℓ2 (σ.frame ℓ2) p1.of (A_tempRoot σ ℓ2 p1.of h3 h4),
      Fresh_transfer _ a1 σ.nextId ℓ2 (σ.frame ℓ2) p1.of (Fresh_tempRoot σ ℓ2 p1.of h4),
      R_transfer (tempRoot σ ℓ2 p1.of) ℓ ℓ2 a1 σ.nextId p1 (σ.frame ℓ2) hau1 hrun2 hfm (R_tempRoot σ ℓ2 p1.of h5),
      C1_transfer _ a1 σ.nextId ℓ2 (σ.frame ℓ2) p1.of (C1_tempRoot σ ℓ2 p1.of h4 h6 h7),
      C2_transfer _ a1 σ.nextId ℓ2 (σ.frame ℓ2) p1.of (C2_tempRoot σ ℓ2 p1.of h4 h6 h7),
      P_transfer _ a1 σ.nextId ℓ2 (σ.frame ℓ2) p1.of (P_tempRoot σ ℓ2 p1.of h8),
      RS_transfer _ a1 σ.nextId ℓ2 (σ.frame ℓ2) p1.of (RS_tempRoot σ ℓ2 p1.of h9)⟩
  | blockEnter ℓ f _ =>
    obtain ⟨c1, c2, c3, c4, c6, c7⟩ := core6 (core_mapThread σ ℓ _) h1 h2 h3 h4 h6 h7
    exact ⟨c1, c2, c3, c4, R_blockEnter σ ℓ f h5, c6, c7, (core_mapThread σ ℓ _).P h8, RS_blockEnter σ ℓ f h9⟩
  | blockExit ℓ _ hfr hnh _ =>
    obtain ⟨e1, e2, e3, e4, e6, e7⟩ := end6 σ (blockDead σ ℓ) h1 h2 h3 h4 h6 h7
    obtain ⟨c1, c2, c3, c4, c6, c7⟩ := core6 (core_mapThread _ ℓ _) e1 e2 e3 e4 e6 e7
    exact ⟨c1, c2, c3, c4, R_blockExit σ ℓ hfr hnh h5, c6, c7, (core_mapThread _ ℓ _).P (P_endObjects σ _ h8), RS_blockExit σ ℓ h9⟩
  | stmtEnter ℓ t _ =>
    obtain ⟨c1, c2, c3, c4, c6, c7⟩ := core6 (core_mapThread σ ℓ _) h1 h2 h3 h4 h6 h7
    exact ⟨c1, c2, c3, c4, R_stmtEnter σ ℓ t h5, c6, c7, (core_mapThread σ ℓ _).P h8, RS_stmtEnter σ ℓ t h9⟩
  | stmtExit ℓ _ hsc hnh _ =>
    obtain ⟨e1, e2, e3, e4, e6, e7⟩ := end6 σ (stmtDead σ ℓ) h1 h2 h3 h4 h6 h7
    have i1 := T1_invalidateUnheld _ (σ.scope ℓ) e1
    have i2 := T2_invalidateUnheld _ (σ.scope ℓ) e2
    have i3 := A_invalidateUnheld _ (σ.scope ℓ) e3
    have i4 := Fresh_invalidateUnheld _ (σ.scope ℓ) e4
    have i6 := C1_invalidateUnheld _ (σ.scope ℓ) e6 e7
    have i7 := C2_invalidateUnheld _ (σ.scope ℓ) e6 e7
    have i8 := P_invalidateUnheld (endObjects σ (stmtDead σ ℓ)) (σ.scope ℓ) (P_endObjects σ (stmtDead σ ℓ) h8)
    obtain ⟨c1, c2, c3, c4, c6, c7⟩ := core6 (core_mapThread _ ℓ _) i1 i2 i3 i4 i6 i7
    exact ⟨c1, c2, c3, c4, R_stmtExit σ ℓ hsc hnh h5, c6, c7, (core_mapThread _ ℓ _).P i8, RS_stmtExit σ ℓ h9⟩
  | spawn ℓ ℓ' f' hrun hsc hnone =>
    have e1 := T1_establish σ ℓ true h1 h4
    have e2 := T2_establish σ ℓ true h2 h4
    have e3 := A_establish σ ℓ true h3
    have e4 := Fresh_establish σ ℓ true h4
    have e6 := C1_establish σ ℓ true h6
    have e7 := C2_establish σ ℓ true h7
    obtain ⟨c1, c2, c3, c4, c6, c7⟩ := core6 (core_setThread _ ℓ' _) e1 e2 e3 e4 e6 e7
    exact ⟨c1, c2, c3, c4, R_spawn σ ℓ ℓ' f' hrun hsc hnone h4 h5, c6, c7, (core_setThread _ ℓ' _).P (P_establish σ ℓ true h8),
      RS_spawn σ ℓ ℓ' f' h9⟩
  | threadDone ℓ' tr f' res htr hdone hfr hsc hob hres hnh _ =>
    obtain ⟨e1, e2, e3, e4, e6, e7⟩ := end6 σ _ h1 h2 h3 h4 h6 h7
    obtain ⟨c1, c2, c3, c4, c6, c7⟩ := core6 ((core_restamp _ _ _ _).trans (core_mapThread _ ℓ' _)) e1 e2 e3 e4 e6 e7
    exact ⟨c1, c2, c3, c4, R_threadDone σ ℓ' tr f' res htr hdone hfr hsc hob hres hnh h5, c6, c7,
      ((core_restamp _ _ _ _).trans (core_mapThread _ ℓ' _)).P (P_endObjects σ _ h8), RS_threadDone σ ℓ' res h9⟩
  | join ℓ ℓ' tr res hrun hne hsc htr hdone hfr hsc' hauh hdist hnorecl =>
    obtain ⟨e1, e2, e3, e4, e6, e7⟩ := end6 σ _ h1 h2 h3 h4 h6 h7
    obtain ⟨c1, c2, c3, c4, c6, c7⟩ := core6 ((core_restamp _ _ _ _).trans ((core_rekey _ _ _).trans (core_setThread _ ℓ' _))) e1 e2 e3 e4 e6 e7
    exact ⟨c1, c2, c3, c4, R_join σ ℓ ℓ' tr res hrun hne hsc htr hdone hfr hsc' hdist hnorecl h5, c6, c7,
      ((core_restamp _ _ _ _).trans ((core_rekey _ _ _).trans (core_setThread _ ℓ' _))).P (P_endObjects σ _ h8),
      RS_join σ ℓ ℓ' tr.handle res h9⟩
  | handleDestroy ℓ ℓ' tr res _ htr hdone hfr hsc _ hdist _ hnorecl =>
    obtain ⟨e1, e2, e3, e4, e6, e7⟩ := end6 σ _ h1 h2 h3 h4 h6 h7
    obtain ⟨c1, c2, c3, c4, c6, c7⟩ := core6 (core_setThread _ ℓ' _) e1 e2 e3 e4 e6 e7
    exact ⟨c1, c2, c3, c4, R_handleDestroy σ ℓ' tr res htr hdone hfr hsc hdist hnorecl h5, c6, c7,
      (core_setThread _ ℓ' _).P (P_endObjects σ _ h8), RS_handleDestroy σ ℓ' tr.handle res h9⟩
  | faultContain ℓ' tr htr hdone _ _ hnh =>
    have m1 := T1_fcMid σ ℓ' tr h1
    have m2 := T2_fcMid σ ℓ' tr h2
    have m3 := A_fcMid σ ℓ' tr h3
    have m4 := Fresh_fcMid σ ℓ' tr h4
    have m6 := C1_fcMid σ ℓ' tr h6
    have m7 := C2_fcMid σ ℓ' tr h7
    obtain ⟨c1, c2, c3, c4, c6, c7⟩ := core6 (core_mapThread _ ℓ' _) m1 m2 m3 m4 m6 m7
    exact ⟨c1, c2, c3, c4, R_faultContain σ ℓ' tr htr hdone hnh h5, c6, c7, (core_mapThread _ ℓ' _).P (P_fcMid σ ℓ' tr h8),
      RS_faultContain σ ℓ' tr h9⟩
  | lock ℓ am pm hrun hsc hpm htv hunl =>
    exact ⟨T1_lock σ ℓ am pm hpm htv h4 h1, T2_lock σ ℓ am pm h4 h2, A_lock σ ℓ am pm hpm h4 h3, Fresh_lock σ ℓ am pm h4,
      R_lock σ ℓ am pm hrun hsc h4 h5, C1_lock σ ℓ am pm h4 h6, C2_lock σ ℓ am pm hunl h7, P_lock σ ℓ am pm h8, RS_lock σ ℓ am pm h9⟩
  | guardDrop ℓ g r ag p _ _ _ _ _ _ _ hp hl hv hnh =>
    exact ⟨T1_guardDrop σ g ag p.of h1, T2_guardDrop σ g ag p.of h2, A_guardDrop σ g ag p.of h3, Fresh_guardDrop σ g ag p.of h4,
      R_guardDrop σ g ag p.of hnh h5, C1_guardDrop σ g ag p.of h6, C2_guardDrop σ g ag p hp hl hv h7, P_guardDrop σ g ag p.of h8,
      RS_guardDrop σ g ag p.of h9⟩

theorem reachable_invariants (σ : State) (h : Reachable σ) : σ.Invariants := by
  induction h with
  | init _ hinv => exact hinv
  | step _ _ _ hs ih => exact preservation _ _ ih hs

end Cobalt
