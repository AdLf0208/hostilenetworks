# Validation — 0.2.0 (SDK package, mod API 0.2)

Validated on Linux with rustc 1.97.0 on 2026-11-18 against the pinned SDK
`0b14f8acfd97fa2c750e42795249dcc981178ca4` (mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, game 26.3, `package_format` 6).
Every step ran inside a `ports/tools/build-slot.sh` lease with
`CARGO_INCREMENTAL=0`, in the port, with no `--target-dir`.

| Check | Result |
|---|---|
| `cmp src/runtime.rs` against the retired bundle's copy | identical (sha256 `4f00bd87…a723`, 23,819 B) |
| `cargo test --offline` before any glue existed | 16 passed, 0 failed |
| `cargo test --offline`, glue stubbed (red leg) | 16 passed, 10 failed (`not yet implemented`) |
| `cargo test --offline` on the final sources | **26 passed, 0 failed** (16 `runtime::`, 10 `support::`) |
| `cargo clippy --all-targets --offline -- -D warnings` | exit 0, 0 warnings |
| `cargo fmt --check` (not `--all`) | exit 0, empty diff |
| `rm -f Cargo.lock && cargo horizon-mod build` | `Built ./target/horizon-mod/hostilenetworks.hmod` |
| second build after removing the lock | byte-identical (`cmp` clean) |
| `cargo horizon-mod check hostilenetworks.hmod` | `hostilenetworks 0.2.0 (native-mod, format native-1/0b14f8a…/e59da90…, for 26.3)`, 62,111 B, sha256 `dfeeb23f…9ba7c`, exit 0 |
| `cargo horizon-mod check hostilenetworks.hmod --json` | `{"ok":true,…,"kind":"native-mod","size":62111,"version":"0.2.0"}` |
| `cargo horizon-mod fits hostilenetworks.hmod --build nightly` | `0b14f8a  made for`, exit 0 |
| `unzip -p hostilenetworks.hmod src/runtime.rs \| sha256sum` | `4f00bd87…a723`, the retired file byte for byte |
| `python3 ports/tools/check-skills.py --notes ports/hostilenetworks --strict` | `Skills line present`, PASS |

Evidence: `ports/_shared/evidence/hostilenetworks/sdk-*.log`, `sdk-uses*.{json,log,txt}`,
`sdk-sha256.txt`, `sdk-entries.txt`.

## Not run in this environment

- **The in-game e2e.** It needs a game the launcher built with this package
  (`horizon-launcher native-build ports/hostilenetworks --instance <i>
  --trust-native-code`) and a matching `horizon-ctl`; neither is installed here, and
  the campaign's shared binaries are pinned at `1d63e0a5`/`41f822b6`, not
  `0b14f8a`. `tools/e2e.sh` is updated for that launch (GAME=, optional
  CONTROL_GAME=, SELFTEST=1) and no 0.2.0 in-game claim is made. The 0.1.0 runs
  (`e2e.log`, `e2e-control.log`, `e2e-negative.log`) remain the behaviour's
  evidence for a `runtime.rs` that did not change.
- **A launcher native compile** (`--trust-native-code`). The compile gate this
  slice has is `cargo test` + `clippy -D warnings` against the pinned SDK API
  crate, the same crate the launcher builds against.

## Historical

`VALIDATION.md` of 0.1.0 (the staged `.hnmod` ladder: `test_template`,
`test_stage`, `horizon.py stage/check/test/build`) moved to
`ports/_shared/evidence/hostilenetworks/retired-hnmod-0.1.0/VALIDATION.md` with
the rest of the retired tree.
