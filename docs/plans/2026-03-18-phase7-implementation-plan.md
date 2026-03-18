# Phase 7: Cloud Deployment & Game Lobby — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** End-to-end cloud deployment: clients browse/create/join games via a Firebase lobby, build fleets in the lobby, then auto-connect to Edgegap game servers.

**Architecture:** Firebase Cloud Functions serve as a lightweight lobby API (game CRUD + Edgegap Deploy API integration). Pulumi provisions all infrastructure. GitHub Actions CI/CD builds everything on tag push. Client adds MainMenu and GameLobby states with HTTP polling.

**Tech Stack:** Firebase Cloud Functions (TypeScript), Firestore, Pulumi (TypeScript), GitHub Actions, Edgegap Deploy API, reqwest (Rust HTTP client, already in deps), Bevy UI.

**Design doc:** `docs/plans/2026-03-18-phase7-cloud-deployment-design.md`

---

## File Structure

### Infrastructure (`infra/`)

```
infra/
├── Pulumi.yaml                    # Pulumi project config
├── Pulumi.dev.yaml                # Dev stack config
├── package.json                   # Pulumi + Firebase deps
├── tsconfig.json                  # TypeScript config
├── index.ts                       # Pulumi program: Firebase project, Firestore, functions deploy
├── firebase.json                  # Firebase CLI config (functions source, firestore rules)
├── firestore.rules                # Deny direct client access
├── firestore.indexes.json         # Composite indexes (cleanup query)
└── functions/
    ├── package.json               # Cloud Functions deps (firebase-functions, firebase-admin, node-fetch)
    ├── tsconfig.json              # Functions TypeScript config
    └── src/
        ├── index.ts               # Function exports (all endpoints)
        ├── games.ts               # Game CRUD: create, list, get, join, launch, delete
        ├── webhook.ts             # Edgegap deployment ready callback
        └── cleanup.ts             # Scheduled TTL cleanup
```

### Client changes

```
src/game/mod.rs                    # Add MainMenu, GameLobby states
src/bin/client.rs                  # Route MainMenu as default for non-editor, non-connect mode
src/lib.rs                         # Add lobby module
src/lobby/
├── mod.rs                         # LobbyPlugin, LobbyApi resource, LobbyConfig resource
├── api.rs                         # HTTP client: create, list, get, join, launch, delete (reqwest, background threads)
├── main_menu.rs                   # MainMenu UI: game list, create game dialog, direct connect
└── game_lobby.rs                  # GameLobby UI: player list, fleet builder (reuse), launch button, status
src/ui/fleet_builder.rs            # Extract fleet builder into reusable functions (spawn in both FleetComposition and GameLobby)
src/ui/mod.rs                      # Update FleetUiPlugin to work with both states
```

### Server changes

```
src/bin/server.rs                  # Add GAME_MAP env var support
```

### CI/CD

```
.github/workflows/release.yml     # Full pipeline: pulumi up, Docker, Edgegap, client builds, map sync
scripts/sync-maps.ts               # Script to update Firestore config/maps from assets/maps/*.ron
```

---

## Task 1: Server GAME_MAP env var

**Files:**
- Modify: `src/bin/server.rs`

This is the smallest change — server reads `GAME_MAP` env var as fallback for `--map`.

- [ ] **Step 1: Add GAME_MAP env var support**

In `src/bin/server.rs`, update `main()` to resolve the map path from env var before CLI arg:

```rust
fn resolve_map(cli_map: Option<String>) -> Option<String> {
    cli_map.or_else(|| std::env::var("GAME_MAP").ok())
}
```

Call it: `.insert_resource(ServerMapPath(resolve_map(cli.map)))`

- [ ] **Step 2: Add test for resolve_map**

Since `resolve_map` is a pure function in the binary (not lib), verify it manually:
```bash
# No env var: uses CLI arg
GAME_MAP= cargo run --bin server -- --map chokepoint --help  # should show help without error
# With env var: overrides CLI
GAME_MAP=test cargo run --bin server -- --help
```

- [ ] **Step 3: Verify server compiles and tests pass**

Run: `cargo check --bin server && cargo test --lib`
Expected: clean compile, 293 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/bin/server.rs
git commit -m "feat: server reads GAME_MAP env var for Edgegap map selection"
```

---

## Task 2: Add MainMenu and GameLobby states

**Files:**
- Modify: `src/game/mod.rs`

- [ ] **Step 1: Add MainMenu and GameLobby to GameState enum**

```rust
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States, Serialize, Deserialize)]
pub enum GameState {
    #[default]
    Setup,
    /// Client: main menu — browse/create/join games
    MainMenu,
    /// Client: in a game lobby — see players, build fleet, wait for launch
    GameLobby,
    WaitingForPlayers,
    Connecting,
    FleetComposition,
    Playing,
    GameOver,
    Editor,
}
```

- [ ] **Step 2: Update game state tests to include new variants**

Add MainMenu and GameLobby to the `game_states_are_distinct` test array.

- [ ] **Step 3: Verify tests pass**

Run: `cargo test --lib`
Expected: all tests pass (new states are just enum variants, no logic yet)

- [ ] **Step 4: Commit**

```bash
git add src/game/mod.rs
git commit -m "feat: add MainMenu and GameLobby game states"
```

---

## Task 3: Lobby API client module

**Files:**
- Create: `src/lobby/mod.rs`
- Create: `src/lobby/api.rs`
- Modify: `src/lib.rs`

This task builds the HTTP client that talks to Firebase Cloud Functions. All HTTP calls run on background threads to avoid blocking the game loop. Results are sent back via channels.

- [ ] **Step 1: Create lobby module with types**

Create `src/lobby/mod.rs` with:
- `LobbyPlugin` (empty for now, will register systems in later tasks)
- `LobbyConfig` resource: `{ api_base_url: String }` — base URL for Firebase functions
- `PlayerName` resource: `pub struct PlayerName(pub String)` — player display name from CLI
- `LobbyApi` resource: holds the channel receivers for async HTTP results
- Data types matching Firestore documents:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub game_id: String,
    pub creator: String,
    pub player_count: usize,
    pub map: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDetail {
    pub game_id: String,
    pub creator: String,
    pub status: String,
    pub players: Vec<PlayerInfo>,
    pub server_address: Option<String>,
    pub map: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub name: String,
    pub team: u8,
}
```

- [ ] **Step 2: Create api.rs with HTTP functions**

Create `src/lobby/api.rs` with functions that spawn threads and return `Receiver<Result<T, String>>`:

```rust
pub fn list_games(api_base: &str) -> Receiver<Result<Vec<GameInfo>, String>>
pub fn create_game(api_base: &str, creator: &str, map: Option<&str>) -> Receiver<Result<String, String>>
pub fn get_game(api_base: &str, game_id: &str) -> Receiver<Result<GameDetail, String>>
pub fn join_game(api_base: &str, game_id: &str, name: &str) -> Receiver<Result<(), String>>
pub fn launch_game(api_base: &str, game_id: &str) -> Receiver<Result<(), String>>
pub fn delete_game(api_base: &str, game_id: &str) -> Receiver<Result<(), String>>
pub fn fetch_maps(api_base: &str) -> Receiver<Result<Vec<String>, String>>
```

Each function creates a `reqwest::blocking::Client`, makes the HTTP call, sends the result through the channel. Example pattern:

```rust
pub fn list_games(api_base: &str) -> Receiver<Result<Vec<GameInfo>, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let url = format!("{api_base}/games");
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let result = client.get(&url).send()
            .map_err(|e| e.to_string())
            .and_then(|r| r.json::<Vec<GameInfo>>().map_err(|e| e.to_string()));
        let _ = tx.send(result);
    });
    rx
}
```

- [ ] **Step 3: Add lobby module to lib.rs**

Add `pub mod lobby;` to `src/lib.rs`.

- [ ] **Step 4: Verify compiles**

Run: `cargo check`
Expected: clean compile (no systems wired up yet, just types and functions)

- [ ] **Step 5: Commit**

```bash
git add src/lobby/ src/lib.rs
git commit -m "feat: lobby API client module with HTTP types and background thread calls"
```

---

## Task 4: MainMenu UI

**Files:**
- Create: `src/lobby/main_menu.rs`
- Modify: `src/lobby/mod.rs`
- Modify: `src/bin/client.rs`

- [ ] **Step 1: Create MainMenu UI**

Create `src/lobby/main_menu.rs` with:

**Resources:**
- `MainMenuState` — holds `games: Vec<GameInfo>`, `maps: Vec<String>`, `poll_timer: Timer` (3s repeating), `error: Option<String>`, `pending_list: Option<Receiver<...>>`, `pending_maps: Option<Receiver<...>>`, `create_dialog_open: bool`, `selected_map: Option<String>`, `player_name: String`, `direct_connect_addr: String`

**Marker components:**
- `MainMenuRoot`, `GameListPanel`, `GameRow(String)`, `JoinButton(String)`, `CreateGameButton`, `DirectConnectButton`, `RefreshButton`, `CreateDialogOverlay`, `MapPickerOption(String)`, `CreateConfirmButton`, `PlayerNameInput`

**Systems:**
- `spawn_main_menu` — OnEnter(MainMenu): spawn full-screen UI with game list area, buttons bar ("Create Game", "Direct Connect", "Refresh"), initialize MainMenuState, fire initial list_games + fetch_maps calls
- `despawn_main_menu` — OnExit(MainMenu): despawn MainMenuRoot, remove MainMenuState
- `poll_game_list` — Update: check poll_timer, fire list_games when timer ticks. Check pending_list receiver for results, update games Vec, trigger rebuild
- `poll_maps` — Update: check pending_maps receiver, update maps Vec
- `rebuild_game_list` — Update (on games changed): clear GameListPanel children, spawn rows with creator name, player count ("/2"), map name, "Join" button per row. Show "No open games" if empty.
- `handle_join_button` — On click: call join_game API, on success transition to GameLobby with game_id
- `handle_create_button` — On click: open create dialog (map picker from maps list + confirm button)
- `handle_create_confirm` — On click: call create_game API with player_name and selected_map, on success transition to GameLobby
- `handle_direct_connect` — On click: set ClientConnectAddress, transition to Connecting (existing flow)
- `handle_refresh` — On click: fire list_games immediately

**UI Layout:**
```
┌─────────────────────────────────────┐
│  NEBULOUS SHOT COMMAND              │
├─────────────────────────────────────┤
│  [Create Game]  [Direct Connect]    │
├─────────────────────────────────────┤
│  OPEN GAMES                [Refresh]│
│  ┌─────────────────────────────────┐│
│  │ CreatorName  1/2  chokepoint [J]││
│  │ OtherPlayer  1/2  untitled   [J]││
│  │                                 ││
│  │      No open games              ││
│  └─────────────────────────────────┘│
└─────────────────────────────────────┘
```

- [ ] **Step 2: Register MainMenu systems in LobbyPlugin**

In `src/lobby/mod.rs`, add to `LobbyPlugin::build()`:
```rust
app.add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
   .add_systems(OnExit(GameState::MainMenu), despawn_main_menu)
   .add_systems(Update, (
       poll_game_list,
       poll_maps,
       rebuild_game_list,
       handle_join_button,
       handle_create_button,
       handle_create_confirm,
       handle_direct_connect,
       handle_refresh,
   ).run_if(in_state(GameState::MainMenu)));
```

- [ ] **Step 3: Update client binary**

In `src/bin/client.rs`:
- Add `LobbyPlugin` to the app plugins
- Add `--name` CLI arg for player display name (default: "Player")
- Add `--lobby-api` CLI arg for Firebase functions URL (default: "http://localhost:5001/PROJECT/REGION")
- Insert `LobbyConfig` resource
- Insert `PlayerName` resource
- Change the non-editor, non-`--connect` startup to transition to `MainMenu` instead of `Connecting`
- When `--connect` is explicitly provided, keep the direct `Connecting` path (skip MainMenu)
- Add `LobbyPlugin` import

The `--connect` flag needs to distinguish between "user provided an address" vs "using default". Change from `default_value` to `Option<String>`:

```rust
#[arg(long)]
connect: Option<String>,
```

If `connect` is Some → direct Connecting mode (existing flow).
If `connect` is None → MainMenu mode (new lobby flow).

- [ ] **Step 4: Add "Return to Menu" on GameOver**

In `src/net/client.rs` `show_game_over_ui()`: add a "Return to Menu" button. Add a system `handle_return_to_menu` that transitions to `MainMenu` on click. Register it in `ClientNetPlugin`.

- [ ] **Step 5: Verify compiles**

Run: `cargo check --bin client`
Expected: clean compile (Firebase functions don't exist yet so HTTP calls will fail, but the UI code is sound)

- [ ] **Step 6: Commit**

```bash
git add src/lobby/main_menu.rs src/lobby/mod.rs src/bin/client.rs src/net/client.rs
git commit -m "feat: MainMenu UI with game list, create game, direct connect"
```

---

## Task 5: Extract fleet builder for reuse

**Files:**
- Modify: `src/ui/fleet_builder.rs`
- Modify: `src/ui/mod.rs`

The fleet builder currently only works in FleetComposition state. It needs to also work in GameLobby. The key change: extract the UI spawning into a function that takes a parent entity (so it can be embedded in the lobby layout), and make the rebuild/handler systems run in either state.

- [ ] **Step 1: Make fleet builder state-agnostic**

In `src/ui/mod.rs`, change `FleetUiPlugin` systems from `.run_if(in_state(GameState::FleetComposition))` to `.run_if(in_state(GameState::FleetComposition).or(in_state(GameState::GameLobby)))`.

The spawn/despawn systems remain on FleetComposition state only — GameLobby will spawn its own UI that includes the fleet builder panel.

- [ ] **Step 2: Extract fleet builder panel as a reusable function**

In `src/ui/fleet_builder.rs`, extract the inner content of `spawn_fleet_ui` (the two-panel layout + footer) into a public function:

```rust
/// Spawn the fleet builder panels as children of the given parent entity.
/// Used by both FleetComposition (standalone) and GameLobby (embedded).
pub fn spawn_fleet_builder_content(commands: &mut ChildSpawnerCommands<'_>) { ... }
```

Keep the existing `spawn_fleet_ui` calling this function wrapped in a FleetUiRoot node (for FleetComposition backwards compatibility).

- [ ] **Step 3: Make submit button behavior configurable**

In GameLobby, the submit button should NOT send a FleetSubmission network trigger (there's no server yet). Instead it should just mark the fleet as "ready" locally. Add a `FleetBuilderMode` resource:

```rust
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub enum FleetBuilderMode {
    /// Connected to server, submit triggers network event
    Online,
    /// In lobby, submit just validates and stores locally
    Lobby,
}
```

Default to `Online` (existing behavior). GameLobby inserts `Lobby`. The `handle_submit_button` system checks this resource.

- [ ] **Step 4: Verify tests pass**

Run: `cargo test --lib && cargo check --bin client`
Expected: all pass, no behavior change in FleetComposition

- [ ] **Step 5: Commit**

```bash
git add src/ui/fleet_builder.rs src/ui/mod.rs
git commit -m "feat: extract fleet builder for reuse in GameLobby, add FleetBuilderMode"
```

---

## Task 6: GameLobby UI

**Files:**
- Create: `src/lobby/game_lobby.rs`
- Modify: `src/lobby/mod.rs`

- [ ] **Step 1: Create GameLobby UI**

Create `src/lobby/game_lobby.rs` with:

**Resources:**
- `GameLobbyState` — holds `game_id: String`, `detail: Option<GameDetail>`, `poll_timer: Timer` (2s repeating), `pending_detail: Option<Receiver<...>>`, `is_creator: bool`, `fleet_ready: bool`

**Marker components:**
- `GameLobbyRoot`, `LobbyInfoPanel`, `PlayerListPanel`, `LaunchButton`, `LobbyStatusText`, `LeaveButton`

**UI Layout:**
```
┌──────────────────────────────────────────────────────┐
│ GAME LOBBY                              [Leave]      │
├──────────────┬───────────────────────────────────────┤
│  PLAYERS     │                                       │
│  ┌──────────┐│    FLEET COMPOSITION                  │
│  │ You  T0  ││    (reused fleet builder panels)      │
│  │ ???  T1  ││                                       │
│  └──────────┘│                                       │
│              │                                       │
│  Map: name   │                                       │
│              │                                       │
│  Status:     │                                       │
│  Waiting...  │                                       │
│              │                                       │
│  [LAUNCH]    │                                       │
├──────────────┴───────────────────────────────────────┤
│  Budget: 450 / 1000                                  │
└──────────────────────────────────────────────────────┘
```

Left sidebar (~25%): player list, map name, status text, launch button (creator only).
Right area (~75%): fleet builder content (via `spawn_fleet_builder_content`).

**Systems:**
- `spawn_game_lobby` — OnEnter(GameLobby): spawn layout, init GameLobbyState from `CurrentGameId` resource, insert `FleetBuilderMode::Lobby`, init `FleetBuilderState`, fire initial get_game
- `despawn_game_lobby` — OnExit(GameLobby): despawn GameLobbyRoot, remove GameLobbyState, remove FleetBuilderMode
- `poll_game_detail` — Update: check poll_timer, fire get_game when timer ticks. Check receiver, update detail. When status becomes "ready": store server_address in `ClientConnectAddress`, store fleet in `AutoFleet` equivalent, transition to Connecting
- `rebuild_player_list` — Update (on detail changed): update player names, status text
- `handle_launch_button` — On click (creator only, 2 players present): call launch_game API, update status to "Launching server..."
- `handle_leave_button` — On click: call delete_game API, transition back to MainMenu

**Connecting with pre-built fleet:**
When transitioning from GameLobby → Connecting → FleetComposition, the fleet built in lobby needs to auto-submit. Use the existing `AutoFleet` resource pattern from `--fleet` CLI. GameLobby stores the fleet into `AutoFleet` before transitioning to Connecting. The existing `auto_submit_fleet` system handles the rest.

- [ ] **Step 2: Add CurrentGameId resource**

Add to `src/lobby/mod.rs`:
```rust
/// Resource set when entering a game lobby (from join or create).
#[derive(Resource, Debug, Clone)]
pub struct CurrentGameId(pub String);
```

MainMenu sets this before transitioning to GameLobby.

- [ ] **Step 3: Register GameLobby systems in LobbyPlugin**

```rust
app.add_systems(OnEnter(GameState::GameLobby), spawn_game_lobby)
   .add_systems(OnExit(GameState::GameLobby), despawn_game_lobby)
   .add_systems(Update, (
       poll_game_detail,
       rebuild_player_list,
       handle_launch_button,
       handle_leave_button,
   ).run_if(in_state(GameState::GameLobby)));
```

- [ ] **Step 4: Wire up auto-submit on connect**

In `src/bin/client.rs`, the `auto_submit_fleet` system is currently only registered when `--fleet` CLI flag is provided. Change this: always register it on `OnEnter(FleetComposition)` with a `resource_exists::<AutoFleet>` run condition, so it fires for both `--fleet` CLI and GameLobby flows:

```rust
// Always register (not inside the `if let Some(fleet_id)` block)
app.add_systems(
    OnEnter(GameState::FleetComposition),
    auto_submit_fleet.run_if(resource_exists::<AutoFleet>),
);
```

Also update `run_game.sh` to pass `--connect 127.0.0.1:5000` explicitly (since `--connect` is now `Option<String>` and no longer defaults to localhost).

- [ ] **Step 5: Verify compiles**

Run: `cargo check --bin client`
Expected: clean compile

- [ ] **Step 6: Commit**

```bash
git add src/lobby/game_lobby.rs src/lobby/mod.rs src/bin/client.rs
git commit -m "feat: GameLobby UI with player list, fleet builder, launch flow"
```

---

## Task 7: Firebase Cloud Functions

**Files:**
- Create: `infra/functions/package.json`
- Create: `infra/functions/tsconfig.json`
- Create: `infra/functions/src/index.ts`
- Create: `infra/functions/src/games.ts`
- Create: `infra/functions/src/webhook.ts`
- Create: `infra/functions/src/cleanup.ts`

- [ ] **Step 1: Set up functions project**

Create `infra/functions/package.json`:
```json
{
  "name": "nebulous-lobby",
  "main": "lib/index.js",
  "scripts": {
    "build": "tsc",
    "serve": "npm run build && firebase emulators:start --only functions,firestore",
    "deploy": "firebase deploy --only functions"
  },
  "engines": { "node": "20" },
  "dependencies": {
    "firebase-admin": "^13.0.0",
    "firebase-functions": "^6.0.0"
  },
  "devDependencies": {
    "typescript": "^5.0.0"
  }
}
```

Create `infra/functions/tsconfig.json`:
```json
{
  "compilerOptions": {
    "module": "commonjs",
    "noImplicitReturns": true,
    "noUnusedLocals": true,
    "outDir": "lib",
    "sourceMap": true,
    "strict": true,
    "target": "es2020",
    "esModuleInterop": true
  },
  "compileOnSave": true,
  "include": ["src"]
}
```

- [ ] **Step 2: Create games.ts with CRUD endpoints**

Create `infra/functions/src/games.ts`:

```typescript
import * as admin from "firebase-admin";
import { onRequest } from "firebase-functions/v2/https";

const db = admin.firestore();

export const createGame = onRequest(async (req, res) => {
  if (req.method !== "POST") { res.status(405).send("Method not allowed"); return; }
  const { creator, map } = req.body;
  if (!creator) { res.status(400).send("creator required"); return; }

  const doc = await db.collection("games").add({
    creator,
    status: "waiting",
    players: [{ name: creator, team: 0 }],
    server_address: null,
    edgegap_request_id: null,
    created_at: admin.firestore.FieldValue.serverTimestamp(),
    map: map || null,
  });
  res.json({ gameId: doc.id });
});

export const listGames = onRequest(async (req, res) => {
  if (req.method !== "GET") { res.status(405).send("Method not allowed"); return; }
  const snapshot = await db.collection("games")
    .where("status", "==", "waiting")
    .orderBy("created_at", "desc")
    .limit(50)
    .get();

  const games = snapshot.docs.map(doc => ({
    game_id: doc.id,
    creator: doc.data().creator,
    player_count: doc.data().players?.length || 0,
    map: doc.data().map,
    status: doc.data().status,
    created_at: doc.data().created_at?.toDate?.()?.toISOString() || null,
  }));
  res.json(games);
});

export const getGame = onRequest(async (req, res) => {
  // Extract game ID from path: /games/GAME_ID
  const gameId = req.path.split("/").pop();
  if (!gameId) { res.status(400).send("game ID required"); return; }

  const doc = await db.collection("games").doc(gameId).get();
  if (!doc.exists) { res.status(404).send("game not found"); return; }
  const data = doc.data()!;

  res.json({
    game_id: doc.id,
    creator: data.creator,
    status: data.status,
    players: data.players || [],
    server_address: data.server_address,
    map: data.map,
  });
});

export const joinGame = onRequest(async (req, res) => {
  if (req.method !== "POST") { res.status(405).send("Method not allowed"); return; }
  const gameId = req.path.split("/").filter(Boolean).find((_, i, arr) => arr[i - 1] === "games");
  const { name } = req.body;
  if (!gameId || !name) { res.status(400).send("game ID and name required"); return; }

  const gameRef = db.collection("games").doc(gameId);
  await db.runTransaction(async (tx) => {
    const doc = await tx.get(gameRef);
    if (!doc.exists) throw new Error("game not found");
    const data = doc.data()!;
    if (data.status !== "waiting") throw new Error("game not accepting players");
    if (data.players.length >= 2) throw new Error("game full");
    tx.update(gameRef, {
      players: [...data.players, { name, team: 1 }],
    });
  });
  res.json({ ok: true });
});

export const launchGame = onRequest(async (req, res) => {
  if (req.method !== "POST") { res.status(405).send("Method not allowed"); return; }
  const gameId = req.path.split("/").filter(Boolean).find((_, i, arr) => arr[i - 1] === "games");
  if (!gameId) { res.status(400).send("game ID required"); return; }

  const gameRef = db.collection("games").doc(gameId);
  const doc = await gameRef.get();
  if (!doc.exists) { res.status(404).send("game not found"); return; }
  const data = doc.data()!;

  // Only creator can launch
  if (req.body.creator !== data.creator) {
    res.status(403).send("only creator can launch");
    return;
  }
  if (data.players.length < 2) {
    res.status(400).send("need 2 players to launch");
    return;
  }

  // Call Edgegap Deploy API
  const edgegapToken = process.env.EDGEGAP_API_TOKEN;
  const edgegapApp = process.env.EDGEGAP_APP_NAME;
  const edgegapVersion = process.env.EDGEGAP_APP_VERSION || "latest";
  const webhookUrl = process.env.EDGEGAP_WEBHOOK_URL;

  if (!edgegapToken || !edgegapApp) {
    // Dev mode: no Edgegap, return localhost
    await gameRef.update({
      status: "ready",
      server_address: "127.0.0.1:5000",
    });
    res.json({ ok: true, dev_mode: true });
    return;
  }

  const deployPayload = {
    application: edgegapApp,
    version: edgegapVersion,
    env_vars: data.map ? [{ key: "GAME_MAP", value: data.map, is_hidden: false }] : [],
    webhook_on_ready: webhookUrl ? { url: webhookUrl } : undefined,
  };

  const deployRes = await fetch("https://api.edgegap.com/v2/deployments", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `token ${edgegapToken}`,
    },
    body: JSON.stringify(deployPayload),
  });

  if (!deployRes.ok) {
    const err = await deployRes.text();
    res.status(502).send(`Edgegap deploy failed: ${err}`);
    return;
  }

  const deployData = await deployRes.json() as { request_id: string };
  await gameRef.update({
    status: "launching",
    edgegap_request_id: deployData.request_id,
  });
  res.json({ ok: true, request_id: deployData.request_id });
});

export const deleteGame = onRequest(async (req, res) => {
  if (req.method !== "DELETE") { res.status(405).send("Method not allowed"); return; }
  const gameId = req.path.split("/").pop();
  if (!gameId) { res.status(400).send("game ID required"); return; }

  // For simplicity: just delete the doc. In production, check authorization.
  await db.collection("games").doc(gameId).delete();
  res.json({ ok: true });
});
```

- [ ] **Step 3: Create webhook.ts**

Create `infra/functions/src/webhook.ts`:

```typescript
import * as admin from "firebase-admin";
import { onRequest } from "firebase-functions/v2/https";

const db = admin.firestore();

export const edgegapWebhook = onRequest(async (req, res) => {
  if (req.method !== "POST") { res.status(405).send("Method not allowed"); return; }

  const { request_id, fqdn, ports } = req.body;
  if (!request_id) { res.status(400).send("request_id required"); return; }

  // Find the game with this edgegap_request_id
  const snapshot = await db.collection("games")
    .where("edgegap_request_id", "==", request_id)
    .limit(1)
    .get();

  if (snapshot.empty) {
    res.status(404).send("no game found for this deployment");
    return;
  }

  const doc = snapshot.docs[0];

  // Extract connection info from Edgegap response
  const gamePort = ports?.gameport;
  const externalPort = gamePort?.external;
  const publicIp = fqdn || req.body.public_ip;

  if (!publicIp || !externalPort) {
    res.status(400).send("missing connection info");
    return;
  }

  await doc.ref.update({
    status: "ready",
    server_address: `${publicIp}:${externalPort}`,
  });

  res.json({ ok: true });
});
```

- [ ] **Step 4: Create cleanup.ts**

Create `infra/functions/src/cleanup.ts`:

```typescript
import * as admin from "firebase-admin";
import { onSchedule } from "firebase-functions/v2/scheduler";

const db = admin.firestore();

export const cleanupStaleGames = onSchedule("every 10 minutes", async () => {
  const cutoff = new Date(Date.now() - 30 * 60 * 1000); // 30 min ago
  const snapshot = await db.collection("games")
    .where("created_at", "<", cutoff)
    .where("status", "in", ["waiting", "launching"])
    .get();

  const batch = db.batch();
  snapshot.docs.forEach(doc => batch.delete(doc.ref));
  await batch.commit();

  console.log(`Cleaned up ${snapshot.size} stale games`);
});
```

- [ ] **Step 5: Create index.ts**

Create `infra/functions/src/index.ts`:

```typescript
import * as admin from "firebase-admin";
admin.initializeApp();

export { createGame, listGames, getGame, joinGame, launchGame, deleteGame } from "./games";
export { edgegapWebhook } from "./webhook";
export { cleanupStaleGames } from "./cleanup";
```

- [ ] **Step 6: Commit**

```bash
git add infra/functions/
git commit -m "feat: Firebase Cloud Functions for game lobby API"
```

---

## Task 8: Pulumi infrastructure

**Files:**
- Create: `infra/Pulumi.yaml`
- Create: `infra/Pulumi.dev.yaml`
- Create: `infra/package.json`
- Create: `infra/tsconfig.json`
- Create: `infra/index.ts`
- Create: `infra/.gitignore`

- [ ] **Step 1: Create Pulumi project scaffold**

Create `infra/Pulumi.yaml`:
```yaml
name: nebulous-lobby
runtime:
  name: nodejs
  options:
    typescript: true
description: Firebase infrastructure for Nebulous Shot Command game lobby
```

Create `infra/Pulumi.dev.yaml`:
```yaml
config:
  gcp:project: nebulous-shot-command
  gcp:region: us-central1
```

Create `infra/package.json`:
```json
{
  "name": "nebulous-lobby-infra",
  "devDependencies": {
    "@types/node": "^20",
    "typescript": "^5"
  },
  "dependencies": {
    "@pulumi/pulumi": "^3",
    "@pulumi/gcp": "^8",
    "@pulumi/command": "^1"
  }
}
```

Create `infra/tsconfig.json`:
```json
{
  "compilerOptions": {
    "strict": true,
    "outDir": "bin",
    "target": "es2020",
    "module": "commonjs",
    "moduleResolution": "node",
    "sourceMap": true,
    "esModuleInterop": true
  },
  "files": ["index.ts"]
}
```

Create `infra/.gitignore`:
```
node_modules/
bin/
```

- [ ] **Step 2: Create Pulumi index.ts**

Create `infra/index.ts`:

```typescript
import * as pulumi from "@pulumi/pulumi";
import * as gcp from "@pulumi/gcp";
import * as command from "@pulumi/command";

const config = new pulumi.Config();
const project = gcp.config.project!;
const region = gcp.config.region || "us-central1";

// Enable required GCP APIs
const firestoreApi = new gcp.projects.Service("firestore-api", {
  service: "firestore.googleapis.com",
  disableOnDestroy: false,
});

const cloudfunctionsApi = new gcp.projects.Service("cloudfunctions-api", {
  service: "cloudfunctions.googleapis.com",
  disableOnDestroy: false,
});

const cloudbuildApi = new gcp.projects.Service("cloudbuild-api", {
  service: "cloudbuild.googleapis.com",
  disableOnDestroy: false,
});

// Firestore database (Native mode)
const firestore = new gcp.firestore.Database("default", {
  locationId: region,
  type: "FIRESTORE_NATIVE",
}, { dependsOn: [firestoreApi] });

// Deploy Cloud Functions via firebase CLI (simpler than individual gcp.cloudfunctionsv2.Function resources)
const functionsInstall = new command.local.Command("functions-install", {
  dir: `${pulumi.getProject()}/../infra/functions`,
  create: "npm ci",
});

const functionsBuild = new command.local.Command("functions-build", {
  dir: `${pulumi.getProject()}/../infra/functions`,
  create: "npm run build",
}, { dependsOn: [functionsInstall] });

// Deploy functions using firebase CLI
const functionsDeploy = new command.local.Command("functions-deploy", {
  dir: `${pulumi.getProject()}/../infra`,
  create: `npx firebase deploy --only functions --project ${project} --force`,
  environment: {
    GOOGLE_APPLICATION_CREDENTIALS: process.env.GOOGLE_APPLICATION_CREDENTIALS || "",
  },
}, { dependsOn: [functionsBuild, cloudfunctionsApi, firestore] });

// Export the functions base URL
export const functionsBaseUrl = pulumi.interpolate`https://${region}-${project}.cloudfunctions.net`;
export const projectId = project;
```

Note: This uses the `firebase` CLI for functions deployment since Pulumi's native GCP Cloud Functions v2 resources require more boilerplate than `firebase deploy`. The `firebase.json` config needed:

Create `infra/firebase.json`:
```json
{
  "functions": {
    "source": "functions",
    "runtime": "nodejs20"
  },
  "firestore": {
    "rules": "firestore.rules"
  }
}
```

Create `infra/firestore.rules`:
```
rules_version = '2';
service cloud.firestore {
  match /databases/{database}/documents {
    // All access through Cloud Functions only — deny direct client access
    match /{document=**} {
      allow read, write: if false;
    }
  }
}
```

Create `infra/firestore.indexes.json` (composite index needed for cleanup query):
```json
{
  "indexes": [
    {
      "collectionGroup": "games",
      "queryScope": "COLLECTION",
      "fields": [
        { "fieldPath": "status", "order": "ASCENDING" },
        { "fieldPath": "created_at", "order": "ASCENDING" }
      ]
    }
  ]
}
```

- [ ] **Step 3: Install dependencies and verify**

Run:
```bash
cd infra && npm install
cd infra/functions && npm install
cd infra && npx tsc --noEmit
```
Expected: no TypeScript errors

- [ ] **Step 4: Commit**

```bash
git add infra/
git commit -m "feat: Pulumi infrastructure for Firebase project, Firestore, Cloud Functions"
```

---

## Task 9: Map sync script

**Files:**
- Create: `scripts/sync-maps.ts`

- [ ] **Step 1: Create map sync script**

Create `scripts/sync-maps.ts`:

```typescript
#!/usr/bin/env npx ts-node
/**
 * Syncs available map names from assets/maps/*.ron to Firestore config/maps document.
 * Run by CI/CD after deployment.
 *
 * Usage: GOOGLE_APPLICATION_CREDENTIALS=key.json FIREBASE_PROJECT_ID=xxx npx ts-node scripts/sync-maps.ts
 */
import * as admin from "firebase-admin";
import * as fs from "fs";
import * as path from "path";

const projectId = process.env.FIREBASE_PROJECT_ID;
if (!projectId) {
  console.error("FIREBASE_PROJECT_ID required");
  process.exit(1);
}

admin.initializeApp({ projectId });
const db = admin.firestore();

const mapsDir = path.resolve(__dirname, "../assets/maps");
const mapFiles = fs.readdirSync(mapsDir)
  .filter(f => f.endsWith(".ron"))
  .map(f => f.replace(".ron", ""));

async function main() {
  await db.collection("config").doc("maps").set({ maps: mapFiles });
  console.log(`Synced ${mapFiles.length} maps: ${mapFiles.join(", ")}`);
}

main().catch(e => { console.error(e); process.exit(1); });
```

Also create `scripts/package.json`:
```json
{
  "dependencies": {
    "firebase-admin": "^13.0.0",
    "ts-node": "^10.0.0",
    "typescript": "^5.0.0"
  }
}
```

- [ ] **Step 2: Commit**

```bash
git add scripts/
git commit -m "feat: map sync script for CI/CD Firestore update"
```

---

## Task 10: GitHub Actions CI/CD

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create release workflow**

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags: ["v*"]

env:
  CARGO_TERM_COLOR: always

jobs:
  infrastructure:
    name: Infrastructure + Server
    runs-on: ubuntu-latest
    outputs:
      version: ${{ steps.version.outputs.version }}
    steps:
      - uses: actions/checkout@v4

      - name: Extract version from tag
        id: version
        run: echo "version=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT

      # -- Pulumi --
      - uses: actions/setup-node@v4
        with:
          node-version: 20

      - name: Install Pulumi CLI
        uses: pulumi/actions@v6
        with:
          command: version

      - name: Install infra dependencies
        run: cd infra && npm ci && cd functions && npm ci

      # Write service account key to file for GCP auth
      - name: Write GCP credentials
        run: echo '${{ secrets.FIREBASE_SERVICE_ACCOUNT_KEY }}' > /tmp/gcp-key.json

      - name: Pulumi up
        uses: pulumi/actions@v6
        with:
          command: up
          stack-name: dev
          work-dir: infra
        env:
          PULUMI_ACCESS_TOKEN: ${{ secrets.PULUMI_ACCESS_TOKEN }}
          GOOGLE_APPLICATION_CREDENTIALS: /tmp/gcp-key.json

      # -- Sync maps --
      - name: Install sync script dependencies
        run: cd scripts && npm ci

      - name: Sync maps to Firestore
        run: npx ts-node scripts/sync-maps.ts
        env:
          FIREBASE_PROJECT_ID: ${{ secrets.FIREBASE_PROJECT_ID }}
          GOOGLE_APPLICATION_CREDENTIALS: /tmp/gcp-key.json

      # -- Server Docker --
      # Build entirely inside Docker (Dockerfile handles nightly + build-std).
      # No host Rust toolchain needed for server.
      - name: Build and push Docker image
        run: |
          echo "${{ secrets.EDGEGAP_DOCKER_PASSWORD }}" | \
            docker login registry.edgegap.com -u "${{ secrets.EDGEGAP_DOCKER_USERNAME }}" --password-stdin
          docker build -t registry.edgegap.com/${{ secrets.EDGEGAP_IMAGE_NAME }}:${{ steps.version.outputs.version }} .
          docker push registry.edgegap.com/${{ secrets.EDGEGAP_IMAGE_NAME }}:${{ steps.version.outputs.version }}

      # -- Edgegap version --
      - name: Create Edgegap app version
        run: |
          curl -s -X POST "https://api.edgegap.com/v1/app/${{ secrets.EDGEGAP_APP_NAME }}/version" \
            -H "Authorization: token ${{ secrets.EDGEGAP_API_TOKEN }}" \
            -H "Content-Type: application/json" \
            -d '{
              "name": "${{ steps.version.outputs.version }}",
              "docker_repository": "registry.edgegap.com/${{ secrets.EDGEGAP_IMAGE_NAME }}",
              "docker_image": "registry.edgegap.com/${{ secrets.EDGEGAP_IMAGE_NAME }}",
              "docker_tag": "${{ steps.version.outputs.version }}",
              "req_cpu": 256,
              "req_memory": 256,
              "ports": [
                {
                  "port": 5000,
                  "protocol": "UDP",
                  "name": "gameport"
                }
              ]
            }'

  client-macos-arm:
    name: Client (macOS ARM)
    needs: infrastructure
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
        with:
          targets: aarch64-apple-darwin
          components: rust-src
      - uses: Swatinem/rust-cache@v2
      - name: Build
        run: cargo build --release --bin client --target aarch64-apple-darwin
      - name: Package
        run: |
          mkdir -p dist
          cp target/aarch64-apple-darwin/release/client dist/nebulous-client
          cd dist && zip nebulous-client-v${{ needs.infrastructure.outputs.version }}-macos-arm64.zip nebulous-client
      - uses: softprops/action-gh-release@v2
        with:
          files: dist/*.zip

  client-macos-intel:
    name: Client (macOS Intel)
    needs: infrastructure
    runs-on: macos-13
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
        with:
          targets: x86_64-apple-darwin
          components: rust-src
      - uses: Swatinem/rust-cache@v2
      - name: Build
        run: cargo build --release --bin client --target x86_64-apple-darwin
      - name: Package
        run: |
          mkdir -p dist
          cp target/x86_64-apple-darwin/release/client dist/nebulous-client
          cd dist && zip nebulous-client-v${{ needs.infrastructure.outputs.version }}-macos-x86_64.zip nebulous-client
      - uses: softprops/action-gh-release@v2
        with:
          files: dist/*.zip

  client-windows:
    name: Client (Windows)
    needs: infrastructure
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
        with:
          targets: x86_64-pc-windows-msvc
          components: rust-src
      - uses: Swatinem/rust-cache@v2
      - name: Build
        run: cargo build --release --bin client --target x86_64-pc-windows-msvc
      - name: Package
        shell: bash
        run: |
          mkdir -p dist
          cp target/x86_64-pc-windows-msvc/release/client.exe dist/nebulous-client.exe
          cd dist && 7z a nebulous-client-v${{ needs.infrastructure.outputs.version }}-windows-x86_64.zip nebulous-client.exe
      - uses: softprops/action-gh-release@v2
        with:
          files: dist/*.zip

  client-linux:
    name: Client (Linux)
    needs: infrastructure
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Linux deps
        run: sudo apt-get update && sudo apt-get install -y libasound2-dev libudev-dev
      - uses: dtolnay/rust-toolchain@nightly
        with:
          targets: x86_64-unknown-linux-gnu
          components: rust-src
      - uses: Swatinem/rust-cache@v2
      - name: Build
        run: cargo build --release --bin client --target x86_64-unknown-linux-gnu
      - name: Package
        run: |
          mkdir -p dist
          cp target/x86_64-unknown-linux-gnu/release/client dist/nebulous-client
          cd dist && zip nebulous-client-v${{ needs.infrastructure.outputs.version }}-linux-x86_64.zip nebulous-client
      - uses: softprops/action-gh-release@v2
        with:
          files: dist/*.zip
```

- [ ] **Step 2: Commit**

```bash
git add .github/
git commit -m "feat: GitHub Actions release pipeline (Pulumi, Docker, Edgegap, client builds)"
```

---

## Task 11: CLAUDE.md update

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update CLAUDE.md**

Update the roadmap section to mark Phase 7 as in-progress. Add new modules to the Architecture section:
- `src/lobby/` module description
- `infra/` directory description
- New GameState variants (MainMenu, GameLobby)
- CI/CD pipeline description
- Updated connection flow

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with Phase 7 lobby and deployment architecture"
```

---

## Task 12: Local end-to-end test

- [ ] **Step 1: Start Firebase emulator**

```bash
cd infra && firebase emulators:start --only functions,firestore
```

- [ ] **Step 2: Start local server**

```bash
cargo run --bin server
```

- [ ] **Step 3: Launch client in lobby mode**

```bash
cargo run --bin client -- --lobby-api http://localhost:5001/nebulous-shot-command/us-central1 --name Player1
```

- [ ] **Step 4: Test the flow**

1. Client shows MainMenu with empty game list
2. Click "Create Game" → select map → confirm
3. Client transitions to GameLobby, shows "Waiting for opponent..."
4. Build a fleet in the lobby
5. Launch a second client (Player2), see the game in the list, click Join
6. Creator sees opponent joined, clicks Launch
7. In dev mode (no Edgegap env vars), launch function returns localhost:5000
8. Both clients auto-connect, auto-submit fleets, game starts

- [ ] **Step 5: Verify direct connect still works**

```bash
cargo run --bin client -- --connect 127.0.0.1:5000
```
Should skip MainMenu, go straight to Connecting → FleetComposition → Playing.

---

## Task Parallelization Notes

Tasks can be grouped for parallel execution:

- **Group A (Rust):** Tasks 1-6 (server change, game states, lobby module, UI, fleet builder, game lobby) — sequential, each builds on the previous
- **Group B (Firebase):** Tasks 7-9 (Cloud Functions, Pulumi, map sync) — independent of Group A
- **Group C (CI/CD):** Task 10 — depends on both Group A and B being complete
- **Task 11 (docs):** after all code tasks
- **Task 12 (test):** after everything

Groups A and B can be worked on in parallel by separate agents using worktrees.
