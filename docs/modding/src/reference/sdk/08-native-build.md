# `native-build`

`cargo sunrise-mod native-build` compiles native mods against one build's SDK exactly as the launcher does: both run
the same code (`crates/sunrise-native-build`). Every SDK release carries the tool for each host it is built for, as
`tool-<target>.tar.xz` holding `cargo-sunrise-mod` (`.exe` on Windows), so a caller that already checked the SDK's
signed `sdk.json` gets a tool made from that build's own source.

The caller does what needs a player or their game: it checks that native code is allowed, fetches and verifies the
SDK's slices, installs the pinned Rust, chooses the mods and which of them a build leaves out, and installs what the
tool built. The tool compiles, and nothing else.

It reads a plan from the first line of standard input, writes one JSON event per line to standard output, and appends
its log to `build.log` in the plan's `work` folder: emptying it before a build is the caller's, as the launcher's own
lines about the build go there first.

```sh
cargo sunrise-mod native-build < plan.json
```

| Exit code | Meaning |
|---|---|
| 0 | It built: the last line is a `built` event. |
| 1 | It did not: the last line is a `failed` event. |
| 2 | No plan: the first line is not one, or names another contract. Standard error says why. |

## The plan

One JSON object on one line. A field the tool doesn't know is refused, and so is any `contract` but `1`.

| Field | What it is |
|---|---|
| `contract` | `1`. A change to any field or event below takes a new number. |
| `work` | The build's own folder: `build.log`, the horizon-mixins tool, the game source a mixin patch copies from, and kept compiles under `prebuilt-build/`. The launcher passes its `native/` folder, so the kept compiles are the ones its in-process build made. |
| `toolchain` | `dir` (holding `bin/cargo` and `bin/rustc`), `target` (its host triple), `cargo_home`, `cargo_config` (`--config` values every cargo command gets; the launcher's Windows ARM64 flags) and `c_bin` (the folder of its C and link programs, when it brings them). The Rust release must be the SDK's (`sdk.json`'s `toolchain.rust`). |
| `package` | `runtime`, `api` and `engine`: the folders the SDK's three slices for this host and side are unpacked into. The API route reads only `runtime` and `api`. |
| `mods` | A mod's folder, or a folder of mod folders, read in path order. Every mod in it is compiled: a caller leaves a mod out by not putting it there. |
| `target` | `client` (the game) or `server` (the headless engine). |
| `registry` | Optional. `true` or `false` answers in advance whether registry crates may be added (their build scripts and procedural macros run while compiling). Absent, the tool asks with a `consent` event. |
| `mixins_from_source` | Optional, `false` by default. Builds horizon-mixins from its pinned source instead of downloading its release. |

## The events

Each line is an object whose `event` names it.

| Event | Fields | When |
|---|---|---|
| `mod` | `id`, `done`, `total` | A mod's own library starts compiling, `done` of `total`. |
| `consent` | `added_crates`, `build_scripts`, `proc_macros` | Registry crates need a yes and the plan gave no `registry`. The caller answers with one line on standard input: `true` agrees, anything else refuses. |
| `built` | `route`, `binary`, `libraries`, `records`, `mods` | The executable and the libraries to install beside it, in load order; `records` is each library's identity, the build record's `libraries`; `route` is `prebuilt-api` or `prebuilt`, as the record names it; `mods` is the compiled mods in dependency order. |
| `failed` | `error` | Why it did not build, as the build log ends with it. A mod blamed by name in it (`Compiling <mod> failed`, `E_NATIVE_BUILD_SCRIPT: <mod>`) is how a caller learns which mod to leave out next time. |

The tool never edits an instance, never selects a version and never installs anything outside `work`.
