# `sunrise-prepare` needs, missing-extras and extras

Three questions a launcher asks a version's own `sunrise-prepare` instead of linking the preparer: what preparing
will need, which extras prepared resources lack, and adding them. With them, and with `--prepare --progress json`
and `sounds` ([launch.md](launch.md#preparing)), a launcher runs the preparer that shipped with each version and
never compiles one in.

## Overview

| | |
| --- | --- |
| Format | `"format": 1` in every reply |
| Level | experimental ([stability.md](stability.md)): its first user is the launcher, after the launcher split |
| Covers | `sunrise-prepare needs`, `missing-extras` and `extras` with `--json`: the replies, the message lines and the exit codes |
| Not in | the Launch API's `--launch-api` options: these are subcommands, as `sounds` and `fonts` are |

```sh
sunrise-prepare needs --json
sunrise-prepare missing-extras --json --output PREPARED
sunrise-prepare extras --json --output PREPARED --minecraft-dir ~/.minecraft --minecraft-dir ~/other-launcher
```

Each takes `--root PATH` (default: the folder the program is in, else the nearest folder above the current one with
`research-lock.json`). Without `--json` each prints the same answer for a person.

## Reference

### `needs --json`

```json
{"format": 1, "server_jar": true, "jdk": false}
```

- `server_jar`: no verified import of the pinned jars is there to reuse, in `--root` or in a sibling version
  folder, so the first `--prepare` needs `--server-jar` (which a launcher downloads only for an owner of the game).
- `jdk`: the version ships no matching exporters, so preparing compiles them and `--java-home` must give a JDK, not
  only a runtime.
- Reusing a sibling's import links it into `--root`, as a preparation would; its lines go to stderr.
- Exit 0, or 1 with an error reply.

### `missing-extras --json`

```json
{"format": 1, "resources": "/home/a/.local/share/sunrise/versions/nightly/.local/sunrise-client/26.3", "prepared": true,
 "missing": [{"kind": "sounds", "what": "the sounds"}]}
```

- `resources`: the folder asked about, `--output` or `--root`'s `.local/sunrise-client/<version>`.
- `prepared`: whether it holds a preparation (`sunrise-resources.json`). When it doesn't, `missing` lists every
  extra and preparing comes first.
- `missing`: the extras it lacks, in the order `extras` adds them. `kind` is one of `packs`, `panorama`, `fonts`,
  `languages`, `sounds`, the same as `sunrise-prepare <kind>`; `what` names it for a person. It is the same test
  `--check` makes for extras: which files are there, not their contents.
- Exit 0 whatever is missing, or 1 with an error reply. It reads files only.

### `extras --json`

Adds what the resources lack, each from the first `--minecraft-dir` whose asset store has it (none given: the
official launcher's folder, or `SUNRISE_MINECRAFT_DIR`). `--assets-dir` and `--version-json` name a store for every
folder. Nothing is downloaded and nothing else is prepared.

- `--only packs,fonts`: only these kinds. An unknown kind exits 2.
- Every line on stdout is one JSON object. `{"message": string}` lines come first, as `--progress json`'s do; the
  last line is the reply:

```json
{"format": 1, "added": ["packs", "fonts"], "failed": [{"kind": "sounds", "error": "…"}],
 "missing": [{"kind": "sounds", "what": "the sounds"}]}
```

- `added`: the kinds it added. When it added any, it also removed the game's cache of prepared data
  (`sunrise-data.bin`), which would otherwise keep answering that they are missing.
- `failed`: each kind no folder could give, with the first folder's reason.
- `missing`: what the resources still lack afterwards, of every kind, as in `missing-extras`.
- Exit 0 when nothing failed, 1 when an extra stayed missing (with the reply) or on an error reply.
- Resources that aren't prepared are an error: prepare them first.

### Errors and exit codes

- An error reply, the last line on stdout, with `error:` and the same text on stderr:
  `{"format": 1, "error": "…"}`.
- Exit 2: a refused command line, with no JSON. That includes `--prepare`, `--server-jar`, `--check`,
  `--progress` and `--launch-api` given to these commands.

### Reading them

- Readers ignore fields and line keys they don't know.
- A reply whose `format` is newer than the reader knows is refused: the reader says to update the launcher.

## Stability

Experimental. A new field, line key or extra `kind` keeps `format`; a changed or removed one makes the next
`format`, and the release contract in `sunrise-release.json` moves with it. `extras::tests` and the `extras_*` tests
in `crates/sunrise-prepare/src/sounds.rs` pin the replies and what `extras` adds.

## Changelog

- format 1: `needs`, `missing-extras` and `extras` with `--json`.
