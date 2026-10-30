import Lake
open Lake DSL

package cobalt where
  -- no Mathlib: the development uses the core library only, so that one
  -- toolchain download builds it

@[default_target]
lean_lib Cobalt where
  roots := #[`Cobalt.State, `Cobalt.Step, `Cobalt.Invariants, `Cobalt.Preservation, `Cobalt.Corollaries, `Cobalt.Elision, `Cobalt.Exec]

-- D-0270: `lake exe replay TRACE` runs a model-checker trace through the executable machine.
@[default_target]
lean_exe replay where
  root := `Replay
