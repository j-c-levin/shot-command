# Edgegap Deployment Plan

Plan for hosting game servers on Edgegap with CI/CD, client auto-update, and on-demand
match servers. Written 2026-03-17 based on research. No timeline — implement when ready.

## Overview

Edgegap runs Docker containers at 615+ edge locations. Every game session is a fresh
container placed near the players, destroyed after the match. No persistent servers.
Free tier available (no credit card), pay-per-minute after that (~$0.017/hr for 0.25 vCPU).

Reference project: https://github.com/RJ/bevygap (Bevy + Edgegap, uses Lightyear not
replicon but Edgegap patterns are directly applicable).

---

## Part 1: Server Edgegap Integration

### 1.1 Port configuration

Server currently hardcodes `--bind 127.0.0.1:5000` as default. Edgegap injects port
assignments via environment variables. Server needs to check env vars first, fall back
to CLI arg for local dev.

Relevant env vars injected by Edgegap at runtime:
- `ARBITRIUM_PORT_GAMEPORT_INTERNAL` — port server should bind to
- `ARBITRIUM_PORT_GAMEPORT_EXTERNAL` — port clients connect to
- `ARBITRIUM_PUBLIC_IP` — public IP of the host
- `ARBITRIUM_PORTS_MAPPING` — JSON with all port details
- `ARBITRIUM_REQUEST_ID` — unique deployment ID

Resolution order: `ARBITRIUM_PORT_GAMEPORT_INTERNAL` env var → `--bind` CLI arg → default.

### 1.2 Self-termination

Edgegap injects `ARBITRIUM_DELETE_URL` and `ARBITRIUM_DELETE_TOKEN`. When match ends
(GameState::GameOver), server should HTTP DELETE to that URL to destroy the container
and stop billing.

Add `reqwest` (blocking, minimal features) as a dependency. Fire the DELETE request
in a system that runs on `OnEnter(GameState::GameOver)` or after a short delay to let
clients receive the GameResult.

If env vars aren't present (local dev), skip termination silently.

### 1.3 PROTOCOL_ID as version gate

Already have `PROTOCOL_ID` in `src/net/mod.rs`. Netcode.io silently rejects mismatched
protocol IDs — clients can't connect if versions differ. Bump the last bytes on every
breaking change to replicated components, network events, or registration order.

Consider deriving from Cargo.toml version at compile time:
```
0x4E45_4255_0000_0000 | (major << 16) | (minor << 8) | patch
```

---

## Part 2: Dockerfile

Multi-stage build:
1. `rust:nightly` builder stage — copy source, `cargo build --release --bin server`
2. `debian:bookworm-slim` runtime stage — copy binary, set entrypoint

Needs `rust-src` component for `build-std`. Image should be small (< 100MB) since
server is headless with MinimalPlugins.

Entrypoint reads `ARBITRIUM_PORT_GAMEPORT_INTERNAL` and binds to `0.0.0.0:<port>`.

---

## Part 3: Edgegap App Configuration

### 3.1 Account setup

- Create account at edgegap.com (free tier)
- Request access to container registry at `registry.edgegap.com`
- Get API token for REST API calls

### 3.2 App and version

- Create Application (named container for versions)
- Create Version specifying:
  - Docker image: `registry.edgegap.com/<PROJECT>/nebulous-server:<tag>`
  - Resources: 0.25 vCPU, 256MB memory (start small, increase if needed)
  - Port: name `gameport`, protocol UDP, internal port number
  - Enable Active Caching for fast (~0.5s) spin-up

### 3.3 Never reuse image tags

Edgegap aggressively caches images. Always use unique tags (semver, timestamp, or
git SHA). Reusing a tag means old image keeps running.

---

## Part 4: CI/CD with GitHub Actions

### 4.1 Trigger

On git tag push matching `v*` pattern (e.g., `v0.1.0`).

### 4.2 Server pipeline (single Linux job)

1. Checkout + install nightly Rust + `rust-src` component
2. `cargo build --release --bin server --target x86_64-unknown-linux-gnu`
3. Build Docker image with tag from git tag
4. `docker login registry.edgegap.com` (credentials in GH secrets)
5. `docker push registry.edgegap.com/<PROJECT>/nebulous-server:<tag>`
6. POST to Edgegap API to create new App Version pointing to new tag
7. Enable Active Caching on the new version

### 4.3 Client pipeline (parallel jobs per platform)

Runs in parallel with server pipeline. Separate job per target:
- **macOS Intel**: `x86_64-apple-darwin` on `macos-latest`, produces `.dmg`
- **macOS ARM**: `aarch64-apple-darwin` on `macos-latest`, produces `.dmg`
- **Windows**: `x86_64-pc-windows-msvc` on `windows-latest`, produces `.zip`
- **Linux**: `x86_64-unknown-linux-gnu` on `ubuntu-latest`, produces `.zip`
  - Needs `libasound2-dev`, `libudev-dev` for Bevy audio/input

All artifacts uploaded as GitHub Release assets.

### 4.4 Nightly Rust gotchas

- `rust-toolchain.toml` should be picked up automatically
- Add `rustup component add rust-src` for `build-std`
- Use `Swatinem/rust-cache@v2` in every job — saves 10+ minutes per build
- Cross-compilation from Linux to other targets is fragile; use native runners

### 4.5 GitHub secrets needed

| Secret | Purpose |
|---|---|
| `EDGEGAP_DOCKER_USERNAME` | Container registry login |
| `EDGEGAP_DOCKER_PASSWORD` | Container registry token |
| `EDGEGAP_IMAGE_NAME` | e.g. `myproject-abc123/nebulous-server` |
| `EDGEGAP_API_TOKEN` | REST API auth for version management |
| `EDGEGAP_APP_NAME` | Application name on Edgegap |

### 4.6 Estimated build times

| Scenario | Time |
|---|---|
| Cold (no cache) | 20-30 min (all platforms parallel) |
| Cached (rust-cache) | 10-15 min |
| With sccache | 5-10 min |

---

## Part 5: Client Auto-Update

### 5.1 Mechanism

Use `self_update` crate. On client launch (before connecting to server):
1. Check GitHub API for latest release tag
2. Compare against `cargo_crate_version!()`
3. If newer: download matching asset, replace binary, restart
4. Then connect to server

Asset naming convention: `nebulous-client-v<version>-<target>.<ext>`
(e.g., `nebulous-client-v0.1.0-x86_64-apple-darwin.tar.gz`)

### 5.2 Version mismatch UX

If client is outdated and auto-update fails (or user declines), show a message:
"Server requires version X.Y.Z. Please update." Better than a silent connection failure.

Optional: lightweight HTTP endpoint (or static JSON on GitHub Pages) that returns
current server version. Client checks before attempting UDP connection.

### 5.3 End-to-end update time

Push tag → GH Actions builds → release assets + Edgegap deploy → client auto-updates:
**~10-15 minutes** (cached). Bottleneck is Rust compilation.

---

## Part 6: On-Demand Match Servers

### 6.1 Matchmaker options

**Option A: Edgegap's built-in matchmaker** (recommended to start)
- Configured via JSON: team sizes, latency rules, skill matching
- For 1v1: `team_count: 2, min_team_size: 1, max_team_size: 1`
- Clients poll ticket status: SEARCHING → MATCH_FOUND → HOST_ASSIGNED
- Matchmaker automatically calls Deploy API when match found
- Returns FQDN + port for client connection
- Free tier: 3 hours matchmaker runtime for testing

**Option B: Custom lobby/matchmaker**
- Your backend collects player IPs
- Calls `POST /v2/deployments` with player IPs
- Edgegap picks optimal location, returns IP:port
- Backend tells clients where to connect
- More control, more work

### 6.2 Deployment API

```
POST /v2/deployments
{
  "application": "nebulous-shot-command",
  "version": "0.1.0",
  "users": [
    { "user_type": "ip_address", "user_data": { "ip_address": "<player1>" } },
    { "user_type": "ip_address", "user_data": { "ip_address": "<player2>" } }
  ],
  "webhook_on_ready": { "url": "https://..." }
}
```

Returns public IP, external port, FQDN, request ID.

### 6.3 Client connection flow

1. Client → matchmaker: "find me a game"
2. Matchmaker finds opponent, requests Edgegap deployment with both IPs
3. Edgegap spins up server near both players (~0.5s cached, ~3s average)
4. Client receives IP:port from matchmaker
5. Client connects via UDP (renet) to that address
6. Match plays
7. Server calls `ARBITRIUM_DELETE_URL` on GameOver → container destroyed

### 6.4 Current `--connect` flag

Client currently takes `--connect <addr>` CLI arg. For matchmaker flow, the client
would get the address at runtime from the matchmaker API instead of CLI. The
`--connect` flag remains useful for local dev (direct connection to local server).

---

## Part 7: Infrastructure Around Edgegap

### 7.1 Pulumi / IaC

Edgegap has **no Pulumi or Terraform provider**. It's API-driven with ephemeral
containers — IaC doesn't fit its model. Edgegap app/version config is managed via
REST API calls in CI/CD.

Pulumi is useful for **surrounding infrastructure** if needed later:
- Custom matchmaker service (Lambda, Fly.io, etc.)
- DNS for matchmaker endpoint
- Monitoring / alerting
- Database for player stats / rankings

For now: not needed. Edgegap's built-in matchmaker + GitHub Actions covers the flow.

---

## Implementation Order

1. **Server env var support** — read `ARBITRIUM_PORT_*` with CLI fallback
2. **Self-termination** — `reqwest` DELETE on GameOver (skip if no env var)
3. **Dockerfile** — multi-stage build, test locally with `docker run`
4. **Edgegap account + app setup** — dashboard config, test manual deployment
5. **GitHub Actions server pipeline** — build, push, create version via API
6. **GitHub Actions client pipeline** — cross-platform builds, GitHub Release
7. **Client auto-update** — `self_update` crate, check on launch
8. **PROTOCOL_ID automation** — derive from Cargo.toml version
9. **Matchmaker integration** — start with Edgegap built-in, configure rules
10. **Client matchmaker flow** — replace `--connect` with matchmaker API for prod

Steps 1-4 can be done independently as a proof of concept. Steps 5-8 are the CI/CD
layer. Steps 9-10 are the matchmaker integration.
