/-
  σ for the fragment (spec/04 §1, the components the fragment's rules write):
  objects with their liveness, contents, owner frame and scope; access paths
  with mode, validity, base, forming thread and the objects holding them;
  destruction obligations and the thread holding each one's authority; the
  threads with their frame and scope stacks, their result once done, and
  their handle. Identities and tokens are natural numbers below `nextId`;
  a component is a function from its key to an optional record, so that an
  update is a pointwise redefinition and a lookup is an application.
-/
namespace Cobalt

abbrev Identity := Nat
abbrev Token := Nat
abbrev Frame := Nat
abbrev Scope := Nat
abbrev Thread := Nat

inductive Mode where
  | shared
  | exclusive
  deriving DecidableEq, Repr

/-- A projection step: the fragment has fields (an index) and nothing else. -/
inductive Proj where
  | field (i : Nat)
  deriving DecidableEq, Repr

structure ObjectRecord where
  alive : Bool
  isResource : Bool
  guard : Bool              -- a `guard<τ>` (`[Lock]`): ended only by `[Guard-Drop]`
  frame : Option Frame      -- the frame that owns it, `none` for a temporary
  scope : Scope             -- the statement scope it was established in (`temp-scope`)
  deriving Repr

structure PathRecord where
  of : Identity
  proj : List Proj
  mode : Mode
  valid : Bool
  base : Option Token
  thread : Option Thread    -- the forming thread (`access-paths(a).thread`); `none` is `transit` (D-0213)
  lock : Bool               -- formed by `[Lock]` (`lock-derived` at depth 0)
  frame : Frame
  scope : Scope
  heldBy : List Identity    -- the live objects whose contents hold this path
  deriving Repr

/-- `state.threads(ℓ)` with the thread's own stacks (`state.frame-stack(ℓ)`, `state.temp-scope-stack(ℓ)`). -/
structure ThreadRec where
  frames : List Frame            -- head first
  scopes : List Scope            -- head first
  done : Option (List Identity)  -- `value`: `none` while running, else the result's objects (`objs-in(r_b)`)
  handle : Identity              -- the `handle<τ>` object `[Spawn]` established
  deriving Repr

/-- `state.sync` for a mutex (spec/04): unlocked, or locked by a thread through its lock path.
    (`poisoned` is `[Fault-Contain]`'s, outside the fragment.) -/
inductive SyncState where
  | unlocked
  | locked (ℓ : Thread) (a : Token)
  deriving DecidableEq, Repr

structure State where
  sync : Identity → SyncState
  objects : Identity → Option ObjectRecord
  paths : Token → Option PathRecord
  holder : Identity → Option Token
  obligation : Identity → Bool        -- `state.destruction-obligations`
  authority : Identity → Option Thread   -- the thread whose `authority(ℓ, destroy, o)` is unconsumed, if any
  contents : Identity → List Token    -- `refs-in(contents(o))`: the reference tokens an object's value holds
  threads : Thread → Option ThreadRec
  nextId : Nat                        -- every identity and token is below it

namespace State

def frames (σ : State) (ℓ : Thread) : List Frame :=
  match σ.threads ℓ with
  | some tr => tr.frames
  | none => []

def scopes (σ : State) (ℓ : Thread) : List Scope :=
  match σ.threads ℓ with
  | some tr => tr.scopes
  | none => []

/-- `current-frame(ℓ)`, `current-scope(ℓ)` (spec/04): the heads; 0 when empty (no step forms anything then). -/
def frame (σ : State) (ℓ : Thread) : Frame := (σ.frames ℓ).headD 0
def scope (σ : State) (ℓ : Thread) : Scope := (σ.scopes ℓ).headD 0

/-- `running(ℓ, Σ)`: the thread exists and its body is not done. -/
def running (σ : State) (ℓ : Thread) : Prop :=
  ∃ tr, σ.threads ℓ = some tr ∧ tr.done = none

/-- `alive(o, σ)`: a record exists and is alive. -/
def alive (σ : State) (o : Identity) : Prop :=
  ∃ r, σ.objects o = some r ∧ r.alive = true

/-- `temporally-valid(a, σ)` (spec/04 §2): the path is valid and its object alive. -/
def temporallyValid (σ : State) (a : Token) : Prop :=
  ∃ p, σ.paths a = some p ∧ p.valid = true ∧ σ.alive p.of

/-- `overlap`: one projection list is a prefix of the other (spec/04 §2). -/
def overlap : List Proj → List Proj → Prop
  | [], _ => True
  | _, [] => True
  | x :: xs, y :: ys => x = y ∧ overlap xs ys

/-- `permitted(m1, m2)` (D-0004): both shared. -/
def permitted (m1 m2 : Mode) : Prop := m1 = Mode.shared ∧ m2 = Mode.shared

/-- `ancestors(a, σ)`: the bases, transitively; `fuel` bounds the walk (its
    adequacy is lemma (A): a base's token is smaller, so `nextId` steps suffice). -/
def ancestors (σ : State) : Nat → Token → List Token
  | 0, _ => []
  | fuel + 1, a =>
    match σ.paths a with
    | some p =>
      match p.base with
      | some b => b :: ancestors σ fuel b
      | none => []
    | none => []

/-- `clash(a, m, σ)` (spec/04 §2, D-0018) for the fragment (no pending
    paths, no locks): a valid non-ancestor path of the same object whose
    projection overlaps and whose mode is not jointly permitted. -/
def clash (σ : State) (a : Token) (m : Mode) : Prop :=
  ∃ a' p p', σ.paths a = some p ∧ σ.paths a' = some p' ∧ a' ≠ a ∧ p'.valid = true ∧
    p'.of = p.of ∧ overlap p.proj p'.proj ∧ a' ∉ ancestors σ σ.nextId a ∧ ¬ permitted m p'.mode

/-- `affine(a, ℓ, Σ)` (spec/08 `[Thread-Affine]`, D-0213): an exclusive path is this thread's, in
    transit, or a thread's that no longer runs. -/
def affine (σ : State) (p : PathRecord) (ℓ : Thread) : Prop :=
  p.thread = some ℓ ∨ p.thread = none ∨ (∃ ℓ', p.thread = some ℓ' ∧ ¬ σ.running ℓ')

/-- No thread's handle is among the objects `dead` names: a handle ends only by `[Join]` or
    `[Handle-Destructor]`, which block until the thread is done (modelled as their own steps). -/
def noHandle (σ : State) (dead : Identity → Bool) : Prop :=
  ∀ ℓ tr, σ.threads ℓ = some tr → dead tr.handle = false

/-- No guard is among the objects `dead` names: a guard ends only by `[Guard-Drop]` (its own step). -/
def noGuard (σ : State) (dead : Identity → Bool) : Prop :=
  ∀ o r, σ.objects o = some r → r.guard = true → dead o = false

end State
end Cobalt
