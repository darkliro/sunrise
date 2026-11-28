# From 1.7.10 coremods and ASM to hooks and mixins

A 1.7.10 coremod patched compiled Java classes while the game loaded: a class transformer rewrote method
bodies, redirected calls, added fields or replaced whole classes. Sunrise loads no Java and patches no
class files. A port does the same job with the mod API in this order:

1. a **hook** or **Part method** when the game already exposes the moment;
2. a **[mixin](../../concepts/native-route/2-mixins.md)** row when it does not, for one named function;
3. a **[bridge](../../concepts/native-route/4-bridges.md)** or a
   **[support module](../../concepts/native-route/5-support-modules.md)** when the helper needs a private
   field, method or shared code inside the game's module.

A mixin changes the game function itself, so it is the closest thing to a transformer. It is still a
declaration in `sunrise-mod.toml`, compiled against one exact game build, not bytecode edited at runtime.
Use a hook whenever one exists: hooks are the stable API and keep the game's own control flow.

## The mapping

| 1.7.10 coremod or ASM | Sunrise route |
| --- | --- |
| `@Mod.EventHandler`, `TickEvent`, `PlayerEvent`, a listener registered in `init` | A hook in `impl Hooks` (`player_logged_in`, `server_tick`, `block_break`, …). The hook's rustdoc names its side, event and NeoForge counterpart. See [Hooks and the Host](../../concepts/hooks-and-the-host.md). |
| `IClassTransformer` replacing a whole method | `[[native.mixin]]` with `kind = "OVERWRITE"` and the function's `fingerprint`. One mod per function; the replacement must own the function's full behaviour. |
| Redirect of one method or field call inside a method | `kind = "BEFORE_CALL"`, `AFTER_CALL` or `WRAP_OPERATION` with the call as `selector`. `WRAP_OPERATION` can run, skip or replace the call. |
| Change to a value a method returns | `kind = "RETURN"` on the returned value, or `MODIFY_EXPRESSION_VALUE` on one call's value. |
| Change to an argument, constant or local | `MODIFY_ARG`/`MODIFY_ARGS`, `MODIFY_CONSTANT`, or `MODIFY_VARIABLE`. |
| Read or write of a private field (`ObfuscationReflectionHelper`) | A `[[native.bridge]]` row with `GET`, `GET_MUT` or `SET`, called from the helper. |
| Call to a private method | A `[[native.bridge]]` row with `INVOKE`. |
| Helper code shared by several injections, needing private game items | A `[[native.module]]` support module in the game module's mirror. |
| A field added to a vanilla class | An **attachment** on the holder (entity, player, block entity, chunk, level). `[[native.state]]` is refused: adding a field changes the prebuilt game's layout. See [Attachments](../../concepts/attachments.md). |
| A hot loop patched for every entity, block entity or chunk | A published loop (`[[native.loop]]`) when the game publishes that loop, or a mixin on the loop body. See [Published loops](../../concepts/native-route/3-loops.md). |
| Bytecode generated with ASM rather than a fixed patch | No route. Declare the behaviour as data, a Part or a hook; a runtime that generates classes has no equivalent. |
| Deobfuscated Minecraft name from a mapping file | The **named target** from this game build: the module path, type and function as the source writes them. Below the gameplay crates, a `#[replaceable]` marker and the `internals.txt` catalogue decide what is targetable. See [Naming the target](../../concepts/native-route/2-mixins.md#naming-the-target). |
| Several coremods patching the same method, ordered by load order | An explicit `priority` per row: higher runs first, default 100. The builder plans every selected mod together, so a conflict or an `OVERWRITE` shared by two mods fails that build instead of loading silently. |

## A hook replaces the event patch first

A coremod that only observed an event or a tick becomes an ordinary hook. The event carries the values the
old patch read, and the host gives the writes the hook is allowed to make.

{{#include ../../../examples/src/coremod_hooks.rs:hook}}

## A mixin replaces the call redirect

When a transformer redirected one call, a mixin row names the function, the place and the helper. The row is
data: no class loader, no transformer list and no access transformer file.

```toml
[[native.mixin]]
id = "redirect-call"
target = "<the game function the old coremod patched>"
kind = "BEFORE_CALL"
selector = "<the call the old coremod redirected>"
helper = "src/mixins/redirect.rs"
bindings = [{ name = "game", ty = "&mut crate::game::ServerGame<'_>", value = "&mut *self" }]
```

```rust,ignore
fn redirect(game: &mut crate::game::ServerGame<'_>) {
    // Runs at the selected call, in the target's module, with its private items in scope.
    let _ = game;
}
```

Read [Mixins](../../concepts/native-route/2-mixins.md) for every kind and binding, and the pinned
`horizon-mixins` documentation for tool-specific syntax. A helper is private, safe, non-generic and has no
`use` lines; it sees the target module's items and private fields, and calls the mod's own crate by name.

## What has no equivalent

The native route deliberately refuses transformations that a class transformer could perform. A port that
needs one of these needs a hook request, a Part or a game change, not a workaround:

- **Generics, `async`, `unsafe`, `const`, `impl Trait` or `!` in the target.** A mixin targets one concrete
  function; only a published loop's body may be generic. Rewrite the behaviour as a hook, or target a
  replaceable caller.
- **A selector inside a closure or a macro the tool cannot validate.** Select a site outside the closure, use
  `HEAD` to leave the body intact, or hook a call the function makes.
- **Replacing a class or its loader, adding variants or fields, or rewriting a signature.** The prebuilt game's
  layout is fixed. Use an attachment for new state and a hook for new behaviour.
- **An implicit `?` return that `RETURN` cannot cover.** Use `HEAD`, or hook the caller.
- **A target that exists only under a `cfg` the signed build does not compile in.** It is absent from the
  build, so no row can name it.

When a target is refused, read the error: `E_MIXIN_TARGET_MISSING` names the right path when the type's module
was mistaken for its `impl` file's module, and `E_FINGERPRINT` prints the current pin. If no target or hook
fits, describe the game action, side, values and whether the port must observe, change or cancel it in a hook
request rather than patching a private copy of the engine.

## Builds are tied to one game build

Every row compiles against the `sunrise` commit pinned in `[native]`, with the `mixins` tool pinned beside it.
A renamed target fails the build; a change elsewhere in the target's body is not reported unless an
`OVERWRITE` fingerprint covers it. Reread the targets you patch when the pin moves, exactly as you would
reread a transformer's mappings for a new Minecraft version. A player's launcher builds the same declarations
from the package: nothing in the port edits the installed game.

The full lifecycle is on [How the native route works](../../concepts/native-route/7-how-it-works.md). The
1.7.10-to-Sunrise mapping for everything else is on
[Porting a 1.7.10 mod to Sunrise](../../reference/porting-from-1.7.10.md).
