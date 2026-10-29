import Cobalt.Corollaries
/-
  The elision lemmas (D-0270): what `cobc` relies on when it omits a check.

  `spec/08` §4 `[Standing-Local]` (D-0188, D-0209): a path held by a thread ℓ that arrived valid and
  clash-free, not lock-derived, not pending, over fresh storage, keeps its standing under every step of
  another thread, so the implementation may check it once where it arrives and omit the use-time tests
  while ℓ holds it. Here, for the fragment:

  - `path_stays`, `object_stays`, `running_stays`: a step by ℓ' ≠ ℓ leaves ℓ's standing path valid, its
    object alive (fresh storage, no guard, no handle, in no finished result, owned by no frame or scope
    of ℓ'), its record's fields as they were, and ℓ running; `standing_step` and `standing_run` put
    them together over one step and over a run of other threads' steps.
  - `read_conflict_diagnosed`, `write_conflict_diagnosed`: a standing path is a `clash` witness for every
    access through a path that overlaps it in an unpermitted mode, so such an access is refused by the
    step's own premise (`spec/08` §4: "every conflict involving such a path is still diagnosed, at the
    other thread's access"). This, not the preservation of `¬clash(a)` the prose states, is what the
    elision rests on: another thread may still bind a pending argument that overlaps `a` (D-0022 finds
    the conflict at the first use), and that use is the access refused here.
  - `unreached_stays`: an object no valid path reaches — an objectless local (D-0200), whose uses lend it
    with no path minted — is reached by no other thread's step: no valid path of it exists afterwards
    either, and it is alive still. So such a local needs no runtime record for the other threads' sake.

  The fragment was made faithful where these lemmas need it (D-0270): `clash` exempts lock paths
  (`sync-exempt`), `[Destroy]` and `[Object-End]` require `solitary`, `[Authority-Transfer]` admits no
  third valid path, a binding or a temporary root is of a temporary of the thread's own scope, and a
  whole-object move is of the thread's own object.
-/
namespace Cobalt
open State

/-! ### the performer -/

/-- A step of the fragment performed by thread ℓ: `Step` with its performer exposed as an index. The
    constructors are `Step`'s, with the same premises. -/
inductive StepBy : Thread → State → State → Prop where
  | establish (σ : State) (ℓ : Thread) (r : Bool) :
      σ.running ℓ → σ.scopes ℓ ≠ [] → StepBy ℓ σ (establish σ ℓ r)
  | bind (σ : State) (ℓ : Thread) (o : Identity) :
      σ.running ℓ → σ.alive o → σ.frames ℓ ≠ [] →
      (∀ ℓ', σ.authority o = some ℓ' → ℓ' = ℓ) →
      σ.scopes ℓ ≠ [] → σ.tempOf ℓ o →
      StepBy ℓ σ (bind σ ℓ o)
  | borrow (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (m : Mode) (pend : Bool) :
      σ.running ℓ → σ.paths a0 = some p0 → σ.temporallyValid a0 →
      (m = Mode.exclusive → p0.mode = Mode.exclusive) →
      (p0.mode = Mode.exclusive → p0.thread = some ℓ) →
      ¬ σ.clash a0 m → StepBy ℓ σ (borrow σ ℓ a0 p0 m pend)
  | project (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (f : Proj) :
      σ.running ℓ → σ.paths a0 = some p0 → σ.temporallyValid a0 → StepBy ℓ σ (project σ ℓ a0 p0 f)
  | lend (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) :
      σ.running ℓ → σ.paths a0 = some p0 → σ.temporallyValid a0 → p0.mode = Mode.exclusive →
      p0.lock = false → σ.affine p0 ℓ → ¬ σ.clash a0 Mode.exclusive → StepBy ℓ σ (lend σ ℓ a0 p0)
  | takePath (σ : State) (ℓ : Thread) (a : Token) (p : PathRecord) :
      σ.running ℓ → σ.paths a = some p → p.mode = Mode.exclusive → p.lock = false → σ.affine p ℓ →
      StepBy ℓ σ (takePath σ ℓ a)
  | read (σ : State) (ℓ : Thread) (a : Token) (p : PathRecord) :
      σ.running ℓ → σ.paths a = some p → σ.temporallyValid a → ¬ σ.clash a Mode.shared →
      (p.mode = Mode.exclusive → p.thread = some ℓ) → StepBy ℓ σ σ
  | write (σ : State) (ℓ : Thread) (a : Token) (p : PathRecord) (removed added : List Token) :
      σ.running ℓ → σ.paths a = some p → σ.temporallyValid a → p.mode = Mode.exclusive → p.thread = some ℓ →
      ¬ σ.clash a Mode.exclusive →
      (∀ t ∈ added, ∃ q, σ.paths t = some q ∧ q.pending = false) →
      StepBy ℓ σ (write σ p.of removed added)
  | tempRoot (σ : State) (ℓ : Thread) (o : Identity) :
      σ.running ℓ → σ.alive o → σ.scopes ℓ ≠ [] → σ.tempOf ℓ o → StepBy ℓ σ (tempRoot σ ℓ o)
  | bindParam (σ : State) (ℓ : Thread) (a : Token) (p : PathRecord) :
      σ.running ℓ → σ.frames ℓ ≠ [] → σ.paths a = some p → σ.temporallyValid a → p.pending = true →
      StepBy ℓ σ (bindParam σ ℓ a)
  | settle (σ : State) (ℓ : Thread) (a : Token) (p : PathRecord) :
      σ.running ℓ → σ.paths a = some p → p.pending = true → StepBy ℓ σ (settle σ a)
  | relocateIn (σ : State) (ℓ : Thread) (oS o : Identity) (path : List Proj) :
      σ.running ℓ → σ.alive oS → σ.alive o → oS ≠ o → path ≠ [] →
      σ.obligation o = true → σ.authority o = some ℓ →
      (σ.obligation oS = true → σ.authority oS = some ℓ) →
      (∀ q, path.isPrefixOf q = true → σ.subs o q = none) →
      (∀ a q, σ.paths a = some q → q.of = oS → q.valid = true → σ.holder oS = some a) →
      σ.noHandle (fun i => decide (i = oS)) → σ.noGuard (fun i => decide (i = oS)) →
      σ.owns ℓ oS →
      StepBy ℓ σ (relocateIn σ oS o path)
  | relocateOut (σ : State) (ℓ : Thread) (o : Identity) (path : List Proj) (moved : List Token) :
      σ.running ℓ → σ.scopes ℓ ≠ [] → σ.alive o → σ.subs o path = some ℓ →
      (∀ t ∈ moved, t ∈ σ.contents o) → StepBy ℓ σ (relocateOut σ ℓ o path moved)
  | destroyComposite (σ : State) (ℓ : Thread) (o : Identity) :
      σ.running ℓ → σ.alive o → σ.authority o = some ℓ → StepBy ℓ σ (destroyComposite σ o)
  | reclaim (σ : State) (ℓ : Thread) (r : Bool) :
      σ.running ℓ → σ.scopes ℓ ≠ [] → StepBy ℓ σ (reclaim σ ℓ r)
  | reattach (σ : State) (ℓ : Thread) (o : Identity) (r : ObjectRecord) :
      σ.running ℓ → σ.objects o = some r → r.alive = true → r.reclaimed = true → σ.obligation o = true →
      StepBy ℓ σ (reattach σ ℓ o)
  | release (σ : State) (ℓ : Thread) (dead : Identity → Bool) :
      σ.running ℓ → (∀ o, dead o = true → ∃ r, σ.objects o = some r ∧ r.reclaimed = true) →
      (∀ o, dead o = true → ¬ σ.foreignHeld ℓ o) →
      σ.noHandle dead → σ.noGuard dead →
      StepBy ℓ σ (endObjects σ dead)
  | rawptrMoveIn (σ : State) (ℓ : Thread) (oS : Identity) :
      σ.running ℓ → σ.alive oS → (σ.obligation oS = true → σ.authority oS = some ℓ) →
      (∀ a q, σ.paths a = some q → q.of = oS → q.valid = true → σ.holder oS = some a) →
      σ.noHandle (fun i => decide (i = oS)) → σ.noGuard (fun i => decide (i = oS)) →
      σ.owns ℓ oS →
      StepBy ℓ σ (moveWhole σ ℓ oS true)
  | rawptrMoveOut (σ : State) (ℓ : Thread) (oS : Identity) (r : ObjectRecord) :
      σ.running ℓ → σ.scopes ℓ ≠ [] → σ.objects oS = some r → r.alive = true → r.reclaimed = true →
      ¬ σ.foreignHeld ℓ oS →
      σ.noHandle (fun i => decide (i = oS)) → σ.noGuard (fun i => decide (i = oS)) →
      StepBy ℓ σ (moveWhole σ ℓ oS false)
  | destroy (σ : State) (ℓ : Thread) (o : Identity) :
      σ.running ℓ → σ.alive o → σ.obligation o = true → σ.authority o = some ℓ →
      σ.noHandle (fun i => decide (i = o)) → σ.noGuard (fun i => decide (i = o)) →
      σ.solitary o →
      StepBy ℓ σ (endObject σ o)
  | endPlain (σ : State) (ℓ : Thread) (o : Identity) :
      σ.running ℓ → σ.alive o → σ.obligation o = false →
      σ.noHandle (fun i => decide (i = o)) → σ.noGuard (fun i => decide (i = o)) →
      σ.solitary o → σ.owns ℓ o →
      StepBy ℓ σ (endObject σ o)
  | transfer (σ : State) (ℓ ℓ2 : Thread) (a1 : Token) (p1 : PathRecord) :
      σ.running ℓ → σ.paths a1 = some p1 → σ.temporallyValid a1 → σ.authority p1.of = some ℓ →
      σ.obligation p1.of = true → σ.running ℓ2 → σ.frames ℓ2 ≠ [] →
      (∀ a q, σ.paths a = some q → q.valid = true → q.of = p1.of → a = a1) →
      StepBy ℓ σ (transferTo σ a1 ℓ2 p1.of)
  | blockEnter (σ : State) (ℓ : Thread) (f : Frame) :
      σ.running ℓ → StepBy ℓ σ (blockEnter σ ℓ f)
  | blockExit (σ : State) (ℓ : Thread) :
      σ.running ℓ → σ.frames ℓ ≠ [] → σ.noHandle (blockDead σ ℓ) → σ.noGuard (blockDead σ ℓ) → StepBy ℓ σ (blockExit σ ℓ)
  | stmtEnter (σ : State) (ℓ : Thread) (t : Scope) :
      σ.running ℓ → StepBy ℓ σ (stmtEnter σ ℓ t)
  | stmtExit (σ : State) (ℓ : Thread) :
      σ.running ℓ → σ.scopes ℓ ≠ [] → σ.noHandle (stmtDead σ ℓ) → σ.noGuard (stmtDead σ ℓ) → StepBy ℓ σ (stmtExit σ ℓ)
  | spawn (σ : State) (ℓ ℓ' : Thread) (f' : Frame) :
      σ.running ℓ → σ.scopes ℓ ≠ [] → σ.threads ℓ' = none → StepBy ℓ σ (spawn σ ℓ ℓ' f')
  | threadDone (σ : State) (ℓ' : Thread) (tr : ThreadRec) (f' : Frame) (res : List Identity) :
      σ.threads ℓ' = some tr → tr.done = none → tr.frames = [f'] → tr.scopes = [] →
      σ.obligation tr.handle = true →
      (∀ o ∈ res, σ.obligation o = true → σ.authority o = some ℓ') →
      σ.noHandle (fun i => blockDead σ ℓ' i && !decide (i ∈ res)) →
      σ.noGuard (fun i => blockDead σ ℓ' i && !decide (i ∈ res)) →
      StepBy ℓ' σ (threadDone σ ℓ' res)
  | join (σ : State) (ℓ ℓ' : Thread) (tr : ThreadRec) (res : List Identity) :
      σ.running ℓ → ℓ ≠ ℓ' → σ.scopes ℓ ≠ [] →
      σ.threads ℓ' = some tr → tr.done = some res → tr.frames = [] → tr.scopes = [] →
      σ.authority tr.handle = some ℓ →
      (∀ ℓ'' tr'', ℓ'' ≠ ℓ' → σ.threads ℓ'' = some tr'' → tr''.handle ≠ tr.handle) →
      (∀ o r, σ.objects o = some r → r.reclaimed = true → σ.obligation o = true → σ.authority o ≠ some ℓ') →
      StepBy ℓ σ (join σ ℓ ℓ' tr.handle res)
  | handleDestroy (σ : State) (ℓ ℓ' : Thread) (tr : ThreadRec) (res : List Identity) :
      σ.running ℓ → σ.threads ℓ' = some tr → tr.done = some res → tr.frames = [] → tr.scopes = [] →
      σ.authority tr.handle = some ℓ →
      (∀ ℓ'' tr'', ℓ'' ≠ ℓ' → σ.threads ℓ'' = some tr'' → tr''.handle ≠ tr.handle ∧ tr''.handle ∉ res) →
      σ.noGuard (fun i => decide (i = tr.handle) || decide (i ∈ res)) →
      (∀ o r, σ.objects o = some r → r.reclaimed = true → σ.obligation o = true → σ.authority o ≠ some ℓ') →
      StepBy ℓ σ (handleDestroy σ ℓ' tr.handle res)
  | faultContain (σ : State) (ℓ' : Thread) (tr : ThreadRec) :
      σ.threads ℓ' = some tr → tr.done = none → σ.contained ℓ' →
      σ.obligation tr.handle = true →
      σ.noHandle (fcDead σ tr) →
      StepBy ℓ' σ (faultContain σ ℓ' tr)
  | lock (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) :
      σ.running ℓ → σ.scopes ℓ ≠ [] → σ.paths am = some pm → σ.temporallyValid am →
      σ.sync pm.of = SyncState.unlocked →
      StepBy ℓ σ (lock σ ℓ am pm)
  | guardDrop (σ : State) (ℓ : Thread) (g : Identity) (r : ObjectRecord) (ag : Token) (p : PathRecord) :
      σ.running ℓ → σ.objects g = some r → r.alive = true → r.guard = true →
      σ.obligation g = true → σ.authority g = some ℓ → σ.contents g = [ag] →
      σ.paths ag = some p → p.lock = true → p.valid = true →
      σ.noHandle (fun i => decide (i = g)) →
      StepBy ℓ σ (guardDrop σ g ag p.of)

/-- A step by some thread is a step. -/
theorem StepBy.toStep {ℓ : Thread} {σ σ' : State} (h : StepBy ℓ σ σ') : Step σ σ' := by
  cases h with
  | establish _ _ r h1 h2 => exact Step.establish σ ℓ r h1 h2
  | bind _ _ o h1 h2 h3 h4 h5 h6 => exact Step.bind σ ℓ o h1 h2 h3 h4 h5 h6
  | borrow _ _ a0 p0 m pend h1 h2 h3 h4 h5 h6 => exact Step.borrow σ ℓ a0 p0 m pend h1 h2 h3 h4 h5 h6
  | project _ _ a0 p0 f h1 h2 h3 => exact Step.project σ ℓ a0 p0 f h1 h2 h3
  | lend _ _ a0 p0 h1 h2 h3 h4 h5 h6 h7 => exact Step.lend σ ℓ a0 p0 h1 h2 h3 h4 h5 h6 h7
  | takePath _ _ a p h1 h2 h3 h4 h5 => exact Step.takePath σ ℓ a p h1 h2 h3 h4 h5
  | read _ _ a p h1 h2 h3 h4 h5 => exact Step.read σ ℓ a p h1 h2 h3 h4 h5
  | write _ _ a p removed added h1 h2 h3 h4 h5 h6 h7 => exact Step.write σ ℓ a p removed added h1 h2 h3 h4 h5 h6 h7
  | tempRoot _ _ o h1 h2 h3 h4 => exact Step.tempRoot σ ℓ o h1 h2 h3 h4
  | bindParam _ _ a p h1 h2 h3 h4 h5 => exact Step.bindParam σ ℓ a p h1 h2 h3 h4 h5
  | settle _ _ a p h1 h2 h3 => exact Step.settle σ ℓ a p h1 h2 h3
  | relocateIn _ _ oS o path h1 h2 h3 h4 h5 h6 h7 h8 h9 h10 h11 h12 h13 => exact Step.relocateIn σ ℓ oS o path h1 h2 h3 h4 h5 h6 h7 h8 h9 h10 h11 h12 h13
  | relocateOut _ _ o path moved h1 h2 h3 h4 h5 => exact Step.relocateOut σ ℓ o path moved h1 h2 h3 h4 h5
  | destroyComposite _ _ o h1 h2 h3 => exact Step.destroyComposite σ ℓ o h1 h2 h3
  | reclaim _ _ r h1 h2 => exact Step.reclaim σ ℓ r h1 h2
  | reattach _ _ o r h1 h2 h3 h4 h5 => exact Step.reattach σ ℓ o r h1 h2 h3 h4 h5
  | release _ _ dead h1 h2 h3 h4 h5 => exact Step.release σ ℓ dead h1 h2 h3 h4 h5
  | rawptrMoveIn _ _ oS h1 h2 h3 h4 h5 h6 h7 => exact Step.rawptrMoveIn σ ℓ oS h1 h2 h3 h4 h5 h6 h7
  | rawptrMoveOut _ _ oS r h1 h2 h3 h4 h5 h6 h7 h8 => exact Step.rawptrMoveOut σ ℓ oS r h1 h2 h3 h4 h5 h6 h7 h8
  | destroy _ _ o h1 h2 h3 h4 h5 h6 h7 => exact Step.destroy σ ℓ o h1 h2 h3 h4 h5 h6 h7
  | endPlain _ _ o h1 h2 h3 h4 h5 h6 h7 => exact Step.endPlain σ ℓ o h1 h2 h3 h4 h5 h6 h7
  | transfer _ _ ℓ2 a1 p1 h1 h2 h3 h4 h5 h6 h7 h8 => exact Step.transfer σ ℓ ℓ2 a1 p1 h1 h2 h3 h4 h5 h6 h7 h8
  | blockEnter _ _ f h1 => exact Step.blockEnter σ ℓ f h1
  | blockExit _ _ h1 h2 h3 h4 => exact Step.blockExit σ ℓ h1 h2 h3 h4
  | stmtEnter _ _ t h1 => exact Step.stmtEnter σ ℓ t h1
  | stmtExit _ _ h1 h2 h3 h4 => exact Step.stmtExit σ ℓ h1 h2 h3 h4
  | spawn _ _ ℓ' f' h1 h2 h3 => exact Step.spawn σ ℓ ℓ' f' h1 h2 h3
  | threadDone _ _ tr f' res h1 h2 h3 h4 h5 h6 h7 h8 => exact Step.threadDone σ ℓ tr f' res h1 h2 h3 h4 h5 h6 h7 h8
  | join _ _ ℓ' tr res h1 h2 h3 h4 h5 h6 h7 h8 h9 h10 => exact Step.join σ ℓ ℓ' tr res h1 h2 h3 h4 h5 h6 h7 h8 h9 h10
  | handleDestroy _ _ ℓ' tr res h1 h2 h3 h4 h5 h6 h7 h8 h9 => exact Step.handleDestroy σ ℓ ℓ' tr res h1 h2 h3 h4 h5 h6 h7 h8 h9
  | faultContain _ _ tr h1 h2 h3 h4 h5 => exact Step.faultContain σ ℓ tr h1 h2 h3 h4 h5
  | lock _ _ am pm h1 h2 h3 h4 h5 => exact Step.lock σ ℓ am pm h1 h2 h3 h4 h5
  | guardDrop _ _ g r ag p h1 h2 h3 h4 h5 h6 h7 h8 h9 h10 h11 => exact Step.guardDrop σ ℓ g r ag p h1 h2 h3 h4 h5 h6 h7 h8 h9 h10 h11

/-- Every step is some thread's. -/
theorem Step.by {σ σ' : State} (h : Step σ σ') : ∃ ℓ, StepBy ℓ σ σ' := by
  cases h with
  | establish ℓ r h1 h2 => exact ⟨ℓ, StepBy.establish σ ℓ r h1 h2⟩
  | bind ℓ o h1 h2 h3 h4 h5 h6 => exact ⟨ℓ, StepBy.bind σ ℓ o h1 h2 h3 h4 h5 h6⟩
  | borrow ℓ a0 p0 m pend h1 h2 h3 h4 h5 h6 => exact ⟨ℓ, StepBy.borrow σ ℓ a0 p0 m pend h1 h2 h3 h4 h5 h6⟩
  | project ℓ a0 p0 f h1 h2 h3 => exact ⟨ℓ, StepBy.project σ ℓ a0 p0 f h1 h2 h3⟩
  | lend ℓ a0 p0 h1 h2 h3 h4 h5 h6 h7 => exact ⟨ℓ, StepBy.lend σ ℓ a0 p0 h1 h2 h3 h4 h5 h6 h7⟩
  | takePath ℓ a p h1 h2 h3 h4 h5 => exact ⟨ℓ, StepBy.takePath σ ℓ a p h1 h2 h3 h4 h5⟩
  | read ℓ a p h1 h2 h3 h4 h5 => exact ⟨ℓ, StepBy.read σ ℓ a p h1 h2 h3 h4 h5⟩
  | write ℓ a p removed added h1 h2 h3 h4 h5 h6 h7 => exact ⟨ℓ, StepBy.write σ ℓ a p removed added h1 h2 h3 h4 h5 h6 h7⟩
  | tempRoot ℓ o h1 h2 h3 h4 => exact ⟨ℓ, StepBy.tempRoot σ ℓ o h1 h2 h3 h4⟩
  | bindParam ℓ a p h1 h2 h3 h4 h5 => exact ⟨ℓ, StepBy.bindParam σ ℓ a p h1 h2 h3 h4 h5⟩
  | settle ℓ a p h1 h2 h3 => exact ⟨ℓ, StepBy.settle σ ℓ a p h1 h2 h3⟩
  | relocateIn ℓ oS o path h1 h2 h3 h4 h5 h6 h7 h8 h9 h10 h11 h12 h13 => exact ⟨ℓ, StepBy.relocateIn σ ℓ oS o path h1 h2 h3 h4 h5 h6 h7 h8 h9 h10 h11 h12 h13⟩
  | relocateOut ℓ o path moved h1 h2 h3 h4 h5 => exact ⟨ℓ, StepBy.relocateOut σ ℓ o path moved h1 h2 h3 h4 h5⟩
  | destroyComposite ℓ o h1 h2 h3 => exact ⟨ℓ, StepBy.destroyComposite σ ℓ o h1 h2 h3⟩
  | reclaim ℓ r h1 h2 => exact ⟨ℓ, StepBy.reclaim σ ℓ r h1 h2⟩
  | reattach ℓ o r h1 h2 h3 h4 h5 => exact ⟨ℓ, StepBy.reattach σ ℓ o r h1 h2 h3 h4 h5⟩
  | release ℓ dead h1 h2 h3 h4 h5 => exact ⟨ℓ, StepBy.release σ ℓ dead h1 h2 h3 h4 h5⟩
  | rawptrMoveIn ℓ oS h1 h2 h3 h4 h5 h6 h7 => exact ⟨ℓ, StepBy.rawptrMoveIn σ ℓ oS h1 h2 h3 h4 h5 h6 h7⟩
  | rawptrMoveOut ℓ oS r h1 h2 h3 h4 h5 h6 h7 h8 => exact ⟨ℓ, StepBy.rawptrMoveOut σ ℓ oS r h1 h2 h3 h4 h5 h6 h7 h8⟩
  | destroy ℓ o h1 h2 h3 h4 h5 h6 h7 => exact ⟨ℓ, StepBy.destroy σ ℓ o h1 h2 h3 h4 h5 h6 h7⟩
  | endPlain ℓ o h1 h2 h3 h4 h5 h6 h7 => exact ⟨ℓ, StepBy.endPlain σ ℓ o h1 h2 h3 h4 h5 h6 h7⟩
  | transfer ℓ ℓ2 a1 p1 h1 h2 h3 h4 h5 h6 h7 h8 => exact ⟨ℓ, StepBy.transfer σ ℓ ℓ2 a1 p1 h1 h2 h3 h4 h5 h6 h7 h8⟩
  | blockEnter ℓ f h1 => exact ⟨ℓ, StepBy.blockEnter σ ℓ f h1⟩
  | blockExit ℓ h1 h2 h3 h4 => exact ⟨ℓ, StepBy.blockExit σ ℓ h1 h2 h3 h4⟩
  | stmtEnter ℓ t h1 => exact ⟨ℓ, StepBy.stmtEnter σ ℓ t h1⟩
  | stmtExit ℓ h1 h2 h3 h4 => exact ⟨ℓ, StepBy.stmtExit σ ℓ h1 h2 h3 h4⟩
  | spawn ℓ ℓ' f' h1 h2 h3 => exact ⟨ℓ, StepBy.spawn σ ℓ ℓ' f' h1 h2 h3⟩
  | threadDone ℓ' tr f' res h1 h2 h3 h4 h5 h6 h7 h8 => exact ⟨ℓ', StepBy.threadDone σ ℓ' tr f' res h1 h2 h3 h4 h5 h6 h7 h8⟩
  | join ℓ ℓ' tr res h1 h2 h3 h4 h5 h6 h7 h8 h9 h10 => exact ⟨ℓ, StepBy.join σ ℓ ℓ' tr res h1 h2 h3 h4 h5 h6 h7 h8 h9 h10⟩
  | handleDestroy ℓ ℓ' tr res h1 h2 h3 h4 h5 h6 h7 h8 h9 => exact ⟨ℓ, StepBy.handleDestroy σ ℓ ℓ' tr res h1 h2 h3 h4 h5 h6 h7 h8 h9⟩
  | faultContain ℓ' tr h1 h2 h3 h4 h5 => exact ⟨ℓ', StepBy.faultContain σ ℓ' tr h1 h2 h3 h4 h5⟩
  | lock ℓ am pm h1 h2 h3 h4 h5 => exact ⟨ℓ, StepBy.lock σ ℓ am pm h1 h2 h3 h4 h5⟩
  | guardDrop ℓ g r ag p h1 h2 h3 h4 h5 h6 h7 h8 h9 h10 h11 => exact ⟨ℓ, StepBy.guardDrop σ ℓ g r ag p h1 h2 h3 h4 h5 h6 h7 h8 h9 h10 h11⟩

/-! ### the hypotheses -/

/-- `[Standing-Local]`'s H1–H3 and H5 for a path `a` of ℓ, with its object's record: ℓ's own (its forming
    thread, after any taking), valid, not lock-derived (H1), not pending (H2), temporally valid (H3; its
    clash-freedom is the separate matter of `read_conflict_diagnosed`), over fresh storage (H5), which is
    no guard; and ℓ running (H6: ℓ holds it). -/
structure Standing (σ : State) (ℓ : Thread) (a : Token) (p : PathRecord) (r : ObjectRecord) : Prop where
  path : σ.paths a = some p
  thread : p.thread = some ℓ
  valid : p.valid = true
  lock : p.lock = false
  pending : p.pending = false
  obj : σ.objects p.of = some r
  alive : r.alive = true
  fresh : r.reclaimed = false
  guard : r.guard = false
  running : σ.running ℓ

/-- The object `o` (record `r`) is not ℓ'`s to end: owned by no open frame of ℓ', a temporary of no open
    scope of ℓ', no thread's handle, and in no finished thread's result. (H6's counterpart for the other
    thread: what it ends by leaving a frame or scope, by finishing, or by a handle's end is its own.) -/
structure ApartObj (σ : State) (ℓ' : Thread) (o : Identity) (r : ObjectRecord) : Prop where
  frame : ∀ f, r.frame = some f → f ∉ σ.frames ℓ'
  scope : r.frame = none → r.scope ∉ σ.scopes ℓ'
  handle : ∀ ℓ'' tr, σ.threads ℓ'' = some tr → tr.handle ≠ o
  result : ∀ ℓ'' tr res, σ.threads ℓ'' = some tr → tr.done = some res → o ∉ res

/-- `ApartObj`, and the path was formed in no open scope of ℓ' (a statement's exit in ℓ' ends the unheld
    paths of its scope). -/
structure Apart (σ : State) (ℓ' : Thread) (o : Identity) (r : ObjectRecord) (p : PathRecord) : Prop where
  obj : ApartObj σ ℓ' o r
  scope : p.scope ∉ σ.scopes ℓ'

/-! ### helpers -/

theorem path_lt (σ : State) (hf : σ.Fresh) (a : Token) (p : PathRecord) (hp : σ.paths a = some p) : a < σ.nextId :=
  hf.1 a p hp

theorem obj_lt (σ : State) (hf : σ.Fresh) (o : Identity) (r : ObjectRecord) (hr : σ.objects o = some r) : o < σ.nextId :=
  hf.2.1 o r hr

theorem ne_of_lt' {a b : Nat} (h : a < b) : a ≠ b := Nat.ne_of_lt h
theorem ne_of_lt_succ {a b : Nat} (h : a < b) : a ≠ b + 1 := Nat.ne_of_lt (Nat.lt_succ_of_lt h)

/-- An object owned by a running thread's open frame or scope, under a thread's authority, is not apart
    from that thread: `inv.resource-authority`'s owner is the authority thread, and `ApartObj` denies it
    every way of owning a fresh-storage object. -/
theorem authority_not_apart (σ : State) (hR : σ.R) (ℓ' : Thread) (o : Identity) (r : ObjectRecord)
    (hr : σ.objects o = some r) (hfresh : r.reclaimed = false) (hob : σ.obligation o = true)
    (hau : σ.authority o = some ℓ') (hrun : σ.running ℓ') (hap : ApartObj σ ℓ' o r) : False := by
  obtain ⟨r', ℓ'', tr, hr', _, hau', htr, how⟩ := hR o hob
  rw [hr] at hr'
  cases hr'
  rw [hau] at hau'
  cases hau'
  rcases how with ⟨f, hfr, hmem⟩ | ⟨hfn, hsc⟩ | ⟨res, hd, _, _⟩ | hrecl
  · exact hap.frame f hfr (by rw [frames_of σ ℓ' tr htr]; exact hmem)
  · exact hap.scope hfn (by rw [scopes_of σ ℓ' tr htr]; exact hsc)
  · obtain ⟨tr', htr', hdone⟩ := hrun
    rw [htr] at htr'
    cases htr'
    rw [hd] at hdone
    cases hdone
  · rw [hfresh] at hrecl
    cases hrecl

/-- What a thread owns (`owns`) is not apart from it. -/
theorem owns_not_apart (σ : State) (ℓ' : Thread) (o : Identity) (r : ObjectRecord) (hr : σ.objects o = some r)
    (hown : σ.owns ℓ' o) (hap : ApartObj σ ℓ' o r) : False := by
  obtain ⟨r', hr', h⟩ := hown
  rw [hr] at hr'
  cases hr'
  rcases h with ⟨f, hfr, hmem⟩ | ⟨hfn, hsc⟩
  · exact hap.frame f hfr hmem
  · exact hap.scope hfn hsc

/-- A temporary of a thread's current scope is not apart from it. -/
theorem tempOf_not_apart (σ : State) (ℓ' : Thread) (o : Identity) (r : ObjectRecord) (hr : σ.objects o = some r)
    (hsc : σ.scopes ℓ' ≠ []) (ht : σ.tempOf ℓ' o) (hap : ApartObj σ ℓ' o r) : False := by
  obtain ⟨r', hr', hfn, hs⟩ := ht
  rw [hr] at hr'
  cases hr'
  exact hap.scope hfn (by rw [hs]; exact head_mem _ 0 hsc)

/-- `blockDead` does not name an object apart from the thread. -/
theorem blockDead_apart (σ : State) (ℓ' : Thread) (o : Identity) (r : ObjectRecord) (hr : σ.objects o = some r)
    (hfr : σ.frames ℓ' ≠ []) (hap : ApartObj σ ℓ' o r) : blockDead σ ℓ' o = false := by
  simp only [blockDead, hr]
  cases hf : r.frame with
  | none => rfl
  | some f =>
    have : f ≠ σ.frame ℓ' := fun heq => hap.frame f hf (by rw [heq]; exact head_mem _ 0 hfr)
    simp [this]

/-- `stmtDead` does not name an object apart from the thread. -/
theorem stmtDead_apart (σ : State) (ℓ' : Thread) (o : Identity) (r : ObjectRecord) (hr : σ.objects o = some r)
    (hsc : σ.scopes ℓ' ≠ []) (hap : ApartObj σ ℓ' o r) : stmtDead σ ℓ' o = false := by
  simp only [stmtDead, hr]
  cases hf : r.frame with
  | some f => simp
  | none =>
    have : r.scope ≠ σ.scope ℓ' := fun heq => hap.scope hf (by rw [heq]; exact head_mem _ 0 hsc)
    simp [this]

/-- `fcDead` does not name an object apart from the thread whose frames and scopes `tr` holds. -/
theorem fcDead_apart (σ : State) (ℓ' : Thread) (tr : ThreadRec) (htr : σ.threads ℓ' = some tr) (o : Identity)
    (r : ObjectRecord) (hr : σ.objects o = some r) (hap : ApartObj σ ℓ' o r) : fcDead σ tr o = false := by
  simp only [fcDead, hr]
  cases hf : r.frame with
  | some f =>
    have : f ∉ tr.frames := fun hm => hap.frame f hf (by rw [frames_of σ ℓ' tr htr]; exact hm)
    simp [this]
  | none =>
    have : r.scope ∉ tr.scopes := fun hm => hap.scope hf (by rw [scopes_of σ ℓ' tr htr]; exact hm)
    simp [this]

/-- A path's record under `endObjects` of a set not naming its object: valid as before. -/
theorem endObjects_path (σ : State) (dead : Identity → Bool) (a : Token) (p : PathRecord) (hp : σ.paths a = some p)
    (hnd : dead p.of = false) :
    (endObjects σ dead).paths a = some { p with valid := p.valid, heldBy := p.heldBy.filter (fun o => !dead o) } := by
  simp [endObjects, hp, hnd]

theorem endObjects_obj (σ : State) (dead : Identity → Bool) (o : Identity) (r : ObjectRecord) (hr : σ.objects o = some r)
    (hnd : dead o = false) : (endObjects σ dead).objects o = some r := by
  simp [endObjects, hr, hnd]

theorem writeWith_path (σ : State) (o : Identity) (refs : List Token) (a : Token) (p : PathRecord) (hp : σ.paths a = some p) :
    ∃ hb : List Identity, (writeWith σ o refs).paths a = some { p with heldBy := hb } := by
  simp [writeWith, hp]

theorem writeWith_obj (σ : State) (o : Identity) (refs : List Token) (i : Identity) : (writeWith σ o refs).objects i = σ.objects i := rfl

theorem restamp_obj (σ : State) (sel : Identity → Bool) (fr : Option Frame) (sc : Option Scope) (o : Identity) (r : ObjectRecord)
    (hr : σ.objects o = some r) :
    ∃ r' : ObjectRecord, (restamp σ sel fr sc).objects o = some r' ∧ r'.alive = r.alive ∧ r'.reclaimed = r.reclaimed ∧ r'.guard = r.guard := by
  simp only [restamp]
  by_cases h : sel o = true
  · exact ⟨{ r with frame := fr, scope := sc.getD r.scope }, by simp [h, hr], rfl, rfl, rfl⟩
  · simp only [Bool.not_eq_true] at h
    exact ⟨r, by simp [h, hr], rfl, rfl, rfl⟩

theorem running_mapThread (σ : State) (ℓ ℓ' : Thread) (f : ThreadRec → ThreadRec) (hne : ℓ' ≠ ℓ) (h : σ.running ℓ) :
    (mapThread σ ℓ' f).running ℓ := by
  obtain ⟨tr, htr, hd⟩ := h
  exact ⟨tr, by simp [mapThread, hne.symm, htr], hd⟩

theorem running_setThread (σ : State) (ℓ ℓ' : Thread) (tr' : Option ThreadRec) (hne : ℓ' ≠ ℓ) (h : σ.running ℓ) :
    (setThread σ ℓ' tr').running ℓ := by
  obtain ⟨tr, htr, hd⟩ := h
  exact ⟨tr, by simp [setThread, hne.symm, htr], hd⟩

/-- A finished thread is not running; so a running thread is not the one a `join` or
    `[Handle-Destructor]` claims. -/
theorem running_ne_done (σ : State) (ℓ ℓ' : Thread) (tr : ThreadRec) (res : List Identity)
    (hrun : σ.running ℓ) (htr : σ.threads ℓ' = some tr) (hd : tr.done = some res) : ℓ ≠ ℓ' := by
  intro heq
  subst heq
  obtain ⟨tr', htr', hd'⟩ := hrun
  rw [htr] at htr'
  cases htr'
  rw [hd] at hd'
  cases hd'

/-! ### the standing of an object -/

/-- `object_stays`: a step by another thread ℓ' leaves an object alive that is fresh storage, no guard,
    and apart from ℓ' (`ApartObj`), with its flags as they were. The cases that end objects are refuted
    by the premises the rules put on them: `[Destroy]` needs the destroying thread's authority, whose
    owner `inv.resource-authority` places in that thread's frames, scopes or result; `[Object-End]`,
    `[Relocate-In]` and `[Rawptr-Move-In]` end what the thread owns; `[Release]` and `[Rawptr-Move-Out]`
    end reclaimed objects; frame, scope and thread exits end what those frames and scopes own; a handle
    or a guard ends only by its own step. -/
theorem object_stays (σ σ' : State) (ℓ ℓ' : Thread) (o : Identity) (r : ObjectRecord)
    (hinv : σ.Invariants) (hs : StepBy ℓ' σ σ') (hne : ℓ' ≠ ℓ) (hrunℓ : σ.running ℓ)
    (hr : σ.objects o = some r) (halive : r.alive = true) (hfresh : r.reclaimed = false) (hguard : r.guard = false)
    (hap : ApartObj σ ℓ' o r) :
    ∃ r' : ObjectRecord, σ'.objects o = some r' ∧ r'.alive = true ∧ r'.reclaimed = false ∧ r'.guard = false := by
  have hf : σ.Fresh := hinv.2.2.2.1
  have hR : σ.R := hinv.2.2.2.2.1
  have hlt : o < σ.nextId := obj_lt σ hf o r hr
  have hne_next : o ≠ σ.nextId := ne_of_lt' hlt
  have hne_next1 : o ≠ σ.nextId + 1 := ne_of_lt_succ hlt
  cases hs with
  | establish _ _ r' _ _ => exact ⟨r, by simp [establish, hne_next, hr], halive, hfresh, hguard⟩
  | bind _ _ o' _ _ _ _ _ _ =>
    simp only [bind]
    by_cases h : o = o'
    · subst h; exact ⟨{ r with frame := some (σ.frame ℓ') }, by simp [hr], halive, hfresh, hguard⟩
    · exact ⟨r, by simp [h, hr], halive, hfresh, hguard⟩
  | borrow _ _ a0 p0 m pend _ _ _ _ _ _ => exact ⟨r, by simp [borrow, hr], halive, hfresh, hguard⟩
  | project _ _ a0 p0 f _ _ _ => exact ⟨r, by simp [project, hr], halive, hfresh, hguard⟩
  | lend _ _ a0 p0 _ _ _ _ _ _ _ => exact ⟨r, by simp [lend, hr], halive, hfresh, hguard⟩
  | takePath _ _ a p _ _ _ _ _ => exact ⟨r, by simp [takePath, hr], halive, hfresh, hguard⟩
  | read _ _ a p _ _ _ _ _ => exact ⟨r, hr, halive, hfresh, hguard⟩
  | write _ _ a p removed added _ _ _ _ _ _ _ => exact ⟨r, by simp [write, writeWith, hr], halive, hfresh, hguard⟩
  | tempRoot _ _ o' _ _ _ _ => exact ⟨r, by simp [tempRoot, hr], halive, hfresh, hguard⟩
  | bindParam _ _ a p _ _ _ _ _ => exact ⟨r, by simp [bindParam, hne_next, hr], halive, hfresh, hguard⟩
  | settle _ _ a p _ _ _ => exact ⟨r, by simp [settle, hr], halive, hfresh, hguard⟩
  | relocateIn _ _ oS o' path _ _ _ _ _ _ _ _ _ _ _ _ hown =>
    have hd : oS ≠ o := fun heq => by subst heq; exact owns_not_apart σ ℓ' oS r hr hown hap
    simp only [relocateIn]
    refine ⟨r, ?_, halive, hfresh, hguard⟩
    simp [endObjects, writeWith, hr, hd.symm]
  | relocateOut _ _ o' path moved _ _ _ _ _ =>
    exact ⟨r, by simp [relocateOut, writeWith, establish, hne_next, hr], halive, hfresh, hguard⟩
  | destroyComposite _ _ o' _ _ _ => exact ⟨r, by simp [destroyComposite, hr], halive, hfresh, hguard⟩
  | reclaim _ _ b _ _ => exact ⟨r, by simp [reclaim, tempRoot, setReclaimed, establish, hne_next, hr], halive, hfresh, hguard⟩
  | reattach _ _ o' r0 _ _ _ _ _ => exact ⟨r, by simp [reattach, tempRoot, rekey, hr], halive, hfresh, hguard⟩
  | release _ _ dead _ hrecl _ _ _ =>
    have hnd : dead o = false := by
      cases h : dead o with
      | false => rfl
      | true =>
        obtain ⟨r', hr', hrecl'⟩ := hrecl o h
        rw [hr] at hr'
        cases hr'
        rw [hfresh] at hrecl'
        cases hrecl'
    exact ⟨r, endObjects_obj σ dead o r hr hnd, halive, hfresh, hguard⟩
  | rawptrMoveIn _ _ oS _ _ _ _ _ _ hown =>
    have hd : oS ≠ o := fun heq => by subst heq; exact owns_not_apart σ ℓ' oS r hr hown hap
    simp only [moveWhole]
    refine ⟨r, ?_, halive, hfresh, hguard⟩
    simp [endObjects, writeWith, mwMid, establish, hr, hd.symm, hne_next]
  | rawptrMoveOut _ _ oS r0 _ _ hr0 _ hrecl0 _ _ _ =>
    have hd : oS ≠ o := fun heq => by
      subst heq
      rw [hr] at hr0
      cases hr0
      rw [hfresh] at hrecl0
      cases hrecl0
    simp only [moveWhole]
    refine ⟨r, ?_, halive, hfresh, hguard⟩
    simp [endObjects, writeWith, mwMid, establish, hr, hd.symm, hne_next]
  | destroy _ _ o' hrun' _ hob hau _ _ _ =>
    have hd : o' ≠ o := fun heq => by subst heq; exact authority_not_apart σ hR ℓ' o' r hr hfresh hob hau hrun' hap
    exact ⟨r, by simp [endObject, endObjects, hr, hd.symm], halive, hfresh, hguard⟩
  | endPlain _ _ o' _ _ _ _ _ _ hown =>
    have hd : o' ≠ o := fun heq => by subst heq; exact owns_not_apart σ ℓ' o' r hr hown hap
    exact ⟨r, by simp [endObject, endObjects, hr, hd.symm], halive, hfresh, hguard⟩
  | transfer _ _ ℓ2 a1 p1 _ _ _ _ _ _ _ _ =>
    simp only [transferTo, transfer]
    obtain ⟨r', hr', ha, hre, hg⟩ := restamp_obj (rekey (tempRoot σ ℓ2 p1.of) (fun i => decide (i = p1.of)) ℓ2) (fun i => decide (i = p1.of)) (some (σ.frame ℓ2)) none o r (by simpa [rekey, tempRoot] using hr)
    exact ⟨r', by simpa using hr', by rw [ha]; exact halive, by rw [hre]; exact hfresh, by rw [hg]; exact hguard⟩
  | blockEnter _ _ f _ => exact ⟨r, by simp [blockEnter, mapThread, hr], halive, hfresh, hguard⟩
  | blockExit _ _ _ hfr _ _ =>
    have hnd := blockDead_apart σ ℓ' o r hr hfr hap
    exact ⟨r, by simp only [blockExit, mapThread]; exact endObjects_obj σ _ o r hr hnd, halive, hfresh, hguard⟩
  | stmtEnter _ _ t _ => exact ⟨r, by simp [stmtEnter, mapThread, hr], halive, hfresh, hguard⟩
  | stmtExit _ _ _ hsc _ _ =>
    have hnd := stmtDead_apart σ ℓ' o r hr hsc hap
    exact ⟨r, by simp only [stmtExit, mapThread, invalidateUnheld]; exact endObjects_obj σ _ o r hr hnd, halive, hfresh, hguard⟩
  | spawn _ _ ℓ'' f' _ _ _ => exact ⟨r, by simp [spawn, setThread, establish, hne_next, hr], halive, hfresh, hguard⟩
  | threadDone _ _ tr f' res htr _ hfr _ _ _ _ _ =>
    have hfr' : σ.frames ℓ' ≠ [] := by rw [frames_of σ ℓ' tr htr, hfr]; simp
    have hnd := blockDead_apart σ ℓ' o r hr hfr' hap
    have hnd' : (blockDead σ ℓ' o && !decide (o ∈ res)) = false := by simp [hnd]
    simp only [threadDone, mapThread]
    obtain ⟨r', hr', ha, hre, hg⟩ := restamp_obj (endObjects σ (fun i => blockDead σ ℓ' i && !decide (i ∈ res))) (fun i => decide (i ∈ res)) none none o r (endObjects_obj σ _ o r hr hnd')
    exact ⟨r', by simpa using hr', by rw [ha]; exact halive, by rw [hre]; exact hfresh, by rw [hg]; exact hguard⟩
  | join _ _ ℓ'' tr res _ _ _ htr hd _ _ _ _ _ =>
    have hnh : (decide (o = tr.handle)) = false := by simp; exact fun h => hap.handle ℓ'' tr htr h.symm
    simp only [join, setThread, rekey]
    obtain ⟨r', hr', ha, hre, hg⟩ := restamp_obj (endObjects σ (fun i => decide (i = tr.handle))) (fun i => decide (i ∈ res)) none (some (σ.scope ℓ')) o r (endObjects_obj σ _ o r hr hnh)
    exact ⟨r', by simpa using hr', by rw [ha]; exact halive, by rw [hre]; exact hfresh, by rw [hg]; exact hguard⟩
  | handleDestroy _ _ ℓ'' tr res _ htr hd _ _ _ _ _ _ =>
    have hnd : (decide (o = tr.handle) || decide (o ∈ res)) = false := by
      simp
      exact ⟨fun h => hap.handle ℓ'' tr htr h.symm, hap.result ℓ'' tr res htr hd⟩
    exact ⟨r, by simp only [handleDestroy, setThread]; exact endObjects_obj σ _ o r hr hnd, halive, hfresh, hguard⟩
  | faultContain _ _ tr htr _ _ _ _ =>
    have hnd := fcDead_apart σ ℓ' tr htr o r hr hap
    exact ⟨r, by simp only [faultContain, mapThread, fcMid]; exact endObjects_obj σ _ o r hr hnd, halive, hfresh, hguard⟩
  | lock _ _ am pm _ _ _ _ _ => exact ⟨r, by simp [lock, hne_next1, hr], halive, hfresh, hguard⟩
  | guardDrop _ _ g rg ag pg _ hg hga hgg _ _ _ _ _ _ _ =>
    have hd : g ≠ o := fun heq => by
      subst heq
      rw [hr] at hg
      cases hg
      rw [hguard] at hgg
      cases hgg
    exact ⟨r, by simp [guardDrop, endObjects, hr, hd.symm], halive, hfresh, hguard⟩

/-! ### the standing of a path -/

/-- `path_stays`: a step by another thread ℓ' leaves ℓ's standing path (`Standing`) as it was but for its
    holders: the same object, projection, mode, forming thread, scope; valid; not lock-derived; not pending.
    The steps that invalidate paths are refuted or miss it: `[Thread-Affine]`'s taking needs the path in
    transit or its thread finished (ℓ runs); a pending path's binding or settling needs it pending; a lock
    path's end needs it lock-derived; `[Authority-Transfer]` from it needs ℓ'`s authority over its object,
    which `inv.resource-authority` puts in ℓ'`s frames (`ApartObj`); a scope's exit ends the unheld paths
    of that scope (`Apart.scope`); and an object's end is `object_stays`. -/
theorem path_stays (σ σ' : State) (ℓ ℓ' : Thread) (a : Token) (p : PathRecord) (r : ObjectRecord)
    (hinv : σ.Invariants) (hs : StepBy ℓ' σ σ') (hne : ℓ' ≠ ℓ) (hst : Standing σ ℓ a p r)
    (hap : Apart σ ℓ' p.of r p) :
    ∃ p' : PathRecord, σ'.paths a = some p' ∧ p'.valid = true ∧ p'.of = p.of ∧ p'.proj = p.proj ∧ p'.mode = p.mode ∧
      p'.thread = p.thread ∧ p'.lock = false ∧ p'.pending = false ∧ p'.scope = p.scope ∧ p'.frame = p.frame := by
  have hf : σ.Fresh := hinv.2.2.2.1
  have hR : σ.R := hinv.2.2.2.2.1
  have hp := hst.path
  have hlt : a < σ.nextId := path_lt σ hf a p hp
  have hne_next : a ≠ σ.nextId := ne_of_lt' hlt
  have hne_next1 : a ≠ σ.nextId + 1 := ne_of_lt_succ hlt
  -- the record with other holders: the fields that matter kept
  have fin : ∀ (σ'' : State) (hb : List Identity), σ''.paths a = some { p with heldBy := hb } →
      ∃ p' : PathRecord, σ''.paths a = some p' ∧ p'.valid = true ∧ p'.of = p.of ∧ p'.proj = p.proj ∧ p'.mode = p.mode ∧
        p'.thread = p.thread ∧ p'.lock = false ∧ p'.pending = false ∧ p'.scope = p.scope ∧ p'.frame = p.frame :=
    fun σ'' hb h => ⟨_, h, hst.valid, rfl, rfl, rfl, rfl, hst.lock, hst.pending, rfl, rfl⟩
  have same : ∀ (σ'' : State), σ''.paths a = some p →
      ∃ p' : PathRecord, σ''.paths a = some p' ∧ p'.valid = true ∧ p'.of = p.of ∧ p'.proj = p.proj ∧ p'.mode = p.mode ∧
        p'.thread = p.thread ∧ p'.lock = false ∧ p'.pending = false ∧ p'.scope = p.scope ∧ p'.frame = p.frame :=
    fun σ'' h => fin σ'' p.heldBy h
  -- an object's end not naming the path's object
  have finEnd : ∀ (σ0 : State) (dead : Identity → Bool) (hb : List Identity), σ0.paths a = some { p with heldBy := hb } →
      dead p.of = false →
      ∃ p' : PathRecord, (endObjects σ0 dead).paths a = some p' ∧ p'.valid = true ∧ p'.of = p.of ∧ p'.proj = p.proj ∧ p'.mode = p.mode ∧
        p'.thread = p.thread ∧ p'.lock = false ∧ p'.pending = false ∧ p'.scope = p.scope ∧ p'.frame = p.frame := by
    intro σ0 dead hb h0 hnd
    exact ⟨{ p with heldBy := hb.filter (fun o => !dead o) }, by simp [endObjects, h0, hnd], hst.valid, rfl, rfl, rfl, rfl, hst.lock, hst.pending, rfl, rfl⟩
  have viaEnd : ∀ (σ0 : State) (dead : Identity → Bool), σ0.paths a = some p → dead p.of = false →
      ∃ p' : PathRecord, (endObjects σ0 dead).paths a = some p' ∧ p'.valid = true ∧ p'.of = p.of ∧ p'.proj = p.proj ∧ p'.mode = p.mode ∧
        p'.thread = p.thread ∧ p'.lock = false ∧ p'.pending = false ∧ p'.scope = p.scope ∧ p'.frame = p.frame :=
    fun σ0 dead h0 hnd => finEnd σ0 dead p.heldBy h0 hnd
  cases hs with
  | establish _ _ b _ _ => exact same _ (by simp [establish, hp])
  | bind _ _ o' _ _ _ _ _ _ => exact same _ (by simp [bind, hne_next, hp])
  | borrow _ _ a0 p0 m pend _ _ _ _ _ _ => exact same _ (by simp [borrow, hne_next, hp])
  | project _ _ a0 p0 f _ _ _ => exact same _ (by simp [project, hne_next, hp])
  | lend _ _ a0 p0 _ _ _ _ _ _ _ => exact same _ (by simp [lend, hne_next, hp])
  | takePath _ _ a' p' _ hp' _ _ haff =>
    by_cases h : a' = a
    · subst h
      rw [hp] at hp'
      cases hp'
      exfalso
      rcases haff with h1 | h2 | ⟨ℓ'', h3, h4⟩
      · rw [hst.thread] at h1; exact hne (Option.some.inj h1).symm
      · rw [hst.thread] at h2; cases h2
      · rw [hst.thread] at h3; cases h3; exact h4 hst.running
    · exact same _ (by simp [takePath, Ne.symm h, hp])
  | read _ _ a' p' _ _ _ _ _ => exact same _ hp
  | write _ _ a' p' removed added _ _ _ _ _ _ _ =>
    obtain ⟨hb, h⟩ := writeWith_path σ p'.of _ a p hp
    exact fin _ hb h
  | tempRoot _ _ o' _ _ _ _ => exact same _ (by simp [tempRoot, hne_next, hp])
  | bindParam _ _ a' p' _ _ hp' _ hpend' =>
    by_cases h : a' = a
    · subst h; rw [hp] at hp'; cases hp'; rw [hst.pending] at hpend'; cases hpend'
    · exact same _ (by simp [bindParam, hne_next1, Ne.symm h, hp])
  | settle _ _ a' p' _ hp' hpend' =>
    by_cases h : a' = a
    · subst h; rw [hp] at hp'; cases hp'; rw [hst.pending] at hpend'; cases hpend'
    · exact same _ (by simp [settle, Ne.symm h, hp])
  | relocateIn _ _ oS o' path _ _ _ _ _ _ _ _ _ _ _ _ hown =>
    have hd : oS ≠ p.of := fun heq => by rw [heq] at hown; exact owns_not_apart σ ℓ' p.of r hst.obj hown hap.obj
    obtain ⟨hb, h⟩ := writeWith_path σ o' (σ.contents o' ++ σ.contents oS) a p hp
    exact finEnd { writeWith σ o' (σ.contents o' ++ σ.contents oS) with subs := relocInSubs σ oS o' path } (fun i => decide (i = oS)) hb h (decide_eq_false (Ne.symm hd))
  | relocateOut _ _ o' path moved _ _ _ _ _ =>
    obtain ⟨hb1, h1⟩ := writeWith_path (establish σ ℓ' true) o' ((σ.contents o').filter (· ∉ moved)) a p hp
    obtain ⟨hb2, h2⟩ := writeWith_path (writeWith (establish σ ℓ' true) o' ((σ.contents o').filter (· ∉ moved))) σ.nextId moved a { p with heldBy := hb1 } h1
    exact fin { writeWith (writeWith (establish σ ℓ' true) o' ((σ.contents o').filter (· ∉ moved))) σ.nextId moved with subs := relocOutSubs σ o' path } hb2 h2
  | destroyComposite _ _ o' _ _ _ => exact same _ (by simp [destroyComposite, hp])
  | reclaim _ _ b _ _ => exact same _ (by simp [reclaim, tempRoot, setReclaimed, establish, hne_next, hne_next1, hp])
  | reattach _ _ o' r0 _ _ _ _ _ => exact same _ (by simp [reattach, tempRoot, rekey, hne_next, hp])
  | release _ _ dead _ hrecl _ _ _ =>
    have hnd : dead p.of = false := by
      cases h : dead p.of with
      | false => rfl
      | true =>
        obtain ⟨r', hr', hrecl'⟩ := hrecl p.of h
        rw [hst.obj] at hr'
        cases hr'
        rw [hst.fresh] at hrecl'
        cases hrecl'
    exact viaEnd σ dead hp hnd
  | rawptrMoveIn _ _ oS _ _ _ _ _ _ hown =>
    have hd : oS ≠ p.of := fun heq => by rw [heq] at hown; exact owns_not_apart σ ℓ' p.of r hst.obj hown hap.obj
    obtain ⟨hb, h⟩ := writeWith_path (mwMid σ ℓ' oS true) σ.nextId (σ.contents oS) a p hp
    exact finEnd _ (fun i => decide (i = oS)) hb h (decide_eq_false (Ne.symm hd))
  | rawptrMoveOut _ _ oS r0 _ _ hr0 _ hrecl0 _ _ _ =>
    have hd : oS ≠ p.of := fun heq => by
      subst heq
      rw [hst.obj] at hr0
      cases hr0
      rw [hst.fresh] at hrecl0
      cases hrecl0
    obtain ⟨hb, h⟩ := writeWith_path (mwMid σ ℓ' oS false) σ.nextId (σ.contents oS) a p hp
    exact finEnd _ (fun i => decide (i = oS)) hb h (decide_eq_false (Ne.symm hd))
  | destroy _ _ o' hrun' _ hob hau _ _ _ =>
    have hd : o' ≠ p.of := fun heq => by rw [heq] at hob hau; exact authority_not_apart σ hR ℓ' p.of r hst.obj hst.fresh hob hau hrun' hap.obj
    exact viaEnd σ _ hp (decide_eq_false (Ne.symm hd))
  | endPlain _ _ o' _ _ _ _ _ _ hown =>
    have hd : o' ≠ p.of := fun heq => by rw [heq] at hown; exact owns_not_apart σ ℓ' p.of r hst.obj hown hap.obj
    exact viaEnd σ _ hp (decide_eq_false (Ne.symm hd))
  | transfer _ _ ℓ2 a1 p1 hrun' hp1 _ hau hob _ _ _ =>
    by_cases h : a1 = a
    · subst h
      rw [hp] at hp1
      cases hp1
      exact absurd hap.obj (fun hap' => authority_not_apart σ hR ℓ' p.of r hst.obj hst.fresh hob hau hrun' hap')
    · exact same _ (by simp [transferTo, transfer, restamp, rekey, tempRoot, Ne.symm h, hne_next, hp])
  | blockEnter _ _ f _ => exact same _ (by simp [blockEnter, mapThread, hp])
  | blockExit _ _ _ hfr _ _ =>
    have hnd := blockDead_apart σ ℓ' p.of r hst.obj hfr hap.obj
    simp only [blockExit, mapThread]
    exact viaEnd σ _ hp hnd
  | stmtEnter _ _ t _ => exact same _ (by simp [stmtEnter, mapThread, hp])
  | stmtExit _ _ _ hsc _ _ =>
    have hnd := stmtDead_apart σ ℓ' p.of r hst.obj hsc hap.obj
    have hps : p.scope ≠ σ.scope ℓ' := fun heq => hap.scope (by rw [heq]; exact head_mem _ 0 hsc)
    simp only [stmtExit, mapThread, invalidateUnheld]
    obtain ⟨p', h1, h2, h3, h4, h5, h6, h7, h8, h9, h10⟩ := viaEnd σ _ hp hnd
    refine ⟨{ p' with valid := if p'.scope = σ.scope ℓ' ∧ p'.heldBy = [] ∧ (endObjects σ (stmtDead σ ℓ')).holder p'.of ≠ some a then false else p'.valid }, by simp [h1], ?_, h3, h4, h5, h6, h7, h8, h9, h10⟩
    rw [h9]
    simp [hps, h2]
  | spawn _ _ ℓ'' f' _ _ _ => exact same _ (by simp [spawn, setThread, establish, hp])
  | threadDone _ _ tr f' res htr _ hfr _ _ _ _ _ =>
    have hfr' : σ.frames ℓ' ≠ [] := by rw [frames_of σ ℓ' tr htr, hfr]; simp
    have hnd := blockDead_apart σ ℓ' p.of r hst.obj hfr' hap.obj
    have hnd' : (blockDead σ ℓ' p.of && !decide (p.of ∈ res)) = false := by simp [hnd]
    simp only [threadDone, mapThread, restamp]
    exact viaEnd σ _ hp hnd'
  | join _ _ ℓ'' tr res _ _ _ htr hd _ _ _ _ _ =>
    have hnh : (decide (p.of = tr.handle)) = false := by simp; exact fun h => hap.obj.handle ℓ'' tr htr h.symm
    simp only [join, setThread, rekey, restamp]
    exact viaEnd σ _ hp hnh
  | handleDestroy _ _ ℓ'' tr res _ htr hd _ _ _ _ _ _ =>
    have hnd : (decide (p.of = tr.handle) || decide (p.of ∈ res)) = false := by
      simp
      exact ⟨fun h => hap.obj.handle ℓ'' tr htr h.symm, hap.obj.result ℓ'' tr res htr hd⟩
    simp only [handleDestroy, setThread]
    exact viaEnd σ _ hp hnd
  | faultContain _ _ tr htr _ _ _ _ =>
    have hnd := fcDead_apart σ ℓ' tr htr p.of r hst.obj hap.obj
    simp only [faultContain, mapThread, fcMid]
    obtain ⟨p', h1, h2, h3, h4, h5, h6, h7, h8, h9, h10⟩ := viaEnd σ _ hp hnd
    refine ⟨{ p' with valid := if p'.lock = true ∧ p'.thread = some ℓ' then false else p'.valid }, by simp [h1], ?_, h3, h4, h5, h6, h7, h8, h9, h10⟩
    simp [h7, h2]
  | lock _ _ am pm _ _ _ _ _ => exact same _ (by simp [lock, hne_next, hp])
  | guardDrop _ _ g rg ag pg _ hg _ hgg _ _ _ hpg hlg _ _ =>
    have hd : g ≠ p.of := fun heq => by
      subst heq
      rw [hst.obj] at hg
      cases hg
      rw [hst.guard] at hgg
      cases hgg
    have hag : ag ≠ a := fun heq => by
      subst heq
      rw [hp] at hpg
      cases hpg
      rw [hst.lock] at hlg
      cases hlg
    simp only [guardDrop]
    obtain ⟨p', h1, h2, h3, h4, h5, h6, h7, h8, h9, h10⟩ := viaEnd σ (fun i => decide (i = g)) hp (decide_eq_false (Ne.symm hd))
    exact ⟨p', by simp [Ne.symm hag, h1], h2, h3, h4, h5, h6, h7, h8, h9, h10⟩


/-- A step by another thread leaves ℓ running: a thread's record changes only by its own steps, by its
    spawn (a fresh record), and by the `[Join]` or `[Handle-Destructor]` of a finished thread. -/
theorem running_stays (σ σ' : State) (ℓ ℓ' : Thread) (hs : StepBy ℓ' σ σ') (hne : ℓ' ≠ ℓ) (h : σ.running ℓ) :
    σ'.running ℓ := by
  cases hs with
  | establish _ _ r _ _ => exact h
  | bind _ _ o _ _ _ _ _ _ => exact h
  | borrow _ _ a0 p0 m pend _ _ _ _ _ _ => exact h
  | project _ _ a0 p0 f _ _ _ => exact h
  | lend _ _ a0 p0 _ _ _ _ _ _ _ => exact h
  | takePath _ _ a p _ _ _ _ _ => exact h
  | read _ _ a p _ _ _ _ _ => exact h
  | write _ _ a p removed added _ _ _ _ _ _ _ => exact h
  | tempRoot _ _ o _ _ _ _ => exact h
  | bindParam _ _ a p _ _ _ _ _ => exact h
  | settle _ _ a p _ _ _ => exact h
  | relocateIn _ _ oS o path _ _ _ _ _ _ _ _ _ _ _ _ _ => exact h
  | relocateOut _ _ o path moved _ _ _ _ _ => exact h
  | destroyComposite _ _ o _ _ _ => exact h
  | reclaim _ _ b _ _ => exact h
  | reattach _ _ o r _ _ _ _ _ => exact h
  | release _ _ dead _ _ _ _ _ => exact h
  | rawptrMoveIn _ _ oS _ _ _ _ _ _ _ => exact h
  | rawptrMoveOut _ _ oS r _ _ _ _ _ _ _ _ => exact h
  | destroy _ _ o _ _ _ _ _ _ _ => exact h
  | endPlain _ _ o _ _ _ _ _ _ _ => exact h
  | transfer _ _ ℓ2 a1 p1 _ _ _ _ _ _ _ _ => exact h
  | blockEnter _ _ f _ => exact running_mapThread σ ℓ ℓ' _ hne h
  | blockExit _ _ _ _ _ _ => exact running_mapThread _ ℓ ℓ' _ hne h
  | stmtEnter _ _ t _ => exact running_mapThread σ ℓ ℓ' _ hne h
  | stmtExit _ _ _ _ _ _ => exact running_mapThread _ ℓ ℓ' _ hne h
  | spawn _ _ ℓ'' f' _ _ hnone =>
    have hne'' : ℓ'' ≠ ℓ := fun heq => by
      subst heq
      obtain ⟨tr, htr, _⟩ := h
      rw [hnone] at htr
      cases htr
    exact running_setThread _ ℓ ℓ'' _ hne'' h
  | threadDone _ _ tr f' res _ _ _ _ _ _ _ _ => exact running_mapThread _ ℓ ℓ' _ hne h
  | join _ _ ℓ'' tr res _ _ _ htr hd _ _ _ _ _ =>
    exact running_setThread _ ℓ ℓ'' none (running_ne_done σ ℓ ℓ'' tr res h htr hd).symm h
  | handleDestroy _ _ ℓ'' tr res _ htr hd _ _ _ _ _ _ =>
    exact running_setThread _ ℓ ℓ'' none (running_ne_done σ ℓ ℓ'' tr res h htr hd).symm h
  | faultContain _ _ tr _ _ _ _ _ => exact running_mapThread _ ℓ ℓ' _ hne h
  | lock _ _ am pm _ _ _ _ _ => exact h
  | guardDrop _ _ g r ag p _ _ _ _ _ _ _ _ _ _ _ => exact h

/-! ### `[Standing-Local]` -/

/-- `[Standing-Local]` for one step: a standing path of ℓ (`Standing`), whose object and scope another
    thread ℓ' leaves alone (`Apart`), stands after any step of ℓ': the same object, projection, mode,
    scope and frame, valid, ℓ's, not lock-derived, not pending, its object alive and as it was. -/
theorem standing_step (σ σ' : State) (ℓ ℓ' : Thread) (a : Token) (p : PathRecord) (r : ObjectRecord)
    (hinv : σ.Invariants) (hs : StepBy ℓ' σ σ') (hne : ℓ' ≠ ℓ) (hst : Standing σ ℓ a p r)
    (hap : Apart σ ℓ' p.of r p) :
    ∃ (p' : PathRecord) (r' : ObjectRecord), Standing σ' ℓ a p' r' ∧ p'.of = p.of ∧ p'.proj = p.proj ∧ p'.mode = p.mode ∧
      p'.scope = p.scope ∧ p'.frame = p.frame := by
  obtain ⟨p', hp', hv, hof, hproj, hmode, hth, hl, hpend, hsc, hfr⟩ := path_stays σ σ' ℓ ℓ' a p r hinv hs hne hst hap
  obtain ⟨r', hr', ha, hre, hg⟩ := object_stays σ σ' ℓ ℓ' p.of r hinv hs hne hst.running hst.obj hst.alive hst.fresh hst.guard hap.obj
  exact ⟨p', r', ⟨hp', by rw [hth]; exact hst.thread, hv, hl, hpend, by rw [hof]; exact hr', ha, hre, hg,
    running_stays σ σ' ℓ ℓ' hs hne hst.running⟩, hof, hproj, hmode, hsc, hfr⟩

/-- A run of steps by threads other than ℓ, each leaving the path `a`'s object and scope alone where it is
    taken (H6 holding throughout the run). -/
inductive OthersRun (ℓ : Thread) (a : Token) : State → State → Prop where
  | refl (σ : State) : OthersRun ℓ a σ σ
  | step (σ σ1 σ2 : State) (ℓ' : Thread) (p : PathRecord) (r : ObjectRecord) :
      ℓ' ≠ ℓ → StepBy ℓ' σ σ1 → σ.paths a = some p → σ.objects p.of = some r → Apart σ ℓ' p.of r p →
      OthersRun ℓ a σ1 σ2 → OthersRun ℓ a σ σ2

/-- `[Standing-Local]` over a run: a path standing in a reachable state stands at the run's end, over the
    same object with the same projection and mode. So an implementation that tested the path where it
    arrived may omit the use-time test while ℓ holds it and the other threads leave its object alone. -/
theorem standing_run (σ σ' : State) (ℓ : Thread) (a : Token) (hrun : OthersRun ℓ a σ σ') :
    ∀ (p : PathRecord) (r : ObjectRecord), Reachable σ → Standing σ ℓ a p r →
      ∃ (p' : PathRecord) (r' : ObjectRecord), Standing σ' ℓ a p' r' ∧ p'.of = p.of ∧ p'.proj = p.proj ∧ p'.mode = p.mode := by
  induction hrun with
  | refl σ => intro p r _ hst; exact ⟨p, r, hst, rfl, rfl, rfl⟩
  | step σ σ1 σ2 ℓ' p0 r0 hne hs hp0 hr0 hap _ ih =>
    intro p r hreach hst
    have hpp : p0 = p := by rw [hst.path] at hp0; exact (Option.some.inj hp0).symm
    subst hpp
    have hrr : r0 = r := by rw [hst.obj] at hr0; exact (Option.some.inj hr0).symm
    subst hrr
    obtain ⟨p', r', hst', hof, hproj, hmode, _, _⟩ := standing_step σ σ1 ℓ ℓ' a p0 r0 (reachable_invariants σ hreach) hs hne hst hap
    obtain ⟨p'', r'', hst'', hof', hproj', hmode'⟩ := ih p' r' (Reachable.step σ σ1 hreach hs.toStep) hst'
    exact ⟨p'', r'', hst'', hof'.trans hof, hproj'.trans hproj, hmode'.trans hmode⟩

/-! ### every conflict is diagnosed at the other access -/

/-- A standing path is a `clash` witness for every other path of its object that overlaps it in a mode
    not jointly permitted (and is not its descendant): `spec/08` §4's closing sentence — the elided
    path's conflicts are diagnosed at the other access, whose own premise refuses it. -/
theorem conflict_diagnosed (σ : State) (ℓ : Thread) (a a' : Token) (p p' : PathRecord) (r : ObjectRecord) (m' : Mode)
    (hst : Standing σ ℓ a p r) (hp' : σ.paths a' = some p') (hlock' : p'.lock = false) (hne : a ≠ a') (hof : p'.of = p.of)
    (hov : overlap p'.proj p.proj) (hanc : a ∉ ancestors σ σ.nextId a') (hperm : ¬ permitted m' p.mode) :
    σ.clash a' m' :=
  ⟨a, p', p, hp', hst.path, hne, hst.valid, hst.pending, hlock', hst.lock, hof.symm, hov, hanc, hperm⟩

/-- A `write` through a path overlapping a standing path is refused: `[Write]`'s `¬clash` premise fails. -/
theorem write_conflict_diagnosed (σ : State) (ℓ : Thread) (a a' : Token) (p p' : PathRecord) (r : ObjectRecord)
    (hst : Standing σ ℓ a p r) (hp' : σ.paths a' = some p') (hlock' : p'.lock = false) (hne : a ≠ a') (hof : p'.of = p.of)
    (hov : overlap p'.proj p.proj) (hanc : a ∉ ancestors σ σ.nextId a') : σ.clash a' Mode.exclusive :=
  conflict_diagnosed σ ℓ a a' p p' r Mode.exclusive hst hp' hlock' hne hof hov hanc (fun h => Mode.noConfusion h.1)

/-- A `read` through a path overlapping a standing exclusive path is refused: `[Read]`'s `¬clash` fails. -/
theorem read_conflict_diagnosed (σ : State) (ℓ : Thread) (a a' : Token) (p p' : PathRecord) (r : ObjectRecord)
    (hst : Standing σ ℓ a p r) (hexcl : p.mode = Mode.exclusive) (hp' : σ.paths a' = some p') (hlock' : p'.lock = false) (hne : a ≠ a')
    (hof : p'.of = p.of) (hov : overlap p'.proj p.proj) (hanc : a ∉ ancestors σ σ.nextId a') : σ.clash a' Mode.shared :=
  conflict_diagnosed σ ℓ a a' p p' r Mode.shared hst hp' hlock' hne hof hov hanc (fun h => by rw [hexcl] at h; exact Mode.noConfusion h.2)

/-! ### an object no path reaches -/

/-- D-0200: an object no valid path reaches (an objectless local of ℓ, whose uses lend it with no path
    minted) is reached by no other thread's step: afterwards no valid path of it exists either, and it is
    alive still. A new path is formed from a valid path of the object (`[Borrow]`, projections,
    `[Thread-Lend]`, `[Lock]`), as the root of a temporary of the thread's own scope (`[Binding-Form]`,
    `[Temp-Root]`), of a fresh object, or of a reclaimed one. So such a local needs no runtime record for
    the other threads' sake: nothing of theirs can touch it. -/
theorem unreached_stays (σ σ' : State) (ℓ ℓ' : Thread) (o : Identity) (r : ObjectRecord)
    (hinv : σ.Invariants) (hs : StepBy ℓ' σ σ') (hne : ℓ' ≠ ℓ) (hrunℓ : σ.running ℓ)
    (hr : σ.objects o = some r) (halive : r.alive = true) (hfresh : r.reclaimed = false) (hguard : r.guard = false)
    (hap : ApartObj σ ℓ' o r)
    (hnone : ∀ a p, σ.paths a = some p → p.of = o → p.valid = false) :
    (∀ a p, σ'.paths a = some p → p.of = o → p.valid = false) ∧
      ∃ r' : ObjectRecord, σ'.objects o = some r' ∧ r'.alive = true := by
  refine ⟨?_, ?_⟩
  · have hf : σ.Fresh := hinv.2.2.2.1
    have hlt : o < σ.nextId := obj_lt σ hf o r hr
    -- a path of a valid path's object is not of `o`
    have ofValid : ∀ a0 p0, σ.paths a0 = some p0 → σ.temporallyValid a0 → p0.of ≠ o := by
      intro a0 p0 hp0 htv heq
      obtain ⟨q, hq, hqv, _⟩ := htv
      rw [hp0] at hq
      cases hq
      rw [hnone a0 p0 hp0 heq] at hqv
      cases hqv
    -- a record mapped keeps `of` and never turns valid
    have mapped : ∀ (f : PathRecord → PathRecord) (a : Token) (p : PathRecord), (σ.paths a).map f = some p →
        (∀ q, (f q).of = q.of) → (∀ q, q.valid = false → (f q).valid = false) → p.of = o → p.valid = false := by
      intro f a p hp hof hv hpo
      cases hq : σ.paths a with
      | none => rw [hq] at hp; cases hp
      | some q =>
        rw [hq] at hp
        simp at hp
        subst hp
        exact hv q (hnone a q hq (by rw [← hof q]; exact hpo))
    have mapped' : ∀ (f : PathRecord → PathRecord) (a : Token) (p : PathRecord), (σ.paths a).map f = some p →
        (∀ q, (f q).of = q.of) → (∀ q, (f q).valid = false) → p.of = o → p.valid = false :=
      fun f a p hp hof hv hpo => mapped f a p hp hof (fun q _ => hv q) hpo
    intro a p hp hpo
    cases hs with
    | establish _ _ b _ _ => exact hnone a p hp hpo
    | bind _ _ o' _ _ _ _ hsc ht =>
      simp only [bind] at hp
      split at hp
      · cases hp
        have hpo' : o' = o := hpo
        rw [hpo'] at ht
        exact (tempOf_not_apart σ ℓ' o r hr hsc ht hap).elim
      · exact hnone a p hp hpo
    | borrow _ _ a0 p0 m pend _ hp0 htv _ _ _ =>
      simp only [borrow] at hp
      split at hp
      · cases hp; exact absurd hpo (ofValid a0 p0 hp0 htv)
      · exact hnone a p hp hpo
    | project _ _ a0 p0 f _ hp0 htv =>
      simp only [project] at hp
      split at hp
      · cases hp; exact absurd hpo (ofValid a0 p0 hp0 htv)
      · exact hnone a p hp hpo
    | lend _ _ a0 p0 _ hp0 htv _ _ _ _ =>
      simp only [lend] at hp
      split at hp
      · cases hp; exact absurd hpo (ofValid a0 p0 hp0 htv)
      · exact hnone a p hp hpo
    | takePath _ _ a' p' _ _ _ _ _ =>
      simp only [takePath] at hp
      split at hp
      · rename_i h; subst h; exact mapped _ a p hp (fun _ => rfl) (fun _ hv => hv) hpo
      · exact hnone a p hp hpo
    | read _ _ a' p' _ _ _ _ _ => exact hnone a p hp hpo
    | write _ _ a' p' removed added _ _ _ _ _ _ _ =>
      simp only [write, writeWith] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => hv) hpo
    | tempRoot _ _ o' _ _ hsc ht =>
      simp only [tempRoot] at hp
      split at hp
      · cases hp
        have hpo' : o' = o := hpo
        rw [hpo'] at ht
        exact (tempOf_not_apart σ ℓ' o r hr hsc ht hap).elim
      · exact hnone a p hp hpo
    | bindParam _ _ a' p' _ _ _ _ _ =>
      simp only [bindParam] at hp
      split at hp
      · cases hp; exact absurd hpo (Nat.ne_of_gt hlt)
      · split at hp
        · rename_i _ h; subst h; exact mapped _ a p hp (fun _ => rfl) (fun _ hv => hv) hpo
        · exact hnone a p hp hpo
    | settle _ _ a' p' _ _ _ =>
      simp only [settle] at hp
      split at hp
      · rename_i h; subst h; exact mapped' _ a p hp (fun _ => rfl) (fun _ => rfl) hpo
      · exact hnone a p hp hpo
    | relocateIn _ _ oS o' path _ _ _ _ _ _ _ _ _ _ _ _ _ =>
      simp only [relocateIn, endObjects, writeWith, Option.map_map] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [Function.comp, hv]) hpo
    | relocateOut _ _ o' path moved _ _ _ _ _ =>
      simp only [relocateOut, writeWith, establish, Option.map_map] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [Function.comp, hv]) hpo
    | destroyComposite _ _ o' _ _ _ => exact hnone a p hp hpo
    | reclaim _ _ b _ _ =>
      rw [reclaim, tempRoot_paths] at hp
      split at hp
      · cases hp; exact absurd (hpo : σ.nextId = o) (Nat.ne_of_gt hlt)
      · exact hnone a p hp hpo
    | reattach _ _ o' r0 _ hr0 _ hrecl _ =>
      rw [reattach, tempRoot_paths] at hp
      split at hp
      · cases hp
        have hpo' : o' = o := hpo
        rw [hpo', hr] at hr0
        cases hr0
        rw [hfresh] at hrecl
        cases hrecl
      · exact hnone a p hp hpo
    | release _ _ dead _ _ _ _ _ =>
      simp only [endObjects] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [hv]) hpo
    | rawptrMoveIn _ _ oS _ _ _ _ _ _ _ =>
      simp only [moveWhole, endObjects, writeWith, mwMid, establish, Option.map_map] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [Function.comp, hv]) hpo
    | rawptrMoveOut _ _ oS r0 _ _ _ _ _ _ _ _ =>
      simp only [moveWhole, endObjects, writeWith, mwMid, establish, Option.map_map] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [Function.comp, hv]) hpo
    | destroy _ _ o' _ _ _ _ _ _ _ =>
      simp only [endObject, endObjects] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [hv]) hpo
    | endPlain _ _ o' _ _ _ _ _ _ _ =>
      simp only [endObject, endObjects] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [hv]) hpo
    | transfer _ _ ℓ2 a1 p1 _ hp1 htv1 _ _ _ _ _ =>
      simp only [transferTo, transfer, restamp, rekey, tempRoot] at hp
      split at hp
      · rename_i h; subst h
        split at hp
        · simp only [Option.map] at hp
          cases hp
          exact absurd (hpo : p1.of = o) (ofValid a p1 hp1 htv1)
        · exact mapped' _ a p hp (fun q => by simp) (fun q => by simp) hpo
      · split at hp
        · cases hp; exact absurd (hpo : p1.of = o) (ofValid a1 p1 hp1 htv1)
        · exact hnone a p hp hpo
    | blockEnter _ _ f _ => exact hnone a p hp hpo
    | blockExit _ _ _ _ _ _ =>
      simp only [blockExit, mapThread, endObjects] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [hv]) hpo
    | stmtEnter _ _ t _ => exact hnone a p hp hpo
    | stmtExit _ _ _ _ _ _ =>
      simp only [stmtExit, mapThread, invalidateUnheld, endObjects, Option.map_map] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [Function.comp, hv]) hpo
    | spawn _ _ ℓ'' f' _ _ _ => exact hnone a p hp hpo
    | threadDone _ _ tr f' res _ _ _ _ _ _ _ _ =>
      simp only [threadDone, mapThread, restamp, endObjects] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [hv]) hpo
    | join _ _ ℓ'' tr res _ _ _ _ _ _ _ _ _ _ =>
      simp only [join, setThread, rekey, restamp, endObjects] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [hv]) hpo
    | handleDestroy _ _ ℓ'' tr res _ _ _ _ _ _ _ _ _ =>
      simp only [handleDestroy, setThread, endObjects] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [hv]) hpo
    | faultContain _ _ tr _ _ _ _ _ =>
      simp only [faultContain, mapThread, fcMid, endObjects, Option.map_map] at hp
      exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [Function.comp, hv]) hpo
    | lock _ _ am pm _ _ hpm htv _ =>
      simp only [lock] at hp
      split at hp
      · cases hp; exact absurd hpo (ofValid am pm hpm htv)
      · exact hnone a p hp hpo
    | guardDrop _ _ g rg ag pg _ _ _ _ _ _ _ _ _ _ _ =>
      simp only [guardDrop, endObjects, Option.map_map] at hp
      split at hp
      · exact mapped' _ a p hp (fun _ => rfl) (fun _ => rfl) hpo
      · exact mapped _ a p hp (fun _ => rfl) (fun _ hv => by simp [hv]) hpo
  · obtain ⟨r', hr', ha, _, _⟩ := object_stays σ σ' ℓ ℓ' o r hinv hs hne hrunℓ hr halive hfresh hguard hap
    exact ⟨r', hr', ha⟩

end Cobalt
