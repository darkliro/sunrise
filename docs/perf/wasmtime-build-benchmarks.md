# Wasmtime and Cranelift build-time benchmarks

Screens the levers in the wasm subsystem's build (`crates/sunrise-wasmtime`, `crates/sunrise-mod-wasm`, the
`cranelift-*`/`wasmtime` crates) so nobody measures the same neutral ones again: every option below was measured
and none of them is worth a change. The measurement's outcome is the last section: the time is not in wasmtime.

## Method

- A cold build per row: a target folder of its own, removed before, never reused (`CARGO_TARGET_DIR`).
- Wall clock from `date +%s%N` around `cargo`; CPU from `/usr/bin/time` where a row says so.
- The same host for every row, nothing else building. Per-run wall-clock noise on this host is about ±5 %
  (the baseline measured 174.4 s once and 176.0 s in the `--timings` rerun).
- Commands: `cargo build -p sunrise-wasmtime` and `cargo check -p sunrise-mod-wasm`. The second is the closer
  number for the wasm subsystem's whole first build.

## The host

`rustc 1.100.0`, 12 logical CPUs — an Intel i5-1334U, 2 P-cores and 8 E-cores — 30 GB RAM, `CARGO_BUILD_JOBS`
default, `-C target-cpu=native` from `~/.cargo/config.toml`. Parallel speedup of a check here is about 7.7×
CPU to wall (483 s CPU in 62.8 s), so an E-core counts for a fraction of a P-core: absolute numbers on a
many-P-core CI box will be lower, while the *comparisons* hold.

## Measured

| Command | Wall | Note |
|---|---|---|
| `cargo build -p sunrise-wasmtime` (cold) | **62.8 s** | 483.8 s CPU, 23 cranelift/wasmtime/pulley units, 2.2 GB peak RSS |
| `cargo check -p sunrise-mod-wasm` (cold, baseline) | **174.4 s** | the number every row below is compared with |

### Profile options

| Change | Wall | Verdict |
|---|---|---|
| `opt-level = 0` for `wasmtime` + `cranelift-codegen` | 177.8 s | no gain |
| `opt-level = 1` | 183.9 s | worse |
| `opt-level = 2` | 181.2 s | worse |
| `opt-level = 3` (current) | 174.4 s | keep |
| `codegen-units = 1` for `cranelift-codegen` | 172.4 s | inside noise; a 50 s crate with 15 s frontend and 35 s codegen does not care |
| `codegen-units = 16` | 174.8 s | none |
| `codegen-units = 64` | 178.3 s | worse |
| `codegen-units = 256` | 185.8 s | worse |
| `wasmparser` `opt-level = 0` | 180.3 s | worse: the validator validates real modules in the fixture tests |
| `wasmparser` `opt-level = 1` | 184.9 s | worse |
| `wasmparser` `opt-level = 2` | 181.8 s | worse |
| `wasmparser` `opt-level = 3` (current) | 174.4 s | keep |

So the maintainers' `opt-level = 3` for `wasmtime`, `cranelift-codegen` and `wasmparser` in the root
`Cargo.toml` is the fastest setting, not a compromise: lowering it costs time.

### Compiler and environment levers

| Change | Wall | Verdict |
|---|---|---|
| `-Z share-generics=y` (`RUSTC_BOOTSTRAP=1`) | 178.8 s | worse |
| `-Z threads=8` (now `--jobs-frontend`) | 181.1 s | worse |
| `CARGO_BUILD_JOBS=2` | **269.9 s** | +55 %: the E-cores are needed, do not cap jobs on this class of CPU |
| `RUSTC_WRAPPER=sccache`, cold cache | 365.8 s | 182 Rust misses, 0 hits |
| `sccache` with a populated cache, fresh target | 207.0 s | **0 Rust hits**: a `cargo check` is served by Cargo's own fingerprint and rustc's incremental cache, not by sccache |
| Edit one `.rs` of `sunrise-mod-wasm`, then `cargo check` (warm target) | **2.0 / 2.1 / 2.3 s** | the day-to-day cost is already tiny; a cold build is not a per-edit cost |

CI already routes rustc through sccache (`tools/ci/compiler-cache.sh`, `dyna-ci.yml`), so nothing is to add
there either.

### Features

- `pulley` stays: `crates/sunrise-mod-wasm/tests/pulley_policy.rs` (the daily `wasm-mods` job) runs a guest under
  `SUNRISE_FIXTURE_TARGET=pulley64`, and `runtime.rs::guest_engine()` reads `Engine::is_pulley()` to settle the
  guest stack policy.
- `parallel-compilation` stays: it is `rayon` only (≈2.4 s), `rayon` is in the workspace through
  `sunrise-client-lib` anyway, and `compile_budget.rs` derives its default compile budget from parallel
  compilation. Dropping both `parallel-compilation` and `pulley` measured 169.6 s, a 4.8 s difference that the
  `pulley` break alone explains and that no full game build can see.
- No foreign architecture is compiled: `cranelift-codegen` builds with `host-arch` only (no `all-arch`). The
  s390x and riscv64 backends are not in the build.

## Where the time actually goes

A `cargo check -p sunrise-mod-wasm --timings` of a cold target, sorted by the unit's total time:

| Unit | Seconds |
|---|---|
| `sunrise-mod-points` build-script (run) | 59.8 |
| `sunrise-mod-marks` build-script (run) | 46.5 |
| `sunrise-mod-api-gen` | 43.0 |
| `sunrise-mod-vocab` build-script (run) | 42.6 |
| `sunrise-mod-wasm` build-script (run) | 37.6 |
| `sunrise-engine-slots` build-script (run) | 36.3 |
| `syn` 3 + `syn` 2 | 33.2 |
| `cranelift-codegen` (check) | 16.1 |
| `cranelift-assembler-x64` (check) | 14.9 |
| `wasmtime` (check) | 7.6 |
| `wasmparser` (check) | 6.8 |

Sums over the profile: about 367 s in Sunrise workspace units against about 183 s in the wasmtime/cranelift
ecosystem. The five largest units are build scripts that each walk the same 756 files, 21 508 lines, of
`crates/sunrise-mod-vocab/src/hooks` with `syn`, in five separate processes, and write large files
(`points_generated.rs` 23 MB/220 048 lines, `engine_points.rs` 16.5 MB/117 042 lines,
`wire_host_generated.rs` 2.2 MB, `mod_api_generated.rs` 1.7 MB). The walk cache in
`sunrise-mod-api-gen/src/source.rs` and the interface cache in `interface_cache.rs` are per process, so they
cannot share that work; they only make a *rerun* cheap, which is why an edit costs 2 s.

Cranelift is not the bottleneck of this subsystem: it is about a quarter of the units and is paid once per
cold build. The next lever is the generator pipeline, a change in `sunrise-mod-api-gen` and the five
build scripts, not in a Cargo profile.

## Reproduce

```sh
CARGO_TARGET_DIR=$(mktemp -d) cargo check -p sunrise-mod-wasm --timings
# the report lands in $CARGO_TARGET_DIR/cargo-timings/cargo-timing.html
```

Compare wall clocks only on a quiet machine: one `rustc` beside the run, or a second build, shifts them by
more than the differences in this page. `tools/build_times.py` (`docs/build-times.md`) is the tool for
the whole game; this page is only the wasm subsystem's screening.
