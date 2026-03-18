# Phase 7: Cloud Deployment & Game Lobby

End-to-end cloud deployment: Firebase lobby backend, Edgegap game servers,
CI/CD via GitHub Actions + Pulumi, client main menu UI with fleet building.

## Architecture

```
Client (Bevy)
  ├── MainMenu: browse/create/join games (HTTP → Firebase Cloud Functions)
  ├── GameLobby: see players, build fleet, creator launches (polls Firebase)
  └── Connecting: UDP connect to Edgegap server (renet)

Firebase Cloud Functions (lobby API)
  ├── CRUD on Firestore game documents
  ├── Calls Edgegap Deploy API on launch
  └── Receives Edgegap webhook when server is ready

Edgegap (game server hosting)
  ├── Spins up Docker container at edge location near players
  ├── Injects ARBITRIUM_* env vars
  └── Server self-terminates on GameOver

GitHub Actions CI/CD (on tag v*)
  ├── pulumi up (Firebase project, Firestore, Cloud Functions)
  ├── Sync maps to Firestore
  ├── Build + push server Docker image to Edgegap registry
  ├── Create Edgegap app version
  └── Build client binaries, upload to GitHub Release
```

## Data Model

Firestore collection `games`, one document per game:

```
games/{gameId}
  creator: string              // player display name
  status: "waiting" | "launching" | "ready" | "closed"
  players: [                   // array for future N-player support
    { name: string, team: 0 }
    { name: string, team: 1 }
  ]
  server_address: string?      // "1.2.3.4:5000" once Edgegap is ready
  edgegap_request_id: string?  // for tracking/cleanup
  created_at: timestamp
  map: string?                 // map name from assets/maps/
```

Status flow: `waiting → launching → ready → closed`

- `waiting` — creator is in, game visible in browse list.
- `launching` — creator hit launch, Edgegap deployment requested.
- `ready` — Edgegap returned IP:port, clients connect.
- `closed` — game started or expired.

Map list stored in `config/maps` document:

```
config/maps
  maps: ["chokepoint", "untitled", ...]
```

Updated by CI/CD from `assets/maps/*.ron` filenames.

## Firebase Cloud Functions API

Seven endpoints:

| Endpoint | Method | What |
|---|---|---|
| `/games` | POST | Create game. Body: `{creator, map?}`. Returns `{gameId}`. |
| `/games` | GET | List open games (`status == "waiting"`). Returns `[{gameId, creator, playerCount, map, createdAt}]`. |
| `/games/:id` | GET | Poll game state. Returns full doc. Client polls every 2-3s in lobby. |
| `/games/:id/join` | POST | Join game. Body: `{name}`. Adds player on team 1. Fails if 2 players already. |
| `/games/:id/launch` | POST | Creator-only. Sets `launching`, calls Edgegap Deploy API. |
| `/games/:id` | DELETE | Leave/cancel. Creator leaving deletes game. Non-creator removes self. |
| `/webhook/edgegap` | POST | Edgegap callback. Receives deployment info, updates game doc to `ready` with `server_address`. |

TTL cleanup: scheduled function (every 10 min) deletes games older than 30 min
still in `waiting` or `launching`.

## Edgegap Integration

The launch function calls Edgegap Deploy API:

```
POST https://api.edgegap.com/v2/deployments
{
  "application": "<app-name>",
  "version": "<version>",
  "env_vars": [
    { "key": "GAME_MAP", "value": "chokepoint" }
  ],
  "webhook_url": "https://<region>-<project>.cloudfunctions.net/webhook/edgegap"
}
```

Edgegap spins up the server container (~0.5s cached, ~3s average), then
hits the webhook with public IP + port. The webhook updates Firestore,
clients see `ready` on next poll.

Server reads `GAME_MAP` env var as fallback for `--map` CLI arg (already
reads `ARBITRIUM_PORT_GAMEPORT_INTERNAL` for port binding).

Server self-terminates on GameOver via `ARBITRIUM_DELETE_URL` (already implemented).

## Client State Flow

```
Setup → MainMenu → GameLobby → Connecting → Playing → GameOver
                 ↑                                       |
                 └───────────────────────────────────────┘
```

### MainMenu State

Game list screen:
- Fetches open games from `GET /games` on enter and every 3s.
- Shows: creator name, player count, map name per row.
- "Create Game" button → opens create dialog (pick map from fetched list, enter name).
- "Join" button per row → `POST /games/:id/join`, transitions to GameLobby.
- "Direct Connect" button → text input for IP:port, transitions to Connecting (dev mode).
- "Return to Menu" also reachable from GameOver.

### GameLobby State

Combined lobby + fleet builder:
- Top section: game info (map, players with team assignments, status text).
- Main section: the existing fleet builder UI (two-panel ship list + weapon slots).
- Creator sees "Launch" button, enabled when 2 players present.
- All players see status: "Waiting for opponent..." → "Launching server..." → "Connecting..."
- Polls `GET /games/:id` every 2s.
- On `status == "ready"`: stores built fleet locally, reads `server_address`, transitions
  to Connecting. Fleet is auto-submitted immediately after connecting (like `--fleet` path).

### Fleet Building in Lobby

Fleet builder is purely client-local during lobby. No server interaction needed.
Existing FleetBuilderState and UI code moves to GameLobby state. Fleet validation
rules are the same (1000pt budget, mount sizes, etc.). Server still validates
on receipt — if somehow invalid, falls back to default fleet.

FleetComposition state remains as fallback for `--connect` direct mode (local dev).

## Server Changes

One small addition:
- Read `GAME_MAP` env var as fallback for `--map` CLI arg.

Everything else (env var port binding, self-termination) already implemented.
Server is unaware of lobbies — just binds, waits for clients, receives fleets,
validates, plays.

## Pulumi Infrastructure

TypeScript Pulumi project in `infra/` directory:

- Firebase project (or import existing)
- Firestore database (Native mode)
- Cloud Functions deployment (from `infra/functions/` source)
- Firestore security rules (lock down direct client access — all through functions)
- Service account for CI/CD

All provisioned via `pulumi up` in CI/CD. No manual Firebase console work.

## CI/CD Pipeline

Single GitHub Actions workflow, triggered on tag `v*`:

```
Tag v* pushed
├── Job 1: Infrastructure + Server
│   ├── pulumi up (Firebase, Firestore, Cloud Functions)
│   ├── Sync assets/maps/*.ron names to Firestore config/maps
│   ├── cargo build --release --bin server (Linux)
│   ├── docker build + push to registry.edgegap.com
│   └── Edgegap API: create version, enable active caching
├── Job 2: Client (macOS ARM)
│   └── cargo build --release --bin client → .dmg → GitHub Release
├── Job 3: Client (macOS Intel)
│   └── cargo build --release --bin client → .dmg → GitHub Release
├── Job 4: Client (Windows)
│   └── cargo build --release --bin client → .zip → GitHub Release
└── Job 5: Client (Linux)
    └── cargo build --release --bin client → .zip → GitHub Release
```

Job 1 runs first (infrastructure must be up before clients can use it).
Jobs 2-5 run in parallel after Job 1.

### GitHub Secrets Required

| Secret | Purpose |
|---|---|
| `PULUMI_ACCESS_TOKEN` | Pulumi Cloud state backend |
| `EDGEGAP_DOCKER_USERNAME` | Container registry login |
| `EDGEGAP_DOCKER_PASSWORD` | Container registry token |
| `EDGEGAP_API_TOKEN` | REST API for version management |
| `EDGEGAP_APP_NAME` | Application name on Edgegap |
| `FIREBASE_PROJECT_ID` | Firebase project identifier |
| `FIREBASE_SERVICE_ACCOUNT_KEY` | JSON key for CI/CD auth |

### Manual Setup (one-time)

1. Create Edgegap account at edgegap.com (free tier).
2. Create Application, get API token + container registry credentials.
3. Add all secrets to GitHub repo settings.
4. Create Pulumi account (free tier) for state management.

## Local Development

No cloud services needed for local dev:

```bash
# Terminal 1: local server
cargo run --bin server

# Terminal 2: client with direct connect (skips lobby)
cargo run --bin client -- --connect 127.0.0.1:5000

# Or with Firebase emulator for lobby testing:
cd infra && firebase emulators:start
# Then client points lobby API at localhost:5001
```

`--connect` flag bypasses MainMenu entirely, goes straight to Connecting.
FleetComposition state remains for this path.

## Implementation Order

| Step | What |
|---|---|
| 1 | Pulumi project: Firebase, Firestore, Cloud Functions scaffold |
| 2 | Cloud Functions: lobby API (create, list, get, join, launch, delete, webhook, TTL) |
| 3 | GitHub Actions CI/CD: pulumi up, Docker build+push, map sync, client builds |
| 4 | Client MainMenu + GameLobby UI with fleet builder |
| 5 | Client HTTP lobby integration (reqwest polling, state transitions) |
| 6 | Server GAME_MAP env var support |
| 7 | End-to-end test |
