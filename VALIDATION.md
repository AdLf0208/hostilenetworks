# Validation — 0.2.3 (SDK package, mod API 0.2)

Validated on Linux with rustc 1.99.1 on 2026-11-21 against the campaign pin
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` (mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, game 26.3, `package_format` 6).
Every compile/test step ran inside a `ports/tools/build-slot.sh` lease with
`CARGO_INCREMENTAL=0`, in the port, with no `--target-dir`, `BUILD_LOCAL=1`
(the port's `.horizon/sdk` is fetched per clone and is not on the far box).

| Check | Result |
|---|---|
| `.horizon/sdk` refetched after the repin | `SDK 0a875da (build 0a875da91dbb…) in ./.horizon/sdk` |
| red leg, the new training tests | 2 failed / 19 passed, `Exit 101` (`ports/_shared/evidence/hostilenetworks/sdk-test-red-training.log`) |
| `cargo test --offline` on the final sources | **31 passed, 0 failed** (16 runtime rules + the new training rules, 10 glue) |
| `cargo clippy --all-targets --offline -- -D warnings` | exit 0, 0 warnings |
| `cargo fmt --check` (not `--all`) | exit 0 |
| `rm -f Cargo.lock && cargo horizon-mod build` | `Built ./target/horizon-mod/hostilenetworks.hmod` |
| second build after removing the lock | byte-identical (`CMP:identical`), no `Cargo.lock` packed |
| `cargo horizon-mod check hostilenetworks.hmod` | `hostilenetworks 0.2.3 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, 64,607 B, sha256 `8843ad11…c432`, exit 0 |
| `cargo horizon-mod fits hostilenetworks.hmod --build 0a875da` | `0a875da  made for`, exit 0 (the pin run is a tautology) |
| `cargo horizon-mod fits hostilenetworks.hmod --build 0b14f8a` | `0b14f8a  fits`, exit 0 |
| `python3 ports/tools/verify-sdk-package.py hostilenetworks` | **1/1** |
| `python3 ports/tools/check-skills.py --notes ports/hostilenetworks --strict` | `Skills line present`, PASS |

## In game (launcher build at `0a875da`)

`ports/tools/launcher-game.sh build hostilenetworks` → `Compiling hostilenetworks (1 of 1)`
(the manifest version changed, so the launcher could not reuse an old build), then one leased
`tools/e2e.sh`:

| Leg | Result |
|---|---|
| default | `PASS` + `EXIT:0` — the whole loop, training mode, all three recipes, all items, both loot tables |
| control (same Nightly **without** the package) | `FAIL: the fabricator is minecraft:air (did the package load?)` + `EXIT:1` |
| `SELFTEST=1` | `FAIL: expected 1 x hostilenetworks:overworld_prediction …; got 0` + `EXIT:1` |
| `NEGATIVE=1` (training counterfactual) | `FAIL: NEGATIVE: training upgraded the model to 21, so the assertion is alive` + `EXIT:1` |
| `python3 ports/tools/check-e2e-logs.py hostilenetworks-e2e --strict` | `logs=4  ok=4` |

Evidence: `ports/_shared/evidence/hostilenetworks/sdk-{test,clippy,fmt,check,fits,fits-other}.log`
(each printing the package size and sha256 it ran against) and
`ports/_shared/evidence/hostilenetworks-e2e/e2e-launcher{,-control,-selftest,-negative}.log`.

## Not asserted

- **Client visuals** (the carried GUI textures, the model-rendering item, lang): loaded by the
  game, not asserted — no renderer claim is made. The screens' text and slot layouts are asserted
  through `ctl screen`/`ctl ui`.
- **The creative tab**: carried, and `horizon-ctl` has no creative-tab read.

## Historical

`VALIDATION.md` of 0.1.0 (the staged `.hnmod` ladder: `test_template`,
`test_stage`, `horizon.py stage/check/test/build`) moved to
`ports/_shared/evidence/hostilenetworks/retired-hnmod-0.1.0/VALIDATION.md` with
the rest of the retired tree. 0.2.0's ladder is in `notes.md` under "Migration to the SDK format".
