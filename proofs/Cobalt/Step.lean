import Cobalt.State
import Cobalt.Invariants
/-
  The fragment's step relation: the state-writing rules of spec/05, 07, 08,
  14 and 19 that the fragment reaches, each as a constructor with the
  rule's premises, performed by a running thread ℓ.
-/
namespace Cobalt
open State

/-! ### the component updates the steps are built from -/

/-- The records `sel` names re-stamped: owner frame `fr`, scope `sc` (kept when `none`). -/
def restamp (σ : State) (sel : Identity → Bool) (fr : Option Frame) (sc : Option Scope) : State :=
  { σ with objects := fun i => if sel i then (σ.objects i).map (fun r => { r with frame := fr, scope := sc.getD r.scope }) else σ.objects i }

/-- The authority of the objects `sel` names re-keyed to `ℓ` (the old token consumed, the new one created). -/
def rekey (σ : State) (sel : Identity → Bool) (ℓ : Thread) : State :=
  { σ with authority := fun i => if sel i then some ℓ else σ.authority i }

def setThread (σ : State) (ℓ : Thread) (tr : Option ThreadRec) : State :=
  { σ with threads := fun l => if l = ℓ then tr else σ.threads l }

def mapThread (σ : State) (ℓ : Thread) (f : ThreadRec → ThreadRec) : State :=
  { σ with threads := fun l => if l = ℓ then (σ.threads l).map f else σ.threads l }

/-- The objects `dead` names end at once: each is no longer alive and under no obligation, every path
    of one of them is invalid, and they leave every `heldBy`. -/
def endObjects (σ : State) (dead : Identity → Bool) : State :=
  { σ with
    objects := fun i => if dead i then (σ.objects i).map (fun r => { r with alive := false }) else σ.objects i,
    obligation := fun i => if dead i then false else σ.obligation i,
    paths := fun t => (σ.paths t).map (fun p =>
      { p with valid := if dead p.of then false else p.valid, heldBy := p.heldBy.filter (fun o => !dead o) }),
    holder := fun i => if dead i then none else σ.holder i }

/-! ### the steps -/

/-- `[Object-Establish-Plain]`/`[Object-Establish-Resource]` in ℓ: a fresh identity, a temporary of ℓ's
    current scope; a resource is under obligation, with ℓ's authority, from its establishment. -/
def establish (σ : State) (ℓ : Thread) (isResource : Bool) : State :=
  let o := σ.nextId
  { σ with
    objects := fun i => if i = o then some { alive := true, isResource := isResource, guard := false, frame := none, scope := σ.scope ℓ } else σ.objects i,
    obligation := fun i => if i = o then isResource else σ.obligation i,
    authority := fun i => if i = o then (if isResource then some ℓ else none) else σ.authority i,
    contents := fun i => if i = o then [] else σ.contents i,
    nextId := o + 1 }

/-- `[Binding-Form]` + `[Store-Binding-Temp]` in ℓ: ℓ's current frame adopts `o`; the root path is minted. -/
def bind (σ : State) (ℓ : Thread) (o : Identity) : State :=
  let a := σ.nextId
  let p : PathRecord := { of := o, proj := [], mode := Mode.exclusive, valid := true, base := none, thread := some ℓ, lock := false, frame := σ.frame ℓ, scope := σ.scope ℓ, heldBy := [] }
  { σ with
    paths := fun t => if t = a then some p else σ.paths t,
    holder := fun i => if i = o then some a else σ.holder i,
    objects := fun i => if i = o then (σ.objects i).map (fun r => { r with frame := some (σ.frame ℓ) }) else σ.objects i,
    nextId := a + 1 }

/-- `[Borrow]` in ℓ: a new path from `a0` with mode `m`, the same object and projection, `a0` as its base. -/
def borrow (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (m : Mode) : State :=
  let a := σ.nextId
  let p : PathRecord := { p0 with mode := m, valid := true, base := some a0, thread := some ℓ, lock := false, frame := σ.frame ℓ, scope := σ.scope ℓ, heldBy := [] }
  { σ with paths := fun t => if t = a then some p else σ.paths t, nextId := a + 1 }

/-- `[Thread-Lend]` in ℓ (D-0213): a `[Borrow]` of an exclusive path for a spawn's argument, whose
    thread is `transit` until a thread takes it. -/
def lend (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) : State :=
  let a := σ.nextId
  let p : PathRecord := { p0 with mode := Mode.exclusive, valid := true, base := some a0, thread := none, lock := false, frame := σ.frame ℓ, scope := σ.scope ℓ, heldBy := [] }
  { σ with paths := fun t => if t = a then some p else σ.paths t, nextId := a + 1 }

/-- `[Thread-Affine]`'s taking: ℓ's first access through an exclusive path that is in transit or a
    finished thread's makes it ℓ's. (The access itself is a `read`/`write`/`borrow` step; a lock path
    is never taken, D-0240.) -/
def takePath (σ : State) (ℓ : Thread) (a : Token) : State :=
  { σ with paths := fun t => if t = a then (σ.paths t).map (fun p => { p with thread := some ℓ }) else σ.paths t }

/-- `[Field-Access]`/`[Index-Checked]` in ℓ: a projection of `a0`, one step longer, the same mode and object. -/
def project (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (f : Proj) : State :=
  let a := σ.nextId
  let p : PathRecord := { p0 with proj := p0.proj ++ [f], valid := true, base := some a0, thread := some ℓ, lock := false, frame := σ.frame ℓ, scope := σ.scope ℓ, heldBy := [] }
  { σ with paths := fun t => if t = a then some p else σ.paths t, nextId := a + 1 }

/-- `[Write]` with the object's contents `refs` afterwards: every path's `held-by` gains or loses `o`
    by what the object holds (D-0215). -/
def writeWith (σ : State) (o : Identity) (refs : List Token) : State :=
  { σ with
    contents := fun i => if i = o then refs else σ.contents i,
    paths := fun t => (σ.paths t).map (fun p =>
      { p with heldBy := if t ∈ refs then (if o ∈ p.heldBy then p.heldBy else o :: p.heldBy) else p.heldBy.filter (· ≠ o) }) }

/-- `[Write]` through a path of `o`: the references the written slot held (`removed`, the slot's own by
    the rule; the fragment does not keep slots apart, so any sublist of the contents is admitted, which
    only weakens the premise) leave the object's contents and the written value's (`added`) join them. -/
def write (σ : State) (o : Identity) (removed added : List Token) : State :=
  writeWith σ o ((σ.contents o).filter (· ∉ removed) ++ added)

/-- `[Lock]` in ℓ through the shared path `am` of the mutex `o`: the lock path `ag` (exclusive, formed in
    ℓ, `am` its base, held by the guard) and the guard object `g` (a resource, a temporary of ℓ's scope,
    ℓ's authority, holding `ag`) are established, and the mutex becomes locked by ℓ through `ag`. -/
def lock (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) : State :=
  let ag := σ.nextId
  let g := σ.nextId + 1
  let p : PathRecord := { pm with mode := Mode.exclusive, valid := true, base := some am, thread := some ℓ, lock := true, frame := σ.frame ℓ, scope := σ.scope ℓ, heldBy := [g] }
  { σ with
    sync := fun i => if i = pm.of then SyncState.locked ℓ ag else σ.sync i,
    paths := fun t => if t = ag then some p else σ.paths t,
    objects := fun i => if i = g then some { alive := true, isResource := true, guard := true, frame := none, scope := σ.scope ℓ } else σ.objects i,
    obligation := fun i => if i = g then true else σ.obligation i,
    authority := fun i => if i = g then some ℓ else σ.authority i,
    contents := fun i => if i = g then [ag] else σ.contents i,
    nextId := g + 1 }

/-- `[Guard-Drop]`, the guard `g`'s destructor run by `[Destroy]`: the guard ends, its lock path `ag`
    ends, and the mutex `o` is unlocked. -/
def guardDrop (σ : State) (g : Identity) (ag : Token) (o : Identity) : State :=
  let σ1 := endObjects σ (fun i => decide (i = g))
  { σ1 with
    sync := fun i => if i = o then SyncState.unlocked else σ.sync i,
    paths := fun t => if t = ag then (σ1.paths t).map (fun p => { p with valid := false }) else σ1.paths t }

/-- `[Object-End]`/`[Destroy]`: one object ends. -/
def endObject (σ : State) (o : Identity) : State := endObjects σ (fun i => decide (i = o))

/-- `[Authority-Transfer]` of `o` from the path `a1` to the root path `a2` (formed in `p2.thread`, in its
    frame `p2.frame`): the authority is re-keyed, the holder re-stamped, the source path ends. -/
def transfer (σ : State) (a1 a2 : Token) (ℓ2 : Thread) (f2 : Frame) (o : Identity) : State :=
  { restamp (rekey σ (fun i => decide (i = o)) ℓ2) (fun i => decide (i = o)) (some f2) none with
    paths := fun t => if t = a1 then (σ.paths t).map (fun p => { p with valid := false }) else σ.paths t,
    holder := fun i => if i = o then some a2 else σ.holder i }

/-- `[Block-Enter]` in ℓ: a frame is pushed on ℓ's stack. -/
def blockEnter (σ : State) (ℓ : Thread) (f : Frame) : State :=
  mapThread σ ℓ (fun tr => { tr with frames := f :: tr.frames })

/-- The objects ℓ's current frame owns (`[Block-Exit]`'s `Owned`). -/
def blockDead (σ : State) (ℓ : Thread) : Identity → Bool := fun i => match σ.objects i with
  | some r => r.frame == some (σ.frame ℓ)
  | none => false

/-- `[Block-Exit]` in ℓ: the current frame's owned objects end (destroyed, for resources; a handle among
    them is `[Join]`ed or `[Handle-Destructor]`ed first, as its own step), and the frame is popped. -/
def blockExit (σ : State) (ℓ : Thread) : State :=
  mapThread (endObjects σ (blockDead σ ℓ)) ℓ (fun tr => { tr with frames := tr.frames.tail })

/-- `[Stmt-Enter]` in ℓ: a statement scope is pushed on ℓ's stack. -/
def stmtEnter (σ : State) (ℓ : Thread) (t : Scope) : State :=
  mapThread σ ℓ (fun tr => { tr with scopes := t :: tr.scopes })

/-- The temporaries of ℓ's current scope (`[Stmt-Exit]`'s `Temps`). -/
def stmtDead (σ : State) (ℓ : Thread) : Identity → Bool := fun i => match σ.objects i with
  | some r => r.frame.isNone && r.scope == σ.scope ℓ
  | none => false

/-- `[Stmt-Exit]`'s `Unheld`: every path formed in scope `t`, held by no object and owning nothing, ends. -/
def invalidateUnheld (σ : State) (t : Scope) : State :=
  { σ with
    paths := fun a => (σ.paths a).map (fun p =>
      { p with valid := if p.scope = t ∧ p.heldBy = [] ∧ σ.holder p.of ≠ some a then false else p.valid }) }

/-- `[Stmt-Exit]` in ℓ: the current scope's temporaries end, its unheld paths end, and it is popped. -/
def stmtExit (σ : State) (ℓ : Thread) : State :=
  mapThread (invalidateUnheld (endObjects σ (stmtDead σ ℓ)) (σ.scope ℓ)) ℓ (fun tr => { tr with scopes := tr.scopes.tail })

/-- `[Spawn]` in ℓ: the handle (a resource, a temporary of ℓ's scope, ℓ's authority) is established, and
    the new thread ℓ' starts with its parameter frame `f'` and an empty scope stack. (Its arguments
    arrive by `[Authority-Transfer]` to paths formed in ℓ', as separate steps.) -/
def spawn (σ : State) (ℓ ℓ' : Thread) (f' : Frame) : State :=
  setThread (establish σ ℓ true) ℓ' (some { frames := [f'], scopes := [], done := none, handle := σ.nextId })

/-- `[Thread-Body-Done]` of ℓ' with the result's objects `res`: the parameter frame's `[Block-Exit]`
    (its owned objects other than the result's end; the result's lose their holder), the stacks are
    empty, and the result is recorded for `[Join]`/`[Handle-Destructor]`. -/
def threadDone (σ : State) (ℓ' : Thread) (res : List Identity) : State :=
  let σ1 := endObjects σ (fun i => blockDead σ ℓ' i && !decide (i ∈ res))
  let σ2 := restamp σ1 (fun i => decide (i ∈ res)) none none
  mapThread σ2 ℓ' (fun tr => { tr with frames := [], scopes := [], done := some res })

/-- `[Join]` by ℓ of the done thread ℓ' (handle `h`, result `res`): the result's authority is re-keyed
    to ℓ and its objects become temporaries of ℓ's current scope; the handle is destroyed (its
    destructor finds the result taken); the thread's record is removed. -/
def join (σ : State) (ℓ ℓ' : Thread) (h : Identity) (res : List Identity) : State :=
  let σ1 := endObjects σ (fun i => decide (i = h))
  let σ2 := restamp σ1 (fun i => decide (i ∈ res)) none (some (σ.scope ℓ))
  setThread (rekey σ2 (fun i => decide (i ∈ res)) ℓ) ℓ' none

/-- `[Handle-Destructor]` by ℓ of the handle `h` of the done thread ℓ' whose result was not taken: the
    result's objects are discarded (destroyed) with the handle; the thread's record is removed. -/
def handleDestroy (σ : State) (ℓ' : Thread) (h : Identity) (res : List Identity) : State :=
  setThread (endObjects σ (fun i => decide (i = h) || decide (i ∈ res))) ℓ' none

/-- A step of the fragment, with the rule's premises as hypotheses; ℓ is the performer. -/
inductive Step : State → State → Prop where
  | establish (σ : State) (ℓ : Thread) (r : Bool) :
      σ.running ℓ → σ.scopes ℓ ≠ [] → Step σ (establish σ ℓ r)
  | bind (σ : State) (ℓ : Thread) (o : Identity) :
      σ.running ℓ → σ.alive o → σ.frames ℓ ≠ [] →
      (∀ ℓ', σ.authority o = some ℓ' → ℓ' = ℓ) →            -- a thread binds what it has authority over (or a plain object)
      Step σ (bind σ ℓ o)
  | borrow (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (m : Mode) :
      σ.running ℓ → σ.paths a0 = some p0 → σ.temporallyValid a0 →
      (m = Mode.exclusive → p0.mode = Mode.exclusive) →
      (p0.mode = Mode.exclusive → p0.thread = some ℓ) →        -- `[Thread-Affine]`, after any taking
      ¬ σ.clash a0 m → Step σ (borrow σ ℓ a0 p0 m)
  | project (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) (f : Proj) :
      σ.running ℓ → σ.paths a0 = some p0 → σ.temporallyValid a0 → Step σ (project σ ℓ a0 p0 f)
  | lend (σ : State) (ℓ : Thread) (a0 : Token) (p0 : PathRecord) :
      σ.running ℓ → σ.paths a0 = some p0 → σ.temporallyValid a0 → p0.mode = Mode.exclusive →
      p0.lock = false → σ.affine p0 ℓ → ¬ σ.clash a0 Mode.exclusive → Step σ (lend σ ℓ a0 p0)
  | takePath (σ : State) (ℓ : Thread) (a : Token) (p : PathRecord) :
      σ.running ℓ → σ.paths a = some p → p.mode = Mode.exclusive → p.lock = false → σ.affine p ℓ →
      Step σ (takePath σ ℓ a)
  | read (σ : State) (ℓ : Thread) (a : Token) (p : PathRecord) :
      σ.running ℓ → σ.paths a = some p → σ.temporallyValid a → ¬ σ.clash a Mode.shared →
      (p.mode = Mode.exclusive → p.thread = some ℓ) → Step σ σ
  | write (σ : State) (ℓ : Thread) (a : Token) (p : PathRecord) (removed added : List Token) :
      σ.running ℓ → σ.paths a = some p → σ.temporallyValid a → p.mode = Mode.exclusive → p.thread = some ℓ →
      ¬ σ.clash a Mode.exclusive → (∀ t ∈ added, σ.paths t ≠ none) → Step σ (write σ p.of removed added)
  | destroy (σ : State) (ℓ : Thread) (o : Identity) :
      σ.running ℓ → σ.alive o → σ.obligation o = true → σ.authority o = some ℓ →
      σ.noHandle (fun i => decide (i = o)) → σ.noGuard (fun i => decide (i = o)) → Step σ (endObject σ o)
  | endPlain (σ : State) (ℓ : Thread) (o : Identity) :
      σ.running ℓ → σ.alive o → σ.obligation o = false →
      σ.noHandle (fun i => decide (i = o)) → σ.noGuard (fun i => decide (i = o)) → Step σ (endObject σ o)
  | transfer (σ : State) (ℓ ℓ2 : Thread) (a1 a2 : Token) (p1 p2 : PathRecord) :
      σ.running ℓ → σ.paths a1 = some p1 → σ.temporallyValid a1 → σ.authority p1.of = some ℓ →
      σ.paths a2 = some p2 → p2.of = p1.of → p2.base = none → p2.valid = true → p2.thread = some ℓ2 →
      σ.running ℓ2 → p2.frame ∈ σ.frames ℓ2 →   -- `a2` was formed by `[Binding-Form]` in an open frame
      Step σ (transfer σ a1 a2 ℓ2 p2.frame p1.of)
  | blockEnter (σ : State) (ℓ : Thread) (f : Frame) :
      σ.running ℓ → Step σ (blockEnter σ ℓ f)
  | blockExit (σ : State) (ℓ : Thread) :
      σ.running ℓ → σ.frames ℓ ≠ [] → σ.noHandle (blockDead σ ℓ) → σ.noGuard (blockDead σ ℓ) → Step σ (blockExit σ ℓ)
  | stmtEnter (σ : State) (ℓ : Thread) (t : Scope) :
      σ.running ℓ → Step σ (stmtEnter σ ℓ t)
  | stmtExit (σ : State) (ℓ : Thread) :
      σ.running ℓ → σ.scopes ℓ ≠ [] → σ.noHandle (stmtDead σ ℓ) → σ.noGuard (stmtDead σ ℓ) → Step σ (stmtExit σ ℓ)
  | spawn (σ : State) (ℓ ℓ' : Thread) (f' : Frame) :
      σ.running ℓ → σ.scopes ℓ ≠ [] → σ.threads ℓ' = none → Step σ (spawn σ ℓ ℓ' f')
  | threadDone (σ : State) (ℓ' : Thread) (tr : ThreadRec) (f' : Frame) (res : List Identity) :
      σ.threads ℓ' = some tr → tr.done = none → tr.frames = [f'] → tr.scopes = [] →
      σ.obligation tr.handle = true →                        -- the handle has not been destroyed: that blocks until done
      (∀ o ∈ res, σ.obligation o = true → σ.authority o = some ℓ') →   -- the result's resources are the thread's own
      σ.noHandle (fun i => blockDead σ ℓ' i && !decide (i ∈ res)) →
      σ.noGuard (fun i => blockDead σ ℓ' i && !decide (i ∈ res)) →
      Step σ (threadDone σ ℓ' res)
  | join (σ : State) (ℓ ℓ' : Thread) (tr : ThreadRec) (res : List Identity) :
      σ.running ℓ → ℓ ≠ ℓ' → σ.scopes ℓ ≠ [] →
      σ.threads ℓ' = some tr → tr.done = some res → tr.frames = [] → tr.scopes = [] →
      σ.authority tr.handle = some ℓ →
      (∀ ℓ'' tr'', ℓ'' ≠ ℓ' → σ.threads ℓ'' = some tr'' → tr''.handle ≠ tr.handle) →   -- handles are distinct
      Step σ (join σ ℓ ℓ' tr.handle res)
  | handleDestroy (σ : State) (ℓ ℓ' : Thread) (tr : ThreadRec) (res : List Identity) :
      σ.running ℓ → σ.threads ℓ' = some tr → tr.done = some res → tr.frames = [] → tr.scopes = [] →
      σ.authority tr.handle = some ℓ →
      (∀ ℓ'' tr'', ℓ'' ≠ ℓ' → σ.threads ℓ'' = some tr'' → tr''.handle ≠ tr.handle ∧ tr''.handle ∉ res) →
          -- a result holding another thread's handle is joined first, then dropped
      σ.noGuard (fun i => decide (i = tr.handle) || decide (i ∈ res)) →
      Step σ (handleDestroy σ ℓ' tr.handle res)

  | lock (σ : State) (ℓ : Thread) (am : Token) (pm : PathRecord) :
      σ.running ℓ → σ.scopes ℓ ≠ [] → σ.paths am = some pm → σ.temporallyValid am →
      σ.sync pm.of = SyncState.unlocked →                   -- blocks while locked by another thread
      Step σ (lock σ ℓ am pm)
  | guardDrop (σ : State) (ℓ : Thread) (g : Identity) (r : ObjectRecord) (ag : Token) (p : PathRecord) :
      σ.running ℓ → σ.objects g = some r → r.alive = true → r.guard = true →
      σ.obligation g = true → σ.authority g = some ℓ → σ.contents g = [ag] →
      σ.paths ag = some p → p.lock = true → p.valid = true →
      σ.noHandle (fun i => decide (i = g)) →
      Step σ (guardDrop σ g ag p.of)

/-- The reachable states: from a state satisfying the invariants (σ_0 does, vacuously), by steps. -/
inductive Reachable : State → Prop where
  | init (σ : State) : σ.Invariants → Reachable σ
  | step (σ σ' : State) : Reachable σ → Step σ σ' → Reachable σ'

end Cobalt
