# CHG-0209 — The small holes of the `std` completeness round

Kind: Change record (`spec/02-schema.md` §2)
Status: ACCEPTED (2026-10-07; D-0181)
Governed by: `CobaltC_Master_Instructions.md` §13
Depends on: D-0181
Affects: `spec/21` (4.44.0) §0, §1, §1b, §2c, §2d, §2e, §2e′, §2h, §2j, §2k, §2l, §2m, §2o, §2p, §2q, §3d; `spec/conformance.md` (3.174.0); the guide; `impl/std/` (thirteen files), `src/`, `cobc/`, `cbrt`

## What changed

- **`spec/21` §2h `rule.stdlib.stringview`:** `[Strip-Prefix]`, `[Lines]`, `[Split-Whitespace]`, `[Code-Point]` (`StringView::strip_prefix`, `strip_suffix`, `lines`, `split_whitespace`, `code_point`, `String::push_code_point`, and the `String::` twins).
- **§2d `rule.stdlib.text`:** `[Parse-Bool]`: `parse<bool>`.
- **§1 `rule.stdlib.vec`:** `[Retain]`, `[Dedup]`. **§1b `rule.stdlib.sort`:** `[Sort-Float]`: `f32`/`f64` in `Vec::sort`, `binary_search`, `PriorityQueue::new`, a NaN last.
- **§0 (`std::math`):** `clamp`, `is_nan`, `is_finite`, `gcd`; **§3d `[Float-Transcendental]`:** `asin`, `acos`, `atan`.
- **§0 (`std::random`):** `[Rng-Shuffle]`, `[Uuid-V4]`.
- **§2e `rule.stdlib.file-handle`:** `[Create-New]`, `[Flush]`, `[Position]`.
- **§2e′ `rule.stdlib.env`:** `[Current-Exe]`, `[Hostname]`.
- **§2k `rule.stdlib.process`:** `[Process-Id]`, `[Child-Terminate]`; `[Child-Streams]` gains `read_error_line`.
- **§2j `rule.stdlib.time`:** `[Iso-Ms]`, `[Http-Date]`; `[From-Iso]` reads a fraction.
- **§2l `rule.stdlib.net`:** `[Keepalive]`, `[Udp-Connect]`.
- **§2m `rule.stdlib.crypto`:** `[Sha1]`; `HashKind` gains `Sha1`.
- **§2o `rule.stdlib.tls`:** `[Tls-Read-Line]`, `[Tls-Peer-Certificate]`; `TlsStream::peer_addr`, `local_addr`.
- **§2p `rule.stdlib.http`:** `[Form]`: `Param`, `find_param`, `form_decode`, `form_encode`, `Url::query_pairs`.
- **§2q `rule.stdlib.postgres`:** `[Pg-Cell]`: `PgRow::get_i64`, `get_f64`, `get_bool`, `get_bytes`.
- **§0's tables** list every item; the listings of §2j, §2k, §2l, §2m, §2o, §2p, §2q are regenerated from the source.
- **Rows:** `conf.text-strip-prefix-suffix`, `conf.text-lines-words`, `conf.text-code-points`, `conf.parse-bool`, `conf.vec-retain`, `conf.vec-dedup`, `conf.vec-sort-floats`, `conf.math-clamp`, `conf.math-nan-finite`, `conf.math-gcd`, `conf.float-inverse-trig`, `conf.rng-shuffle`, `conf.uuid-v4`, `conf.time-iso-ms`, `conf.time-http-date`, `conf.file-create-new`, `conf.file-flush-position`, `conf.env-current-exe`, `conf.env-hostname`, `conf.process-id`, `conf.child-terminate`, `conf.child-read-error-line`, `conf.tcp-keepalive`, `conf.udp-connect`, `conf.sha1`, `conf.hmac-sha1`, `conf.tls-read-line`, `conf.tls-peer-certificate`, `conf.http-forms`, `conf.postgres-typed-cells`.

- **Rows re-pointed**, since what they refused now reads or orders: `conf.parse-not-numeric` (`parse<String>` in place of `parse<bool>`), `conf.sort-not-key-type` (a struct with an `f64` field in place of `f64`), `conf.priority-queue-not-key` (`Vec<u8>`), `conf.priority-queue-generic-key-checked` (`Vec<i32>`).

## Compatibility classification

Additive, except that `HashKind` gains a variant (an exhaustive `match` on it in a program gains an arm) and `DateTime::from_iso` accepts a seconds fraction it refused.
