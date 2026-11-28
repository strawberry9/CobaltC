# CHG-0208 — A PostgreSQL client in `std::database`

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-07; D-0180)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0180
Affects: `spec/21` (4.43.0) §0, §2q; `spec/conformance.md` (3.173.0); the guide; `impl/std/database.cb`, `impl/std/std.cb`, `src/prelude.rs`

## What changed

- **`spec/21` §2q `rule.stdlib.postgres`:** `[Pg-Url]`, `[Pg-Connect]`,
  `[Pg-Auth]`, `[Pg-Query]`, `[Pg-Result]`, `[Pg-Execute]`, `[Pg-Script]`,
  `[Pg-Notice]`, `[Pg-Error]`; the listing.
- **`spec/21` §0:** `std::database` in the module table; `PgUrl`,
  `PgSslMode`, `PgValue`, `PgColumn`, `PgRow`, `PgResult`, `PgConnection`,
  `PgError`, `PgServerError`, `Scram`, `scram_salted_password` in the
  items table.
- **Rows:** `conf.scram-sha256-vectors`, `conf.postgres-url`,
  `conf.postgres-values`, `conf.postgres-loopback`.

## Compatibility classification

Additive.
