import Cobalt.Exec
import Std.Data.HashMap
/-
  `lake exe replay TRACE`: runs a trace of the model checker (`cobmodel trace`, D-0270) through the
  fragment's executable machine (`Cobalt/Exec.lean`). Each line names a step with the model's own
  identities; the replay maps them to the machine's (allocated in order, as the steps allocate them),
  decides the step's premises (`check`) and performs it (`apply`). An admitted action is a `Step`
  (`exec_sound`), so a trace that replays whole has been shown to lie within the fragment's rules; the
  first action the machine refuses is reported, with the line, and the exit status is 1.

  Two readings of the rules the fragment does not yet take are applied as counted gaps, not steps:
  `inv A` (`[Object-End]`'s `Dead`: a path whose only holder ended is invalid) and `rsc A S` (`[Stmt-Exit]`:
  a statement result's unheld paths pass to the parent scope). A line `out …` ends the replay where the
  model left the fragment (vectors, channels, atomics, slices, closures, statics, arrays); what came
  before has been checked.

  Line forms (T, T2 threads; O objects; A tokens; F frames; S scopes; all the model's numbers):
    ben T F · bex T · sen T S · sex T
    est T O R · bnd T O A · bnh T O A H (O is the handle or guard object H the machine already has)
    brw T A0 A M P · prj T A0 A F · lnd T A0 A · tak T A · rd T A · set T A
    wr T A n r1 … rn m d1 … dm · tmp T O A · bpa T A O AR
    xfr T T2 A1 O1 O2 A2 (the object O1 moves: the machine keeps its identity under O2, root A2)
    rcl T O A · rel T n O1 … On · dst T O · epl T O
    spn T T2 F H · tdn T2 n O1 … On · jon T T2 · hds T T2 · fct T2
    lck T AM AG G · gdp T G · inv A · rsc A S · out reason…
-/
open Cobalt Cobalt.State

structure Maps where
  obj : Std.HashMap Nat Nat := {}
  tok : Std.HashMap Nat Nat := {}
  tid : Std.HashMap Nat Nat := {}
  nextTid : Nat := 1

structure Run where
  σ : State
  maps : Maps
  steps : Nat := 0
  gaps : Nat := 0

def σ0 : State :=
  { sync := fun _ => SyncState.unlocked, objects := fun _ => none, paths := fun _ => none, holder := fun _ => none,
    obligation := fun _ => false, authority := fun _ => none, subs := fun _ _ => none, contents := fun _ => [],
    threads := fun ℓ => if ℓ = 0 then some { frames := [], scopes := [], done := none, handle := 0 } else none,
    nextId := 1 }

inductive Outcome where
  | ok (steps gaps : Nat)
  | outside (steps gaps : Nat) (reason : String)
  | refused (steps : Nat) (line : Nat) (text : String)
  | malformed (line : Nat) (text : String)

def Outcome.text : Outcome → String
  | .ok s g => s!"ok: {s} step(s) admitted, {g} gap(s) applied"
  | .outside s g r => s!"outside the fragment ({r}): {s} step(s) admitted before it, {g} gap(s) applied"
  | .refused s l t => s!"REFUSED at line {l}: `{t}` after {s} admitted step(s)"
  | .malformed l t => s!"malformed trace at line {l}: `{t}`"

abbrev M := ExceptT String (StateM Run)

def nat (w : String) : M Nat :=
  match w.toNat? with
  | some n => pure n
  | none => throw s!"not a number: {w}"

def lookup (kind : String) (m : Std.HashMap Nat Nat) (k : Nat) : M Nat :=
  match m[k]? with
  | some v => pure v
  | none => throw s!"unmapped {kind} {k}"

def getObj (k : Nat) : M Nat := do lookup "object" (← get).maps.obj k
def getTok (k : Nat) : M Nat := do lookup "token" (← get).maps.tok k
def getTid (k : Nat) : M Nat := do
  if k = 0 then pure 0 else lookup "thread" (← get).maps.tid k

def mapObj (k v : Nat) : M Unit := modify fun r => { r with maps := { r.maps with obj := r.maps.obj.insert k v } }
def mapTok (k v : Nat) : M Unit := modify fun r => { r with maps := { r.maps with tok := r.maps.tok.insert k v } }
def mapTid (k : Nat) : M Nat := do
  let r ← get
  let v := r.maps.nextTid
  set { r with maps := { r.maps with tid := r.maps.tid.insert k v, nextTid := v + 1 } }
  pure v

/-- The state's components as arrays over the identities, tokens and threads in use, read back through
    constant-time lookups: the same state, with the chain of updates `apply` layered on it flattened.
    (`subs` is kept as it is: no replayed action reads it.) The actions read nothing above the bounds
    (`Fresh`, `ThreadsBelow`), so the materialised state agrees with the layered one on every lookup
    the machine makes. -/
def materialize (σ : State) (nTid : Nat) : State :=
  let n := σ.nextId
  let objs := Array.ofFn (n := n) (fun i => σ.objects i.val)
  let paths := Array.ofFn (n := n) (fun i => σ.paths i.val)
  let holder := Array.ofFn (n := n) (fun i => σ.holder i.val)
  let obl := Array.ofFn (n := n) (fun i => σ.obligation i.val)
  let auth := Array.ofFn (n := n) (fun i => σ.authority i.val)
  let cont := Array.ofFn (n := n) (fun i => σ.contents i.val)
  let sync := Array.ofFn (n := n) (fun i => σ.sync i.val)
  let thr := Array.ofFn (n := nTid) (fun i => σ.threads i.val)
  { σ with
    objects := fun i => (objs[i]?).getD none,
    paths := fun i => (paths[i]?).getD none,
    holder := fun i => (holder[i]?).getD none,
    obligation := fun i => (obl[i]?).getD false,
    authority := fun i => (auth[i]?).getD none,
    contents := fun i => (cont[i]?).getD [],
    sync := fun i => (sync[i]?).getD SyncState.unlocked,
    threads := fun i => (thr[i]?).getD none }

/-- Which premises of a refused action held (the common actions), for the report. -/
def why (σ : State) (n : Nat) : Act → String
  | .write ℓ a _ added =>
    match σ.paths a with
    | some p => s!"running={runningB σ ℓ} valid={tvB σ a} exclusive={decide (p.mode = Mode.exclusive)} own={decide (p.thread = some ℓ)} clash={clashB σ a Mode.exclusive} added-bound={added.all (fun t => match σ.paths t with | some q => !q.pending | none => false)}"
    | none => "no such path"
  | .read ℓ a =>
    match σ.paths a with
    | some p => s!"running={runningB σ ℓ} valid={tvB σ a} clash={clashB σ a Mode.shared} affine={!decide (p.mode = Mode.exclusive) || decide (p.thread = some ℓ)}"
    | none => "no such path"
  | .borrow ℓ a0 m _ =>
    match σ.paths a0 with
    | some p0 => s!"running={runningB σ ℓ} valid={tvB σ a0} mode-ok={!decide (m = Mode.exclusive) || decide (p0.mode = Mode.exclusive)} own={!decide (p0.mode = Mode.exclusive) || decide (p0.thread = some ℓ)} clash={clashB σ a0 m}"
    | none => "no such path"
  | .blockExit ℓ => s!"running={runningB σ ℓ} frames={nonemptyB (σ.frames ℓ)} handles={noHandleB σ n (blockDead σ ℓ)} guards={noGuardB σ (blockDead σ ℓ)}"
  | .stmtExit ℓ => s!"running={runningB σ ℓ} scopes={nonemptyB (σ.scopes ℓ)} handles={noHandleB σ n (stmtDead σ ℓ)} guards={noGuardB σ (stmtDead σ ℓ)}"
  | .endPlain ℓ o => s!"running={runningB σ ℓ} alive={aliveB σ o} plain={!σ.obligation o} handles={noHandleB σ n (fun i => decide (i = o))} guards={noGuardB σ (fun i => decide (i = o))} solitary={solitaryB σ o} owns={ownsB σ ℓ o}"
  | .bind ℓ o => s!"running={runningB σ ℓ} alive={aliveB σ o} frames={nonemptyB (σ.frames ℓ)} scopes={nonemptyB (σ.scopes ℓ)} temp={tempOfB σ ℓ o}"
  | _ => ""

/-- The step: refused when `check` says no. -/
def step (act : Act) (line : String) : M Unit := do
  let r ← get
  let n := r.maps.nextTid
  if check r.σ n act then
    set { r with σ := materialize (apply r.σ act) n, steps := r.steps + 1 }
  else
    throw s!"REFUSED {line} [{why r.σ n act}]"

def gap (f : State → State) : M Unit := modify fun r => { r with σ := f r.σ, gaps := r.gaps + 1 }

def mode (w : String) : M Mode :=
  if w = "1" then pure Mode.exclusive else pure Mode.shared

def nats (ws : List String) : M (List Nat) := ws.mapM nat

/-- One line: its action, with the identities it introduces mapped as the machine allocates them. -/
def line (text : String) : M (Option String) := do
  let ws := (text.splitOn " ").filter (· ≠ "")
  match ws with
  | [] => pure none
  | op :: args =>
    let next := (← get).σ.nextId
    match op, args with
    | "ben", [t, f] => step (.blockEnter (← getTid (← nat t)) ((← nat f) + 1)) text; pure none
    | "bex", [t] => step (.blockExit (← getTid (← nat t))) text; pure none
    | "sen", [t, s] => step (.stmtEnter (← getTid (← nat t)) ((← nat s) + 1)) text; pure none
    | "sex", [t] => step (.stmtExit (← getTid (← nat t))) text; pure none
    | "est", [t, o, r] =>
      step (.establish (← getTid (← nat t)) ((← nat r) = 1)) text
      mapObj (← nat o) next; pure none
    | "bnd", [t, o, a] =>
      step (.bind (← getTid (← nat t)) (← getObj (← nat o))) text
      mapTok (← nat a) next; pure none
    | "bnh", [t, o, a, h] =>
      let ho ← getObj (← nat h)
      mapObj (← nat o) ho
      step (.bind (← getTid (← nat t)) ho) text
      mapTok (← nat a) next; pure none
    | "brw", [t, a0, a, m, p] =>
      step (.borrow (← getTid (← nat t)) (← getTok (← nat a0)) (← mode m) ((← nat p) = 1)) text
      mapTok (← nat a) next; pure none
    | "prj", [t, a0, a, f] =>
      step (.project (← getTid (← nat t)) (← getTok (← nat a0)) (Proj.field (← nat f))) text
      mapTok (← nat a) next; pure none
    | "lnd", [t, a0, a] =>
      step (.lend (← getTid (← nat t)) (← getTok (← nat a0))) text
      mapTok (← nat a) next; pure none
    | "tak", [t, a] => step (.takePath (← getTid (← nat t)) (← getTok (← nat a))) text; pure none
    | "rd", [t, a] => step (.read (← getTid (← nat t)) (← getTok (← nat a))) text; pure none
    | "set", [t, a] => step (.settle (← getTid (← nat t)) (← getTok (← nat a))) text; pure none
    | "wr", t :: a :: n :: rest =>
      let k ← nat n
      let removed ← (rest.take k).mapM (fun w => do getTok (← nat w))
      match rest.drop k with
      | m :: rest2 =>
        let j ← nat m
        let added ← (rest2.take j).mapM (fun w => do getTok (← nat w))
        step (.write (← getTid (← nat t)) (← getTok (← nat a)) removed added) text; pure none
      | [] => throw "malformed wr"
    | "tmp", [t, o, a] =>
      step (.tempRoot (← getTid (← nat t)) (← getObj (← nat o))) text
      mapTok (← nat a) next; pure none
    | "bpa", [t, a, o, ar] =>
      step (.bindParam (← getTid (← nat t)) (← getTok (← nat a))) text
      mapObj (← nat o) next; mapTok (← nat ar) (next + 1); pure none
    | "xfr", [t, t2, a1, o1, o2, a2] =>
      let lo ← getObj (← nat o1)
      step (.transfer (← getTid (← nat t)) (← getTid (← nat t2)) (← getTok (← nat a1))) text
      mapObj (← nat o2) lo; mapTok (← nat a2) next; pure none
    | "rcl", [t, o, a] =>
      step (.reclaim (← getTid (← nat t)) false) text
      mapObj (← nat o) next; mapTok (← nat a) (next + 1); pure none
    | "rel", t :: _ :: os =>
      let ds ← os.mapM (fun w => do getObj (← nat w))
      step (.release (← getTid (← nat t)) ds) text; pure none
    | "dst", [t, o] => step (.destroy (← getTid (← nat t)) (← getObj (← nat o))) text; pure none
    | "epl", [t, o] => step (.endPlain (← getTid (← nat t)) (← getObj (← nat o))) text; pure none
    | "spn", [t, t2, f, h] =>
      let lt ← getTid (← nat t)
      let lt2 ← mapTid (← nat t2)
      step (.spawn lt lt2 ((← nat f) + 1)) text
      mapObj (← nat h) next; pure none
    | "tdn", t2 :: _ :: os =>
      let res ← os.mapM (fun w => do getObj (← nat w))
      step (.threadDone (← getTid (← nat t2)) res) text; pure none
    | "jon", [t, t2] => step (.join (← getTid (← nat t)) (← getTid (← nat t2))) text; pure none
    | "hds", [t, t2] => step (.handleDestroy (← getTid (← nat t)) (← getTid (← nat t2))) text; pure none
    | "fct", [t2] => step (.faultContain (← getTid (← nat t2))) text; pure none
    | "lck", [t, am, ag, g] =>
      step (.lock (← getTid (← nat t)) (← getTok (← nat am))) text
      mapTok (← nat ag) next; mapObj (← nat g) (next + 1); pure none
    | "gdp", [t, g] => step (.guardDrop (← getTid (← nat t)) (← getObj (← nat g))) text; pure none
    | "inv", [a] =>
      let la ← getTok (← nat a)
      gap (fun σ => { σ with paths := fun x => if x = la then (σ.paths x).map (fun p => { p with valid := false }) else σ.paths x })
      pure none
    | "rsc", [a, s] =>
      let la ← getTok (← nat a)
      let sc := (← nat s) + 1
      gap (fun σ => { σ with paths := fun x => if x = la then (σ.paths x).map (fun p => { p with scope := sc }) else σ.paths x })
      pure none
    | "out", rest => pure (some (" ".intercalate rest))
    | "end", _ => pure none
    | _, _ => throw s!"malformed {text}"

def replay (lines : List String) : Outcome := Id.run do
  let mut run : Run := { σ := σ0, maps := {} }
  let mut k := 0
  for l in lines do
    k := k + 1
    let ws := (l.splitOn " ").filter (· ≠ "")
    match ws with
    | [] => continue
    | w :: _ => if w.startsWith "#" then continue
    let (res, run') := (line l).run run
    run := run'
    match res with
    | .ok none => pure ()
    | .ok (some reason) => return .outside run.steps run.gaps reason
    | .error e =>
      if e.startsWith "REFUSED " then return .refused run.steps k (String.ofList (e.toList.drop 8))
      else return .malformed k e
  return .ok run.steps run.gaps

/-- `replay --time TRACE`: the lines one by one, with the milliseconds each took (for the machine's cost). -/
def timed (lines : List String) : IO Outcome := do
  let mut run : Run := { σ := σ0, maps := {} }
  let mut k := 0
  for l in lines do
    k := k + 1
    let ws := (l.splitOn " ").filter (· ≠ "")
    match ws with
    | [] => continue
    | w :: _ => if w.startsWith "#" then continue
    let t0 ← IO.monoMsNow
    let (res, run') := (line l).run run
    run := run'
    let t1 ← IO.monoMsNow
    IO.println s!"{t1 - t0} ms  {l}"
    match res with
    | .ok none => pure ()
    | .ok (some reason) => return .outside run.steps run.gaps reason
    | .error e =>
      if e.startsWith "REFUSED " then return .refused run.steps k (String.ofList (e.toList.drop 8))
      else return .malformed k e
  return .ok run.steps run.gaps

def main (args : List String) : IO UInt32 := do
  match args with
  | ["--time", path] =>
    let text ← IO.FS.readFile path
    let o ← timed (text.splitOn "\n")
    IO.println s!"replay {path}: {o.text}"
    pure 0
  | [path] =>
    let text ← IO.FS.readFile path
    let o := replay (text.splitOn "\n")
    IO.println s!"replay {path}: {o.text}"
    match o with
    | .refused .. => pure 1
    | .malformed .. => pure 2
    | _ => pure 0
  | _ =>
    IO.eprintln "usage: replay [--time] TRACE"
    pure 2
