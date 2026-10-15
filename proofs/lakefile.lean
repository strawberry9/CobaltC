import Lake
open Lake DSL

package cobalt where
  -- no Mathlib: the development uses the core library only, so that one
  -- toolchain download builds it

@[default_target]
lean_lib Cobalt where
  roots := #[`Cobalt.State, `Cobalt.Step, `Cobalt.Invariants, `Cobalt.Preservation, `Cobalt.Corollaries]
