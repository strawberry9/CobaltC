import Cobalt.State
/-
  spec/23's auxiliary state lemmas as predicates on σ.
-/
namespace Cobalt
namespace State

/-- (T1): every valid path's object is alive. -/
def T1 (σ : State) : Prop :=
  ∀ a p, σ.paths a = some p → p.valid = true → σ.alive p.of

/-- (T2), spec/04 §1's definition of `held-by` kept as a field:
    `held-by(a) = { o | alive(o) ∧ a ∈ refs-in(contents(o)) }`. -/
def T2 (σ : State) : Prop :=
  ∀ a p, σ.paths a = some p → ∀ o, o ∈ p.heldBy ↔ (σ.alive o ∧ a ∈ σ.contents o)

/-- (A): a path's base, when it has one, is a path with a smaller token
    (formed earlier), so ancestor chains are finite. -/
def A (σ : State) : Prop :=
  ∀ a p b, σ.paths a = some p → p.base = some b → b < a ∧ σ.paths b ≠ none

/-- Every identity and token in use is below `nextId`. -/
def Fresh (σ : State) : Prop :=
  (∀ a p, σ.paths a = some p → a < σ.nextId) ∧ (∀ o r, σ.objects o = some r → o < σ.nextId) ∧
  (∀ o t, t ∈ σ.contents o → t < σ.nextId)

/-- The owner that will discharge an obligation (`inv.resource-authority`'s cases for a fragment
    without sub-resources or raw storage): the holder's frame is open in the authority's thread, or the
    object is a temporary of a scope open there, or it is in the result of that thread, done, whose
    handle is still under obligation (`[Join]` or `[Handle-Destructor]` will claim the result), or it is a
    reclaimed object (spec/20 §2: its owner is the `unsafe` author, under `discharge: trusted`; the fourth
    case of `inv.resource-authority`). -/
def Owner (σ : State) (o : Identity) (r : ObjectRecord) (tr : ThreadRec) : Prop :=
  (∃ f, r.frame = some f ∧ f ∈ tr.frames) ∨
  (r.frame = none ∧ r.scope ∈ tr.scopes) ∨
  (∃ res, tr.done = some res ∧ o ∈ res ∧ σ.obligation tr.handle = true) ∨
  r.reclaimed = true

/-- (R), `inv.resource-authority` for the fragment: every object under a destruction obligation is
    alive, exactly one thread holds its authority, and that thread owns it in one of `Owner`'s ways. -/
def R (σ : State) : Prop :=
  ∀ o, σ.obligation o = true → ∃ r ℓ tr, σ.objects o = some r ∧ r.alive = true ∧
    σ.authority o = some ℓ ∧ σ.threads ℓ = some tr ∧ Owner σ o r tr

/-- (C), `inv.concurrency-validity`'s state lemma for the fragment, one direction: a mutex locked by ℓ
    through `a` has `a` as a lock path of its own, exclusive, formed in ℓ. -/
def C1 (σ : State) : Prop :=
  ∀ o ℓ a, σ.sync o = SyncState.locked ℓ a →
    ∃ p, σ.paths a = some p ∧ p.lock = true ∧ p.of = o ∧ p.thread = some ℓ ∧ p.mode = Mode.exclusive

/-- (C), the other direction: a valid lock path is the lock its mutex is held through, by its thread —
    so at most one lock path of a mutex is valid at a time. -/
def C2 (σ : State) : Prop :=
  ∀ a p, σ.paths a = some p → p.lock = true → p.valid = true → ∃ ℓ, p.thread = some ℓ ∧ σ.sync p.of = SyncState.locked ℓ a

/-- (P), D-0107/D-0242: a pending path is held by no object — it is bound to its parameter or makes
    its one access before anything stores it, so its exemption from `clash` lasts only until then. -/
def P (σ : State) : Prop :=
  ∀ a p, σ.paths a = some p → p.pending = true → p.heldBy = []

/-- (RS), `inv.resource-authority`'s third owner case ("`(o, path)` with `o` live") for the fragment: a
    sub-resource's key is a proper sub-path, its container is itself under obligation (an aggregate
    holding a resource is a resource, spec/12 §3), with the same authority thread — so by (R) the
    container's owner, alive with its thread's frame, scope or result, is the sub-obligation's. -/
def RS (σ : State) : Prop :=
  ∀ o q ℓ, σ.subs o q = some ℓ → q ≠ [] ∧ σ.obligation o = true ∧ σ.authority o = some ℓ

def Invariants (σ : State) : Prop := T1 σ ∧ T2 σ ∧ A σ ∧ Fresh σ ∧ R σ ∧ C1 σ ∧ C2 σ ∧ P σ ∧ RS σ

end State
end Cobalt
