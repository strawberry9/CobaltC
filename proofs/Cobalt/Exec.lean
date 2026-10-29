import Cobalt.Elision
/-
  The fragment's machine, executable (D-0270): an action names a step and its data; `check` decides the
  step's premises; `apply` performs it; `exec_sound` says an admitted action is a `Step`. The model
  checker (`impl/model`, D-0219) writes the steps it takes as a trace, and `Replay` runs them through
  this machine, so the proven relation and the running model are compared on the model's own programs.

  The premises are decided by searching the tokens, identities and threads below the bounds the
  invariants give (`Fresh`: every token and identity in use is below `nextId`; `ThreadsBelow`: every
  thread with a record is below the bound the replayer keeps), so `check` is complete for reachable
  states and the soundness theorem needs only those bounds.
-/
namespace Cobalt
open State

/-! ### decidable readings of the premises -/

def nonemptyB {α : Type} : List α → Bool
  | [] => false
  | _ :: _ => true

theorem nonempty_of {α : Type} {l : List α} (h : nonemptyB l = true) : l ≠ [] := by
  cases l with
  | nil => cases h
  | cons x xs => exact List.cons_ne_nil x xs

def runningB (σ : State) (ℓ : Thread) : Bool :=
  match σ.threads ℓ with
  | some tr => tr.done.isNone
  | none => false

theorem running_of (σ : State) (ℓ : Thread) (h : runningB σ ℓ = true) : σ.running ℓ := by
  unfold runningB at h
  cases htr : σ.threads ℓ with
  | none => rw [htr] at h; cases h
  | some tr =>
    simp only [htr] at h
    refine ⟨tr, htr, ?_⟩
    cases hd : tr.done with
    | none => rfl
    | some _ => simp [hd] at h

theorem running_to (σ : State) (ℓ : Thread) (h : σ.running ℓ) : runningB σ ℓ = true := by
  obtain ⟨tr, htr, hd⟩ := h
  simp [runningB, htr, hd]

theorem not_running_of (σ : State) (ℓ : Thread) (h : runningB σ ℓ = false) : ¬ σ.running ℓ := by
  intro hr
  rw [running_to σ ℓ hr] at h
  cases h

def aliveB (σ : State) (o : Identity) : Bool :=
  match σ.objects o with
  | some r => r.alive
  | none => false

theorem alive_of (σ : State) (o : Identity) (h : aliveB σ o = true) : σ.alive o := by
  unfold aliveB at h
  cases hr : σ.objects o with
  | none => rw [hr] at h; cases h
  | some r => simp only [hr] at h; exact ⟨r, hr, h⟩

def tvB (σ : State) (a : Token) : Bool :=
  match σ.paths a with
  | some p => p.valid && aliveB σ p.of
  | none => false

theorem tv_of (σ : State) (a : Token) (h : tvB σ a = true) : σ.temporallyValid a := by
  unfold tvB at h
  cases hp : σ.paths a with
  | none => rw [hp] at h; cases h
  | some p =>
    simp only [hp, Bool.and_eq_true] at h
    exact ⟨p, hp, h.1, alive_of σ p.of h.2⟩

def overlapB : List Proj → List Proj → Bool
  | [], _ => true
  | _, [] => true
  | x :: xs, y :: ys => decide (x = y) && overlapB xs ys

theorem overlap_to : ∀ (l1 l2 : List Proj), overlap l1 l2 → overlapB l1 l2 = true
  | [], _, _ => rfl
  | _ :: _, [], _ => rfl
  | x :: xs, y :: ys, h => by
    simp only [overlap] at h
    simp only [overlapB, Bool.and_eq_true, decide_eq_true_eq]
    exact ⟨h.1, overlap_to xs ys h.2⟩

def permittedB (m1 m2 : Mode) : Bool := decide (m1 = Mode.shared) && decide (m2 = Mode.shared)

theorem permittedB_false_of (m1 m2 : Mode) (h : ¬ permitted m1 m2) : permittedB m1 m2 = false := by
  cases m1 <;> cases m2 <;> simp_all [permittedB, permitted]

/-- One candidate witness of `clash σ a m`. -/
def clashWit (σ : State) (a : Token) (m : Mode) (a' : Token) : Bool :=
  match σ.paths a, σ.paths a' with
  | some p, some p' =>
    decide (a' ≠ a) && p'.valid && !p'.pending && !p.lock && !p'.lock && decide (p'.of = p.of) && overlapB p.proj p'.proj &&
      !decide (a' ∈ ancestors σ σ.nextId a) && !permittedB m p'.mode
  | _, _ => false

def clashB (σ : State) (a : Token) (m : Mode) : Bool := (List.range σ.nextId).any (clashWit σ a m)

theorem not_clash_of (σ : State) (hf : σ.Fresh) (a : Token) (m : Mode) (h : clashB σ a m = false) : ¬ σ.clash a m := by
  intro hc
  obtain ⟨a', p, p', hp, hp', hne, hv, hpend, hlock, hlock', hof, hov, hanc, hperm⟩ := hc
  have hlt : a' < σ.nextId := hf.1 a' p' hp'
  have hw : clashWit σ a m a' = true := by
    simp only [clashWit, hp, hp']
    simp [hne, hv, hpend, hlock, hlock', hof, overlap_to _ _ hov, hanc, permittedB_false_of m p'.mode hperm]
  have hany : (List.range σ.nextId).any (clashWit σ a m) = true :=
    List.any_eq_true.mpr ⟨a', List.mem_range.mpr hlt, hw⟩
  unfold clashB at h
  rw [h] at hany
  cases hany

def affineB (σ : State) (p : PathRecord) (ℓ : Thread) : Bool :=
  match p.thread with
  | some ℓ' => decide (ℓ' = ℓ) || !runningB σ ℓ'
  | none => true

theorem affine_of (σ : State) (p : PathRecord) (ℓ : Thread) (h : affineB σ p ℓ = true) : σ.affine p ℓ := by
  unfold affineB at h
  cases ht : p.thread with
  | none => exact Or.inr (Or.inl ht)
  | some ℓ' =>
    rw [ht] at h
    simp only [Bool.or_eq_true, decide_eq_true_eq, Bool.not_eq_true'] at h
    rcases h with h | h
    · exact Or.inl (by rw [ht, h])
    · exact Or.inr (Or.inr ⟨ℓ', ht, not_running_of σ ℓ' h⟩)

/-- Every thread with a record is below `n` (the replayer allocates thread numbers in order). -/
def ThreadsBelow (σ : State) (n : Nat) : Prop := ∀ ℓ tr, σ.threads ℓ = some tr → ℓ < n

def noHandleB (σ : State) (n : Nat) (dead : Identity → Bool) : Bool :=
  (List.range n).all (fun ℓ => match σ.threads ℓ with
    | some tr => !dead tr.handle
    | none => true)

theorem noHandle_of (σ : State) (n : Nat) (hth : ThreadsBelow σ n) (dead : Identity → Bool) (h : noHandleB σ n dead = true) :
    σ.noHandle dead := by
  intro ℓ tr htr
  have hm := List.all_eq_true.mp h ℓ (List.mem_range.mpr (hth ℓ tr htr))
  simp only [htr, Bool.not_eq_true'] at hm
  exact hm

def noGuardB (σ : State) (dead : Identity → Bool) : Bool :=
  (List.range σ.nextId).all (fun o => match σ.objects o with
    | some r => !(r.alive && r.guard && dead o)
    | none => true)

theorem noGuard_of (σ : State) (hf : σ.Fresh) (dead : Identity → Bool) (h : noGuardB σ dead = true) : σ.noGuard dead := by
  intro o r hr ha hg
  have hm := List.all_eq_true.mp h o (List.mem_range.mpr (hf.2.1 o r hr))
  simp only [hr, ha, hg, Bool.true_and, Bool.not_eq_true'] at hm
  exact hm

def containedB (σ : State) (ℓ : Thread) : Bool :=
  (List.range σ.nextId).all (fun a => match σ.paths a with
    | some p => !(decide (p.thread = some ℓ) && decide (p.mode = Mode.exclusive) && p.taken)
    | none => true)

theorem contained_of (σ : State) (hf : σ.Fresh) (ℓ : Thread) (h : containedB σ ℓ = true) : σ.contained ℓ := by
  intro a p hp ht hm
  have hm' := List.all_eq_true.mp h a (List.mem_range.mpr (hf.1 a p hp))
  simp only [hp, ht, hm, decide_true, Bool.true_and, Bool.not_eq_true'] at hm'
  exact hm'

/-- One candidate witness of `foreignHeld σ ℓ o`. -/
def fhWit (σ : State) (ℓ : Thread) (o : Identity) (a : Token) : Bool :=
  match σ.paths a with
  | some p => p.valid && decide (p.of = o) && decide (p.base ≠ none) && decide (p.mode = Mode.exclusive) &&
      decide (p.heldBy ≠ []) && decide (p.thread ≠ some ℓ) &&
      (decide (p.thread = none) || (match p.thread with | some ℓ' => runningB σ ℓ' | none => false))
  | none => false

def foreignHeldB (σ : State) (ℓ : Thread) (o : Identity) : Bool := (List.range σ.nextId).any (fhWit σ ℓ o)

theorem not_foreignHeld_of (σ : State) (hf : σ.Fresh) (ℓ : Thread) (o : Identity) (h : foreignHeldB σ ℓ o = false) :
    ¬ σ.foreignHeld ℓ o := by
  intro hfh
  obtain ⟨a, p, hp, hv, hof, hb, hm, hh, ht, hrest⟩ := hfh
  have hlt : a < σ.nextId := hf.1 a p hp
  have hw : fhWit σ ℓ o a = true := by
    simp only [fhWit, hp]
    rcases hrest with hn | ⟨ℓ', ht', hr⟩
    · simp [hv, hof, hb, hm, hh, ht, hn]
    · have hne : ℓ' ≠ ℓ := fun heq => ht (by rw [ht', heq])
      simp [hv, hof, hb, hm, hh, ht', hne, running_to σ ℓ' hr]
  have hany : (List.range σ.nextId).any (fhWit σ ℓ o) = true := List.any_eq_true.mpr ⟨a, List.mem_range.mpr hlt, hw⟩
  unfold foreignHeldB at h
  rw [h] at hany
  cases hany

def solitaryB (σ : State) (o : Identity) : Bool :=
  (List.range σ.nextId).all (fun a => match σ.paths a with
    | some p => !(p.valid && decide (p.of = o)) || decide (σ.holder o = some a)
    | none => true)

theorem solitary_of (σ : State) (hf : σ.Fresh) (o : Identity) (h : solitaryB σ o = true) : σ.solitary o := by
  intro a p hp hv hof
  have hm := List.all_eq_true.mp h a (List.mem_range.mpr (hf.1 a p hp))
  simp only [hp, hv, hof, decide_true, Bool.and_self, Bool.not_true, Bool.false_or, decide_eq_true_eq] at hm
  exact hm

def tempOfB (σ : State) (ℓ : Thread) (o : Identity) : Bool :=
  match σ.objects o with
  | some r => r.frame.isNone && decide (r.scope = σ.scope ℓ)
  | none => false

theorem tempOf_of (σ : State) (ℓ : Thread) (o : Identity) (h : tempOfB σ ℓ o = true) : σ.tempOf ℓ o := by
  unfold tempOfB at h
  cases hr : σ.objects o with
  | none => rw [hr] at h; cases h
  | some r =>
    simp only [hr, Bool.and_eq_true, decide_eq_true_eq, Option.isNone_iff_eq_none] at h
    exact ⟨r, hr, h.1, h.2⟩

def ownsB (σ : State) (ℓ : Thread) (o : Identity) : Bool :=
  match σ.objects o with
  | some r => match r.frame with
    | some f => decide (f ∈ σ.frames ℓ)
    | none => decide (r.scope ∈ σ.scopes ℓ)
  | none => false

theorem owns_of (σ : State) (ℓ : Thread) (o : Identity) (h : ownsB σ ℓ o = true) : σ.owns ℓ o := by
  unfold ownsB at h
  cases hr : σ.objects o with
  | none => rw [hr] at h; cases h
  | some r =>
    simp only [hr] at h
    refine ⟨r, hr, ?_⟩
    cases hfr : r.frame with
    | some f => simp only [hfr] at h; exact Or.inl ⟨f, rfl, of_decide_eq_true h⟩
    | none => simp only [hfr] at h; exact Or.inr ⟨rfl, of_decide_eq_true h⟩

/-! ### the actions -/

/-- A step with its data, as the trace names it (the fragment's steps the model checker takes). -/
inductive Act where
  | establish (ℓ : Thread) (r : Bool)
  | bind (ℓ : Thread) (o : Identity)
  | borrow (ℓ : Thread) (a0 : Token) (m : Mode) (pend : Bool)
  | project (ℓ : Thread) (a0 : Token) (f : Proj)
  | lend (ℓ : Thread) (a0 : Token)
  | takePath (ℓ : Thread) (a : Token)
  | read (ℓ : Thread) (a : Token)
  | write (ℓ : Thread) (a : Token) (removed added : List Token)
  | tempRoot (ℓ : Thread) (o : Identity)
  | bindParam (ℓ : Thread) (a : Token)
  | settle (ℓ : Thread) (a : Token)
  | reclaim (ℓ : Thread) (r : Bool)
  | release (ℓ : Thread) (dead : List Identity)
  | destroy (ℓ : Thread) (o : Identity)
  | endPlain (ℓ : Thread) (o : Identity)
  | transfer (ℓ ℓ2 : Thread) (a1 : Token)
  | blockEnter (ℓ : Thread) (f : Frame)
  | blockExit (ℓ : Thread)
  | stmtEnter (ℓ : Thread) (t : Scope)
  | stmtExit (ℓ : Thread)
  | spawn (ℓ ℓ' : Thread) (f' : Frame)
  | threadDone (ℓ' : Thread) (res : List Identity)
  | join (ℓ ℓ' : Thread)
  | handleDestroy (ℓ ℓ' : Thread)
  | faultContain (ℓ' : Thread)
  | lock (ℓ : Thread) (am : Token)
  | guardDrop (ℓ : Thread) (g : Identity)
  deriving Repr

def inList (l : List Nat) : Nat → Bool := fun i => decide (i ∈ l)

/-- The premises of the action's step, decided. -/
def check (σ : State) (n : Nat) : Act → Bool
  | .establish ℓ _ => runningB σ ℓ && nonemptyB (σ.scopes ℓ)
  | .bind ℓ o =>
    runningB σ ℓ && aliveB σ o && nonemptyB (σ.frames ℓ) &&
      (match σ.authority o with | some ℓ' => decide (ℓ' = ℓ) | none => true) &&
      nonemptyB (σ.scopes ℓ) && tempOfB σ ℓ o
  | .borrow ℓ a0 m pend =>
    match σ.paths a0 with
    | some p0 => runningB σ ℓ && tvB σ a0 && (!decide (m = Mode.exclusive) || decide (p0.mode = Mode.exclusive)) &&
        (!decide (p0.mode = Mode.exclusive) || decide (p0.thread = some ℓ)) && !clashB σ a0 m && (pend || !pend)
    | none => false
  | .project ℓ a0 _ => runningB σ ℓ && tvB σ a0 && (σ.paths a0).isSome
  | .lend ℓ a0 =>
    match σ.paths a0 with
    | some p0 => runningB σ ℓ && tvB σ a0 && decide (p0.mode = Mode.exclusive) && !p0.lock && affineB σ p0 ℓ && !clashB σ a0 Mode.exclusive
    | none => false
  | .takePath ℓ a =>
    match σ.paths a with
    | some p => runningB σ ℓ && decide (p.mode = Mode.exclusive) && !p.lock && affineB σ p ℓ
    | none => false
  | .read ℓ a =>
    match σ.paths a with
    | some p => runningB σ ℓ && tvB σ a && !clashB σ a Mode.shared && (!decide (p.mode = Mode.exclusive) || decide (p.thread = some ℓ))
    | none => false
  | .write ℓ a _ added =>
    match σ.paths a with
    | some p => runningB σ ℓ && tvB σ a && decide (p.mode = Mode.exclusive) && decide (p.thread = some ℓ) && !clashB σ a Mode.exclusive &&
        added.all (fun t => match σ.paths t with | some q => !q.pending | none => false)
    | none => false
  | .tempRoot ℓ o => runningB σ ℓ && aliveB σ o && nonemptyB (σ.scopes ℓ) && tempOfB σ ℓ o
  | .bindParam ℓ a =>
    match σ.paths a with
    | some p => runningB σ ℓ && nonemptyB (σ.frames ℓ) && tvB σ a && p.pending
    | none => false
  | .settle ℓ a =>
    match σ.paths a with
    | some p => runningB σ ℓ && p.pending
    | none => false
  | .reclaim ℓ _ => runningB σ ℓ && nonemptyB (σ.scopes ℓ)
  | .release ℓ ds =>
    runningB σ ℓ && ds.all (fun o => match σ.objects o with | some r => r.reclaimed | none => false) &&
      ds.all (fun o => !foreignHeldB σ ℓ o) && noHandleB σ n (inList ds) && noGuardB σ (inList ds)
  | .destroy ℓ o =>
    runningB σ ℓ && aliveB σ o && σ.obligation o && decide (σ.authority o = some ℓ) &&
      noHandleB σ n (fun i => decide (i = o)) && noGuardB σ (fun i => decide (i = o)) && solitaryB σ o
  | .endPlain ℓ o =>
    runningB σ ℓ && aliveB σ o && !σ.obligation o &&
      noHandleB σ n (fun i => decide (i = o)) && noGuardB σ (fun i => decide (i = o)) && solitaryB σ o && ownsB σ ℓ o
  | .transfer ℓ ℓ2 a1 =>
    match σ.paths a1 with
    | some p1 =>
      runningB σ ℓ && tvB σ a1 && decide (σ.authority p1.of = some ℓ) && σ.obligation p1.of && runningB σ ℓ2 &&
        nonemptyB (σ.frames ℓ2) &&
        (List.range σ.nextId).all (fun a => match σ.paths a with
          | some q => !(q.valid && decide (q.of = p1.of)) || decide (a = a1)
          | none => true)
    | none => false
  | .blockEnter ℓ _ => runningB σ ℓ
  | .blockExit ℓ => runningB σ ℓ && nonemptyB (σ.frames ℓ) && noHandleB σ n (blockDead σ ℓ) && noGuardB σ (blockDead σ ℓ)
  | .stmtEnter ℓ _ => runningB σ ℓ
  | .stmtExit ℓ => runningB σ ℓ && nonemptyB (σ.scopes ℓ) && noHandleB σ n (stmtDead σ ℓ) && noGuardB σ (stmtDead σ ℓ)
  | .spawn ℓ ℓ' _ => runningB σ ℓ && nonemptyB (σ.scopes ℓ) && (σ.threads ℓ').isNone
  | .threadDone ℓ' res =>
    match σ.threads ℓ' with
    | some tr =>
      tr.done.isNone && (match tr.frames with | [_] => true | _ => false) && decide (tr.scopes = []) && σ.obligation tr.handle &&
        res.all (fun o => !σ.obligation o || decide (σ.authority o = some ℓ')) &&
        noHandleB σ n (fun i => blockDead σ ℓ' i && !decide (i ∈ res)) && noGuardB σ (fun i => blockDead σ ℓ' i && !decide (i ∈ res))
    | none => false
  | .join ℓ ℓ' =>
    match σ.threads ℓ' with
    | some tr =>
      match tr.done with
      | some _ =>
        runningB σ ℓ && decide (ℓ ≠ ℓ') && nonemptyB (σ.scopes ℓ) && decide (tr.frames = []) && decide (tr.scopes = []) &&
          decide (σ.authority tr.handle = some ℓ) &&
          (List.range n).all (fun ℓ'' => decide (ℓ'' = ℓ') || match σ.threads ℓ'' with | some tr'' => decide (tr''.handle ≠ tr.handle) | none => true) &&
          (List.range σ.nextId).all (fun o => match σ.objects o with
            | some r => !(r.reclaimed && σ.obligation o && decide (σ.authority o = some ℓ'))
            | none => true)
      | none => false
    | none => false
  | .handleDestroy ℓ ℓ' =>
    match σ.threads ℓ' with
    | some tr =>
      match tr.done with
      | some res =>
        runningB σ ℓ && decide (tr.frames = []) && decide (tr.scopes = []) && decide (σ.authority tr.handle = some ℓ) &&
          (List.range n).all (fun ℓ'' => decide (ℓ'' = ℓ') || match σ.threads ℓ'' with
            | some tr'' => decide (tr''.handle ≠ tr.handle) && !decide (tr''.handle ∈ res)
            | none => true) &&
          noGuardB σ (fun i => decide (i = tr.handle) || decide (i ∈ res)) &&
          (List.range σ.nextId).all (fun o => match σ.objects o with
            | some r => !(r.reclaimed && σ.obligation o && decide (σ.authority o = some ℓ'))
            | none => true)
      | none => false
    | none => false
  | .faultContain ℓ' =>
    match σ.threads ℓ' with
    | some tr => tr.done.isNone && containedB σ ℓ' && σ.obligation tr.handle && noHandleB σ n (fcDead σ tr)
    | none => false
  | .lock ℓ am =>
    match σ.paths am with
    | some pm => runningB σ ℓ && nonemptyB (σ.scopes ℓ) && tvB σ am && decide (σ.sync pm.of = SyncState.unlocked)
    | none => false
  | .guardDrop ℓ g =>
    match σ.objects g with
    | some r =>
      match σ.contents g with
      | [ag] =>
        match σ.paths ag with
        | some p => runningB σ ℓ && r.alive && r.guard && σ.obligation g && decide (σ.authority g = some ℓ) && p.lock && p.valid &&
            noHandleB σ n (fun i => decide (i = g))
        | none => false
      | _ => false
    | none => false

/-- The step performed (meaningful where `check` admitted it). -/
def apply (σ : State) : Act → State
  | .establish ℓ r => establish σ ℓ r
  | .bind ℓ o => bind σ ℓ o
  | .borrow ℓ a0 m pend => match σ.paths a0 with
    | some p0 => borrow σ ℓ a0 p0 m pend
    | none => σ
  | .project ℓ a0 f => match σ.paths a0 with
    | some p0 => project σ ℓ a0 p0 f
    | none => σ
  | .lend ℓ a0 => match σ.paths a0 with
    | some p0 => lend σ ℓ a0 p0
    | none => σ
  | .takePath ℓ a => takePath σ ℓ a
  | .read _ _ => σ
  | .write _ a removed added => match σ.paths a with
    | some p => write σ p.of removed added
    | none => σ
  | .tempRoot ℓ o => tempRoot σ ℓ o
  | .bindParam ℓ a => bindParam σ ℓ a
  | .settle _ a => settle σ a
  | .reclaim ℓ r => reclaim σ ℓ r
  | .release _ ds => endObjects σ (inList ds)
  | .destroy _ o => endObject σ o
  | .endPlain _ o => endObject σ o
  | .transfer _ ℓ2 a1 => match σ.paths a1 with
    | some p1 => transferTo σ a1 ℓ2 p1.of
    | none => σ
  | .blockEnter ℓ f => blockEnter σ ℓ f
  | .blockExit ℓ => blockExit σ ℓ
  | .stmtEnter ℓ t => stmtEnter σ ℓ t
  | .stmtExit ℓ => stmtExit σ ℓ
  | .spawn ℓ ℓ' f' => spawn σ ℓ ℓ' f'
  | .threadDone ℓ' res => threadDone σ ℓ' res
  | .join ℓ ℓ' => match σ.threads ℓ' with
    | some tr => match tr.done with
      | some res => join σ ℓ ℓ' tr.handle res
      | none => σ
    | none => σ
  | .handleDestroy _ ℓ' => match σ.threads ℓ' with
    | some tr => match tr.done with
      | some res => handleDestroy σ ℓ' tr.handle res
      | none => σ
    | none => σ
  | .faultContain ℓ' => match σ.threads ℓ' with
    | some tr => faultContain σ ℓ' tr
    | none => σ
  | .lock ℓ am => match σ.paths am with
    | some pm => lock σ ℓ am pm
    | none => σ
  | .guardDrop _ g => match σ.contents g with
    | [ag] => match σ.paths ag with
      | some p => guardDrop σ g ag p.of
      | none => σ
    | _ => σ

/-- An admitted action, performed: `some` state. -/
def exec (σ : State) (n : Nat) (act : Act) : Option State :=
  if check σ n act then some (apply σ act) else none

/-! ### soundness: an admitted action is a step -/

theorem exec_sound (σ : State) (n : Nat) (act : Act) (hinv : σ.Invariants) (hth : ThreadsBelow σ n)
    (hc : check σ n act = true) : Step σ (apply σ act) := by
  have hf : σ.Fresh := hinv.2.2.2.1
  cases act with
  | establish ℓ r =>
    simp only [check, Bool.and_eq_true] at hc
    exact Step.establish σ ℓ r (running_of σ ℓ hc.1) (nonempty_of hc.2)
  | bind ℓ o =>
    simp only [check, Bool.and_eq_true] at hc
    obtain ⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩ := hc
    refine Step.bind σ ℓ o (running_of σ ℓ h1) (alive_of σ o h2) (nonempty_of h3) ?_ (nonempty_of h5) (tempOf_of σ ℓ o h6)
    intro ℓ' hau
    rw [hau] at h4
    exact of_decide_eq_true h4
  | borrow ℓ a0 m pend =>
    cases hp0 : σ.paths a0 with
    | none => simp [check, hp0] at hc
    | some p0 =>
      simp only [check, hp0, Bool.and_eq_true, Bool.or_eq_true, Bool.not_eq_true', decide_eq_true_eq, decide_eq_false_iff_not] at hc
      obtain ⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, _⟩ := hc
      simp only [apply, hp0]
      refine Step.borrow σ ℓ a0 p0 m pend (running_of σ ℓ h1) hp0 (tv_of σ a0 h2) ?_ ?_ (not_clash_of σ hf a0 m h5)
      · intro hm; rcases h3 with h | h
        · exact absurd hm h
        · exact h
      · intro hm; rcases h4 with h | h
        · exact absurd hm h
        · exact h
  | project ℓ a0 f =>
    cases hp0 : σ.paths a0 with
    | none => simp [check, hp0] at hc
    | some p0 =>
      simp only [check, hp0, Bool.and_eq_true] at hc
      simp only [apply, hp0]
      exact Step.project σ ℓ a0 p0 f (running_of σ ℓ hc.1.1) hp0 (tv_of σ a0 hc.1.2)
  | lend ℓ a0 =>
    cases hp0 : σ.paths a0 with
    | none => simp [check, hp0] at hc
    | some p0 =>
      simp only [check, hp0, Bool.and_eq_true, Bool.not_eq_true', decide_eq_true_eq] at hc
      obtain ⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩ := hc
      simp only [apply, hp0]
      exact Step.lend σ ℓ a0 p0 (running_of σ ℓ h1) hp0 (tv_of σ a0 h2) h3 h4 (affine_of σ p0 ℓ h5) (not_clash_of σ hf a0 _ h6)
  | takePath ℓ a =>
    cases hp : σ.paths a with
    | none => simp [check, hp] at hc
    | some p =>
      simp only [check, hp, Bool.and_eq_true, Bool.not_eq_true', decide_eq_true_eq] at hc
      obtain ⟨⟨⟨h1, h2⟩, h3⟩, h4⟩ := hc
      exact Step.takePath σ ℓ a p (running_of σ ℓ h1) hp h2 h3 (affine_of σ p ℓ h4)
  | read ℓ a =>
    cases hp : σ.paths a with
    | none => simp [check, hp] at hc
    | some p =>
      simp only [check, hp, Bool.and_eq_true, Bool.or_eq_true, Bool.not_eq_true', decide_eq_true_eq, decide_eq_false_iff_not] at hc
      obtain ⟨⟨⟨h1, h2⟩, h3⟩, h4⟩ := hc
      refine Step.read σ ℓ a p (running_of σ ℓ h1) hp (tv_of σ a h2) (not_clash_of σ hf a _ h3) ?_
      intro hm; rcases h4 with h | h
      · exact absurd hm h
      · exact h
  | write ℓ a removed added =>
    cases hp : σ.paths a with
    | none => simp [check, hp] at hc
    | some p =>
      simp only [check, hp, Bool.and_eq_true, Bool.not_eq_true', decide_eq_true_eq] at hc
      obtain ⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩ := hc
      simp only [apply, hp]
      refine Step.write σ ℓ a p removed added (running_of σ ℓ h1) hp (tv_of σ a h2) h3 h4 (not_clash_of σ hf a _ h5) ?_
      intro t ht
      have hm := List.all_eq_true.mp h6 t ht
      cases hq : σ.paths t with
      | none => simp [hq] at hm
      | some q =>
        simp only [hq, Bool.not_eq_true'] at hm
        exact ⟨q, rfl, hm⟩
  | tempRoot ℓ o =>
    simp only [check, Bool.and_eq_true] at hc
    obtain ⟨⟨⟨h1, h2⟩, h3⟩, h4⟩ := hc
    exact Step.tempRoot σ ℓ o (running_of σ ℓ h1) (alive_of σ o h2) (nonempty_of h3) (tempOf_of σ ℓ o h4)
  | bindParam ℓ a =>
    cases hp : σ.paths a with
    | none => simp [check, hp] at hc
    | some p =>
      simp only [check, hp, Bool.and_eq_true] at hc
      obtain ⟨⟨⟨h1, h2⟩, h3⟩, h4⟩ := hc
      exact Step.bindParam σ ℓ a p (running_of σ ℓ h1) (nonempty_of h2) hp (tv_of σ a h3) h4
  | settle ℓ a =>
    cases hp : σ.paths a with
    | none => simp [check, hp] at hc
    | some p =>
      simp only [check, hp, Bool.and_eq_true] at hc
      exact Step.settle σ ℓ a p (running_of σ ℓ hc.1) hp hc.2
  | reclaim ℓ r =>
    simp only [check, Bool.and_eq_true] at hc
    exact Step.reclaim σ ℓ r (running_of σ ℓ hc.1) (nonempty_of hc.2)
  | release ℓ ds =>
    simp only [check, Bool.and_eq_true] at hc
    obtain ⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩ := hc
    refine Step.release σ ℓ (inList ds) (running_of σ ℓ h1) ?_ ?_ (noHandle_of σ n hth _ h4) (noGuard_of σ hf _ h5)
    · intro o ho
      have hm := List.all_eq_true.mp h2 o (of_decide_eq_true ho)
      cases hr : σ.objects o with
      | none => simp [hr] at hm
      | some r => simp only [hr] at hm; exact ⟨r, rfl, hm⟩
    · intro o ho
      have hm := List.all_eq_true.mp h3 o (of_decide_eq_true ho)
      simp only [Bool.not_eq_true'] at hm
      exact not_foreignHeld_of σ hf ℓ o hm
  | destroy ℓ o =>
    simp only [check, Bool.and_eq_true, decide_eq_true_eq] at hc
    obtain ⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩, h7⟩ := hc
    exact Step.destroy σ ℓ o (running_of σ ℓ h1) (alive_of σ o h2) h3 h4 (noHandle_of σ n hth _ h5) (noGuard_of σ hf _ h6) (solitary_of σ hf o h7)
  | endPlain ℓ o =>
    simp only [check, Bool.and_eq_true, Bool.not_eq_true'] at hc
    obtain ⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩, h7⟩ := hc
    exact Step.endPlain σ ℓ o (running_of σ ℓ h1) (alive_of σ o h2) h3 (noHandle_of σ n hth _ h4) (noGuard_of σ hf _ h5) (solitary_of σ hf o h6) (owns_of σ ℓ o h7)
  | transfer ℓ ℓ2 a1 =>
    cases hp1 : σ.paths a1 with
    | none => simp [check, hp1] at hc
    | some p1 =>
      simp only [check, hp1, Bool.and_eq_true, decide_eq_true_eq] at hc
      obtain ⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩, h7⟩ := hc
      simp only [apply, hp1]
      refine Step.transfer σ ℓ ℓ2 a1 p1 (running_of σ ℓ h1) hp1 (tv_of σ a1 h2) h3 h4 (running_of σ ℓ2 h5) (nonempty_of h6) ?_
      intro a q hq hv hof
      have hm := List.all_eq_true.mp h7 a (List.mem_range.mpr (hf.1 a q hq))
      simp only [hq, hv, hof, decide_true, Bool.and_self, Bool.not_true, Bool.false_or, decide_eq_true_eq] at hm
      exact hm
  | blockEnter ℓ f =>
    simp only [check] at hc
    exact Step.blockEnter σ ℓ f (running_of σ ℓ hc)
  | blockExit ℓ =>
    simp only [check, Bool.and_eq_true] at hc
    obtain ⟨⟨⟨h1, h2⟩, h3⟩, h4⟩ := hc
    exact Step.blockExit σ ℓ (running_of σ ℓ h1) (nonempty_of h2) (noHandle_of σ n hth _ h3) (noGuard_of σ hf _ h4)
  | stmtEnter ℓ t =>
    simp only [check] at hc
    exact Step.stmtEnter σ ℓ t (running_of σ ℓ hc)
  | stmtExit ℓ =>
    simp only [check, Bool.and_eq_true] at hc
    obtain ⟨⟨⟨h1, h2⟩, h3⟩, h4⟩ := hc
    exact Step.stmtExit σ ℓ (running_of σ ℓ h1) (nonempty_of h2) (noHandle_of σ n hth _ h3) (noGuard_of σ hf _ h4)
  | spawn ℓ ℓ' f' =>
    simp only [check, Bool.and_eq_true, Option.isNone_iff_eq_none] at hc
    obtain ⟨⟨h1, h2⟩, h3⟩ := hc
    exact Step.spawn σ ℓ ℓ' f' (running_of σ ℓ h1) (nonempty_of h2) h3
  | threadDone ℓ' res =>
    cases htr : σ.threads ℓ' with
    | none => simp [check, htr] at hc
    | some tr =>
      simp only [check, htr, Bool.and_eq_true, Option.isNone_iff_eq_none, decide_eq_true_eq] at hc
      obtain ⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩, h7⟩ := hc
      cases hfr : tr.frames with
      | nil => rw [hfr] at h2; cases h2
      | cons f' rest =>
        cases rest with
        | cons _ _ => rw [hfr] at h2; cases h2
        | nil =>
          refine Step.threadDone σ ℓ' tr f' res htr h1 hfr h3 h4 ?_ (noHandle_of σ n hth _ h6) (noGuard_of σ hf _ h7)
          intro o ho hob
          have hm := List.all_eq_true.mp h5 o ho
          simp only [hob, Bool.not_true, Bool.false_or, decide_eq_true_eq] at hm
          exact hm
  | join ℓ ℓ' =>
    cases htr : σ.threads ℓ' with
    | none => simp [check, htr] at hc
    | some tr =>
      cases hd : tr.done with
      | none => simp [check, htr, hd] at hc
      | some res =>
        simp only [check, htr, hd, Bool.and_eq_true, decide_eq_true_eq] at hc
        obtain ⟨⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩, h7⟩, h8⟩ := hc
        simp only [apply, htr, hd]
        refine Step.join σ ℓ ℓ' tr res (running_of σ ℓ h1) h2 (nonempty_of h3) htr hd h4 h5 h6 ?_ ?_
        · intro ℓ'' tr'' hne htr''
          have hm := List.all_eq_true.mp h7 ℓ'' (List.mem_range.mpr (hth ℓ'' tr'' htr''))
          simp only [htr'', Bool.or_eq_true, decide_eq_true_eq] at hm
          rcases hm with h | h
          · exact absurd h hne
          · exact h
        · intro o r hr hrecl hob
          have hm := List.all_eq_true.mp h8 o (List.mem_range.mpr (hf.2.1 o r hr))
          simp only [hr, hrecl, hob, Bool.true_and, Bool.not_eq_true', decide_eq_false_iff_not] at hm
          exact hm
  | handleDestroy ℓ ℓ' =>
    cases htr : σ.threads ℓ' with
    | none => simp [check, htr] at hc
    | some tr =>
      cases hd : tr.done with
      | none => simp [check, htr, hd] at hc
      | some res =>
        simp only [check, htr, hd, Bool.and_eq_true, decide_eq_true_eq] at hc
        obtain ⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩, h7⟩ := hc
        simp only [apply, htr, hd]
        refine Step.handleDestroy σ ℓ ℓ' tr res (running_of σ ℓ h1) htr hd h2 h3 h4 ?_ (noGuard_of σ hf _ h6) ?_
        · intro ℓ'' tr'' hne htr''
          have hm := List.all_eq_true.mp h5 ℓ'' (List.mem_range.mpr (hth ℓ'' tr'' htr''))
          simp only [htr'', Bool.or_eq_true, Bool.and_eq_true, Bool.not_eq_true', decide_eq_true_eq, decide_eq_false_iff_not] at hm
          rcases hm with h | h
          · exact absurd h hne
          · exact h
        · intro o r hr hrecl hob
          have hm := List.all_eq_true.mp h7 o (List.mem_range.mpr (hf.2.1 o r hr))
          simp only [hr, hrecl, hob, Bool.true_and, Bool.not_eq_true', decide_eq_false_iff_not] at hm
          exact hm
  | faultContain ℓ' =>
    cases htr : σ.threads ℓ' with
    | none => simp [check, htr] at hc
    | some tr =>
      simp only [check, htr, Bool.and_eq_true, Option.isNone_iff_eq_none] at hc
      obtain ⟨⟨⟨h1, h2⟩, h3⟩, h4⟩ := hc
      simp only [apply, htr]
      exact Step.faultContain σ ℓ' tr htr h1 (contained_of σ hf ℓ' h2) h3 (noHandle_of σ n hth _ h4)
  | lock ℓ am =>
    cases hpm : σ.paths am with
    | none => simp [check, hpm] at hc
    | some pm =>
      simp only [check, hpm, Bool.and_eq_true, decide_eq_true_eq] at hc
      obtain ⟨⟨⟨h1, h2⟩, h3⟩, h4⟩ := hc
      simp only [apply, hpm]
      exact Step.lock σ ℓ am pm (running_of σ ℓ h1) (nonempty_of h2) hpm (tv_of σ am h3) h4
  | guardDrop ℓ g =>
    cases hr : σ.objects g with
    | none => simp [check, hr] at hc
    | some r =>
      cases hcg : σ.contents g with
      | nil => simp [check, hr, hcg] at hc
      | cons ag rest =>
        cases rest with
        | cons _ _ => simp [check, hr, hcg] at hc
        | nil =>
          cases hp : σ.paths ag with
          | none => simp [check, hr, hcg, hp] at hc
          | some p =>
            simp only [check, hr, hcg, hp, Bool.and_eq_true, decide_eq_true_eq] at hc
            obtain ⟨⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩, h7⟩, h8⟩ := hc
            simp only [apply, hcg, hp]
            exact Step.guardDrop σ ℓ g r ag p (running_of σ ℓ h1) hr h2 h3 h4 h5 hcg hp h6 h7 (noHandle_of σ n hth _ h8)

/-- So a replayed trace of admitted actions stays within the reachable states: each state is reachable
    from the first, and every lemma proved of reachable states holds of it. -/
theorem exec_reachable (σ σ' : State) (n : Nat) (act : Act) (hr : Reachable σ) (hth : ThreadsBelow σ n)
    (h : exec σ n act = some σ') : Reachable σ' := by
  unfold exec at h
  split at h
  · rename_i hc
    cases h
    exact Reachable.step σ _ hr (exec_sound σ n act (reachable_invariants σ hr) hth hc)
  · cases h

end Cobalt
