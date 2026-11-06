/-
  σ for the fragment (spec/04 §1, the components the fragment's rules write):
  objects with their liveness, contents, owner frame and scope; access paths
  with mode, validity, base, forming thread and the objects holding them;
  destruction obligations (of objects and of their sub-resources) and the thread holding each one's authority; the
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
  reclaimed : Bool          -- `storage-kind = reclaimed` (spec/20 §2): raw storage an `unsafe` author claimed
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
  taken : Bool              -- taken over by `[Thread-Affine]` (D-0237): its thread is no longer contained
  pending : Bool            -- an argument's path until `[Call]` binds it (D-0107), a projection's until its one
                            -- access (D-0242): `clash` does not count it
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
  | poisoned                -- `[Fault-Contain]`: held by a thread whose fault was contained (D-0204)
  deriving DecidableEq, Repr

structure State where
  sync : Identity → SyncState
  objects : Identity → Option ObjectRecord
  paths : Token → Option PathRecord
  holder : Identity → Option Token
  obligation : Identity → Bool        -- `state.destruction-obligations`
  authority : Identity → Option Thread   -- the thread whose `authority(ℓ, destroy, o)` is unconsumed, if any
  subs : Identity → List Proj → Option Thread
      -- the sub-resources: `(o, path) ∈ destruction-obligations` for `path ≠ ε` (`[Relocate-In]`), each with
      -- the thread whose `authority(ℓ, destroy, (o, path))` is unconsumed
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

/-- `clash(a, m, σ)` (spec/04 §2, D-0018) for the fragment: a valid, non-pending (D-0107, D-0242),
    non-ancestor path of the same object whose projection overlaps and whose mode is not jointly
    permitted; neither path lock-derived (`sync-exempt`, spec/19 §2: a lock path reaches the mutex's
    interior, which only it reaches, and the paths the mutex is reached through are no conflict with it,
    nor it with them; D-0270). -/
def clash (σ : State) (a : Token) (m : Mode) : Prop :=
  ∃ a' p p', σ.paths a = some p ∧ σ.paths a' = some p' ∧ a' ≠ a ∧ p'.valid = true ∧ p'.pending = false ∧
    p.lock = false ∧ p'.lock = false ∧ p'.of = p.of ∧ overlap p.proj p'.proj ∧ a' ∉ ancestors σ σ.nextId a ∧ ¬ permitted m p'.mode

/-- `solitary(a)` (spec/08 §1): no valid path of the object other than the holder's root path — what
    `[Destroy]` and `[Authority-Transfer]` require (`[Destroy-Not-Solitary]`,
    `[Authority-Transfer-Aliased]`; D-0270). -/
def solitary (σ : State) (o : Identity) : Prop :=
  ∀ a p, σ.paths a = some p → p.valid = true → p.of = o → σ.holder o = some a

/-- `o` is a temporary of ℓ's current statement scope: what `[Binding-Form]` adopts and `[Temp-Root]`
    roots (D-0270). -/
def tempOf (σ : State) (ℓ : Thread) (o : Identity) : Prop :=
  ∃ r, σ.objects o = some r ∧ r.frame = none ∧ r.scope = σ.scope ℓ

/-- `o` is ℓ's own: owned by one of its open frames, or a temporary of one of its open scopes — what
    ℓ may end (`[Object-End]`) or move whole (`[Relocate-In]`, `[Rawptr-Move-In]`) (D-0270). -/
def owns (σ : State) (ℓ : Thread) (o : Identity) : Prop :=
  ∃ r, σ.objects o = some r ∧ ((∃ f, r.frame = some f ∧ f ∈ σ.frames ℓ) ∨ (r.frame = none ∧ r.scope ∈ σ.scopes ℓ))

/-- `affine(a, ℓ, Σ)` (spec/08 `[Thread-Affine]`, D-0213): an exclusive path is this thread's, in
    transit, or a thread's that no longer runs. -/
def affine (σ : State) (p : PathRecord) (ℓ : Thread) : Prop :=
  p.thread = some ℓ ∨ p.thread = none ∨ (∃ ℓ', p.thread = some ℓ' ∧ ¬ σ.running ℓ')

/-- `contained(ℓ, Σ)` for the fragment: ℓ holds no exclusive path it was lent or took over (its own
    exclusive paths, formed by its borrows, are its own data). -/
def contained (σ : State) (ℓ : Thread) : Prop :=
  ∀ a p, σ.paths a = some p → p.thread = some ℓ → p.mode = Mode.exclusive → p.taken = false

/-- `foreign-held(o, ℓ, Σ)` (spec/20 §2, D-0214): another running thread, or a thread not yet started, holds an
    exclusive non-root path to `o` that something stores. -/
def foreignHeld (σ : State) (ℓ : Thread) (o : Identity) : Prop :=
  ∃ a p, σ.paths a = some p ∧ p.valid = true ∧ p.of = o ∧ p.base ≠ none ∧ p.mode = Mode.exclusive ∧ p.heldBy ≠ [] ∧
    p.thread ≠ some ℓ ∧ (p.thread = none ∨ ∃ ℓ', p.thread = some ℓ' ∧ σ.running ℓ')

/-- No thread's handle is among the objects `dead` names: a handle ends only by `[Join]` or
    `[Handle-Destructor]`, which block until the thread is done (modelled as their own steps). -/
def noHandle (σ : State) (dead : Identity → Bool) : Prop :=
  ∀ ℓ tr, σ.threads ℓ = some tr → dead tr.handle = false

/-- No live guard is among the objects `dead` names: a guard ends only by `[Guard-Drop]` (its own step);
    a guard already ended is nothing to a later end (D-0270). -/
def noGuard (σ : State) (dead : Identity → Bool) : Prop :=
  ∀ o r, σ.objects o = some r → r.alive = true → r.guard = true → dead o = false

end State
end Cobalt
