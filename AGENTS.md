# Working on this port (SDK package, mod API 0.2)

This is a flat Horizon `.hmod` package: `horizon-mod.toml` + `Cargo.toml` +
`src/lib.rs` (the one `impl mod_api::Hooks`) + `src/runtime.rs` (the pure rules,
byte-identical to the retired `.hnmod` bundle's copy, tests included) +
`assets/` + `data/`. The SDK is fetched into `.horizon/sdk/` (gitignored) and
never packed.

The retired `.hnmod` layout (`mixins/`, per-hook helper `.rs` files,
`horizon-mixin.toml`, the injection plan, `compatibility.json`,
`rust-toolchain.toml`, `tools/horizon.py`, `tools/package.py`) is gone and must
not come back; its tree is kept as evidence in
`../_shared/evidence/hostilenetworks/retired-hnmod-0.1.0/`.

## Rules

- **`src/runtime.rs` never changes.** Every gameplay rule lives there and is
  tested there; the glue calls it. Prove it with
  `cmp src/runtime.rs ../_shared/evidence/hostilenetworks/retired-hnmod-0.1.0/mixins/hostilenetworks/runtime.rs`
  after any `cargo fmt`.
- The glue is one Part per machine (block entity, tick, screen, draw, click,
  menu button) plus declarations (container, energy, block entity type). Do not
  reintroduce per-hook files.
- **Every build, test and lint runs in a `ports/tools/build-slot.sh` lease**, with
  `CARGO_INCREMENTAL=0`, `cd` inside the wrapped command, no `--target-dir`, no
  private `BUILD_SLOTS_DIR`. See `../_shared/build-slots-usage.md`.
- `cargo fmt --check` (never `--all`: that reports the fetched SDK), then
  `cargo clippy --all-targets -- -D warnings`, then `cargo test`.
- `rm -f Cargo.lock` immediately before `cargo horizon-mod build`; a packed lock
  changes the artifact. `Cargo.lock` is in `.gitignore`.
- The deliverable is `hostilenetworks.hmod` in this directory, built with
  `cargo horizon-mod build` and copied from `target/horizon-mod/`.
- Local-only until the maintainer approves publishing (ARCHITECTURE.md §8).
