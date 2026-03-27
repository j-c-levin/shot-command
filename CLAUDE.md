# Nebulous Shot Command — Claude Notes

## Project

Bevy 0.18 space tactical game inspired by Nebulous: Fleet Command. Player maneuvers ships
to locate and destroy enemies. Physics-based movement with momentum, facing control, and
waypoint queuing. Three ship classes with distinct handling.
Client/server multiplayer architecture with `bevy_replicon` + `bevy_replicon_renet`.

## Sub-documents

| File | Contents |
|---|---|
| [`src/CLAUDE.md`](src/CLAUDE.md) | Architecture: module map, system ordering, connection flow, key patterns |
| [`docs/CLAUDE.md`](docs/CLAUDE.md) | Roadmap: completed phases, known bugs, what's next |
| `src/*/CLAUDE.md` | Per-module details (components, systems, pure functions) |

## Build & workflow

```bash
cargo run --bin server                # dev server (headless, 60Hz tick loop)
cargo run --bin client                # dev client (lobby mode, needs Firebase emulator)
cargo run --bin client -- --name Me --lobby-api http://localhost:5001  # lobby with custom name/API
cargo run --bin server -- --bind 0.0.0.0:5000  # server on custom address
cargo run --bin client -- --connect 1.2.3.4:5000  # client to remote server
cargo check                           # quick compilation check
cargo test                            # unit tests only (pure function + World-level, no full App)
cargo build --release --bin server    # optimized server for deployment
cargo run --bin client -- --fleet 1   # auto-submit preset fleet 1 (BB with radar)
cargo run --bin client -- --fleet 2   # auto-submit preset fleet 2 (Scout with nav radar)
./run_game.sh                         # quick dev: server + fleet 1 vs fleet 2
cargo run --bin client -- --editor             # map editor (no networking)
cargo run --bin client -- --editor --map x.ron  # edit existing map file
cargo run --bin server -- --map chokepoint.ron  # server loads designed map
cargo run --bin server -- --team-count 3 --players-per-team 1  # 3-team FFA
cargo run --bin client -- --team-count 3 --players-per-team 1  # client matching server config
cd infra && firebase emulators:start --only functions,firestore  # local lobby backend
```

Requires **nightly Rust** (`rust-toolchain.toml`). The `.cargo/config.toml` uses `-Z` flags
for share-generics and multi-threaded compilation, plus `build-std` for std rebuilds.

First build from clean is ~4-5 minutes (Bevy is large). Subsequent builds are fast.
**Never run `cargo clean` unless absolutely necessary.**

## Testing

All tests are **pure-function or World-level only** — no full App, no render context, no asset
server. This keeps `cargo test` fast and avoids GPU/window dependencies. Currently 308 tests.

- **Pure math** (physics, LOS, fade): plain `#[test]`, no imports beyond `bevy::prelude::*`
- **Resource/component presence**: `World::new()` + `world.insert_resource()` / `world.spawn()`
- **Avoid**: spinning up `App` with `DefaultPlugins` in tests

Tests live in `#[cfg(test)]` blocks at the bottom of each module file.

## Bevy 0.18 notes

- `MeshPickingPlugin` is NOT in `DefaultPlugins` — must add explicitly alongside DefaultPlugins
- `OnEnter` for default state fires before `Startup` commands are flushed — cannot query Startup-spawned entities
- Use `commands.add_observer(fn)` (global) when target entities may not exist yet; filter by component inside
- `hotpatching` and `reflect_auto_register` features disabled (Cranelift incompatibility on macOS)
- Picking uses observer pattern: `.observe(|event: On<Pointer<Click>>| { ... })`
- Use `event.event_target()` not `event.target()` in picking observers
- Meshes: `Mesh3d(handle)`, Materials: `MeshMaterial3d(handle)`
- States: `#[derive(States)]` with `init_state::<T>()`
- Ambient light: `GlobalAmbientLight` as resource, NOT `AmbientLight` as entity
- `Image::new_fill` requires 5th arg: `RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD`
- `emissive` field on `StandardMaterial` takes `LinearRgba`, not `Color` — use `LinearRgba::new(r, g, b, a)`
- `MinimalPlugins` does NOT include `StatesPlugin` — add it explicitly when using states on server

## bevy_replicon 0.39 notes

- `ConnectedClient` vs `AuthorizedClient`: messages/replication only work after auth. Use `On<Add, AuthorizedClient>` for post-connect logic.
- `ReplicationRegistry::despawn` hook: called AFTER entity is removed from entity map. Cannot keep entity alive for fade — use ghost entities instead.
- `FilterRegistry::register_scope::<Entity>()` for manual entity-level visibility. Call `ClientVisibility::set(entity, bit, visible)` each frame.
- Client events: `add_mapped_client_event::<T>(Channel::Ordered)` + `MapEntities` derive with `#[entities]` on Entity fields.
- Server events: `add_server_event::<T>(Channel::Ordered)`. Send via `commands.server_trigger(ToClients { mode, message })`.
- Client sends triggers via `commands.client_trigger(event)` (from `ClientTriggerExt` trait).
- Server receives client events as `On<FromClient<T>>` observers.

## Pre-approvals

The following tools and skills are pre-approved for autonomous use:
- All file read/write/edit operations
- All bash commands for building, testing, and running
- All glob and grep searches
- All LSP operations
- All MCP tools (context7, firebase, playwright)
- All skills (superpowers, bevy, domain-driven-design, etc.)
- All agent/subagent dispatching

## Git notes

GPG signing may not be available in all environments. Use `git -c commit.gpgsign=false commit` if needed.

## Reference projects

- Bevy 0.18 examples: `~/.cargo/registry/src/index.crates.io-*/bevy-0.18.*/examples/`
