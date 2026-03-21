# Multi-Team Multiplayer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Support configurable N-team (1-4), M-players-per-team (1-3) multiplayer games with up to 12 players.

**Architecture:** Introduce a `GameConfig` resource as the single source of truth for team/player counts. Replace all hardcoded 2-team arrays and `Team::opponent()` usage with dynamic structures keyed by team ID. The `Player(Entity)` ownership model is unchanged — it already supports multiple clients per team.

**Tech Stack:** Rust/Bevy 0.18, bevy_replicon 0.39, Firebase Cloud Functions (TypeScript), RON map format.

**Design Spec:** `docs/plans/2026-03-21-multi-team-multiplayer-design.md`

---

## File Structure

### New files
- None — all changes are to existing files.

### Modified files (by task)

| File | Responsibility |
|------|---------------|
| `src/game/mod.rs` | GameConfig resource, remove Team::opponent/PLAYER/ENEMY |
| `src/net/commands.rs` | TeamAssignment gains slot, LobbyState multi-team variants |
| `src/net/server.rs` | Dynamic team assignment, N-team spawn logic, max_clients from config |
| `src/fleet/lobby.rs` | Creator-launched lobby, multi-team submission tracking |
| `src/weapon/damage.rs` | Last-team-standing win condition |
| `src/control_point/mod.rs` | N-team capture (plurality), dynamic TeamScores |
| `src/fog/mod.rs` | Replace Team::PLAYER/ENEMY with LocalTeam-based logic |
| `src/ui/fleet_builder.rs` | Update LobbyState match arms for new variants |
| `src/net/materializer.rs` | Team color palette for 4 teams |
| `src/map/editor.rs` | Spawn colors for teams 2-3 |
| `src/bin/server.rs` | CLI flags for team-count/players-per-team |
| `src/lobby/mod.rs` | GameInfo gains config fields |
| `src/lobby/api.rs` | Pass config to createGame, team switching API |
| `src/lobby/main_menu.rs` | Game creation dialog with config options |
| `src/lobby/game_lobby.rs` | Team switching UI, multi-team player list |
| `infra/functions/src/games.ts` | N-team game creation, join with slot, switch team, launch validation |

---

### Task 1: GameConfig Resource & Team Model Cleanup

**Files:**
- Modify: `src/game/mod.rs`

This task introduces the `GameConfig` resource and removes the hardcoded 2-team assumptions from the `Team` type. All downstream tasks depend on this.

- [ ] **Step 1: Add GameConfig resource**

In `src/game/mod.rs`, add after the `Team` impl block:

```rust
/// Configuration for the current game's team/player structure.
/// Inserted by server at startup; replicated to clients.
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct GameConfig {
    pub team_count: u8,
    pub players_per_team: u8,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self { team_count: 2, players_per_team: 1 }
    }
}

impl GameConfig {
    pub fn max_players(&self) -> usize {
        self.team_count as usize * self.players_per_team as usize
    }
}
```

- [ ] **Step 2: Remove Team::PLAYER, Team::ENEMY, Team::opponent()**

Remove the `PLAYER` and `ENEMY` constants and the `opponent()` method from `impl Team`. Keep `Team(pub u8)`, `index()`, and `is_friendly()`.

- [ ] **Step 3: Add team_color helper to Team**

Add a method that returns a display color for any team index (up to 4):

```rust
impl Team {
    pub fn color(&self) -> Color {
        match self.0 {
            0 => Color::srgb(0.2, 0.6, 1.0),  // Blue
            1 => Color::srgb(1.0, 0.2, 0.2),  // Red
            2 => Color::srgb(0.2, 1.0, 0.3),  // Green
            3 => Color::srgb(1.0, 0.8, 0.1),  // Yellow
            _ => Color::srgb(0.5, 0.5, 0.5),  // Gray fallback
        }
    }
}
```

- [ ] **Step 4: Fix tests in src/game/mod.rs**

Remove tests: `team_constants_are_distinct`, `team_equality`, `team_opponent`.
Add test for `GameConfig::default()` and `GameConfig::max_players()`.
Add test for `Team::color()` returning distinct colors for 0-3.

- [ ] **Step 5: Run `cargo test` on game module**

Run: `cargo test --lib game`
Expected: All tests pass.

- [ ] **Step 6: Fix src/fog/mod.rs — replace Team::PLAYER/ENEMY with team comparison**

In `detect_enemies()` (line 168-205), replace hardcoded `Team::PLAYER` with proper team comparison. The detect_enemies system runs on the client and needs `LocalTeam`:

```rust
fn detect_enemies(
    mut commands: Commands,
    local_team: Res<LocalTeam>,
    player_ships: Query<(&Transform, &ShipClass, &Team), With<Ship>>,
    enemy_query: Query<(Entity, &Transform, &Team, Option<&Detected>), With<Ship>>,
    asteroid_query: Query<(&Transform, &AsteroidSize), With<Asteroid>>,
) {
    let my_team = match local_team.0 {
        Some(t) => t,
        None => return,
    };
    // ...
    for (enemy_entity, enemy_transform, enemy_team, maybe_detected) in &enemy_query {
        if *enemy_team == my_team { continue; }  // Skip friendly ships
        // ...
        for (player_transform, class, player_team) in &player_ships {
            if *player_team != my_team { continue; }  // Only use own-team ships as observers
            // ...
        }
    }
}
```

- [ ] **Step 7: Fix src/weapon/damage.rs — stub check_win_condition to compile**

Temporarily replace the `Team(0).opponent()` / `Team(1).opponent()` calls with `Team(1)` / `Team(0)` explicitly. The full N-team rewrite happens in Task 4, but the code must compile now.

- [ ] **Step 8: Fix src/control_point/mod.rs — remove opponent() call in update_score_display**

In `update_score_display` (line 379), replace:
```rust
let enemy_id = local_team.0.map(|t| t.opponent().index()).unwrap_or(1);
```
with:
```rust
let enemy_id = if local_id == 0 { 1 } else { 0 };  // Temporary 2-team compat, full fix in Task 5
```

- [ ] **Step 9: Fix src/ui/fleet_builder.rs — update LobbyState match arms**

The `LobbyState` enum changes happen in Task 2, but the fleet builder UI references `WaitingForOpponent`, `OpponentSubmitted`, `OpponentComposing`. These will need updating when the enum changes. For now, ensure game/mod.rs compiles cleanly.

Run: `cargo check`
Expected: Compiles successfully.

- [ ] **Step 10: Commit**

```bash
git add src/game/mod.rs src/fog/mod.rs src/weapon/damage.rs src/control_point/mod.rs
git commit -m "feat: add GameConfig resource, remove Team::opponent/PLAYER/ENEMY"
```

---

### Task 2: Server Team Assignment & Network Config

**Files:**
- Modify: `src/net/server.rs` (on_client_connected, setup_server, ClientTeams)
- Modify: `src/net/commands.rs` (TeamAssignment, LobbyState)
- Modify: `src/bin/server.rs` (CLI flags)

This task makes the server assign teams/slots dynamically based on GameConfig and increases max_clients.

- [ ] **Step 1: Add CLI flags for team config**

In `src/bin/server.rs`, add to the `Cli` struct:

```rust
#[arg(long, default_value = "2")]
team_count: u8,

#[arg(long, default_value = "1")]
players_per_team: u8,
```

Insert `GameConfig` resource in `main()`:

```rust
.insert_resource(GameConfig { team_count: cli.team_count, players_per_team: cli.players_per_team })
```

Also read from env vars `GAME_TEAM_COUNT` and `GAME_PLAYERS_PER_TEAM` (Edgegap support), with CLI as fallback.

- [ ] **Step 2: Update ClientTeams to store PlayerSlot**

In `src/net/server.rs`, change `ClientTeams`:

```rust
#[derive(Clone, Debug)]
pub struct PlayerSlot {
    pub team: Team,
    pub slot: u8,
}

#[derive(Resource, Debug, Default)]
pub struct ClientTeams {
    pub map: HashMap<Entity, PlayerSlot>,
}
```

Update all code that reads `client_teams.map.get(...)` to extract `.team` where it previously got a `Team` directly.

- [ ] **Step 3: Update TeamAssignment command**

In `src/net/commands.rs`, extend `TeamAssignment`:

```rust
pub struct TeamAssignment {
    pub team: Team,
    pub slot: u8,
}
```

- [ ] **Step 4: Update on_client_connected for N-team assignment**

In `src/net/server.rs`, change the team assignment logic. The server needs to honor the team assignments from the Firebase lobby. Two approaches:

**For lobby mode (Edgegap):** Pass player→team mapping as an env var (e.g., `GAME_PLAYERS=alice:0,bob:0,charlie:1,dave:1`). Server parses on startup into a `LobbyPlayerTeams` resource. On connect, the client sends its player name (add to a new `ClientIdentify` event), server looks up the assigned team.

**For direct mode (--connect):** Fall back to round-robin assignment by connection order:

```rust
fn on_client_connected(..., config: Res<GameConfig>) {
    let max = config.max_players();
    let current = client_teams.map.len();
    if current >= max {
        warn!("Server full, rejecting client");
        return;
    }
    // Direct mode: assign by connection order
    let team_id = (current / config.players_per_team as usize) as u8;
    let slot = (current % config.players_per_team as usize) as u8;
    let team = Team(team_id);
    client_teams.map.insert(client_entity, PlayerSlot { team, slot });
    // Send TeamAssignment { team, slot }
}
```

**Note:** For the initial implementation, round-robin assignment is sufficient. The Firebase lobby already handles team switching — the server just needs to know the final team assignments. Pass them via Edgegap env vars in Task 8 Step 6 (alongside GAME_TEAM_COUNT). The `on_client_connected` handler checks `LobbyPlayerTeams` first, falls back to round-robin if not in lobby mode.

- [ ] **Step 5: Update max_clients in setup_server**

Change `max_clients: 2` to `max_clients: config.max_players()` (read from `Res<GameConfig>`). The `setup_server` system needs `GameConfig` as a parameter.

- [ ] **Step 6: Update LobbyState enum for multi-team**

In `src/net/commands.rs`, replace opponent-specific variants:

```rust
pub enum LobbyState {
    Composing,
    WaitingForOpponent,  // Keep but rename semantically: still "waiting for others"
    Countdown(f32),
    Rejected(String),
    /// N players have submitted their fleets (informational)
    SubmissionCount(u32),
}
```

Remove `OpponentSubmitted` and `OpponentComposing` — replace with `SubmissionCount(n)` which tells each client how many total submissions exist.

- [ ] **Step 6b: Update fleet builder LobbyState match arms**

In `src/ui/fleet_builder.rs` (around line 1607-1625), update the match on `LobbyState` to handle the new variants. Replace:
- `LobbyState::WaitingForOpponent` → `LobbyState::WaitingForOpponent` (kept, renamed semantically)
- `LobbyState::OpponentSubmitted` → `LobbyState::SubmissionCount(n)` with message like "{n} players submitted"
- `LobbyState::OpponentComposing` → remove (no longer exists)

Also update `src/net/server.rs:on_client_connected` (line 234) which sends `LobbyState::OpponentSubmitted` — change to `LobbyState::SubmissionCount`.

- [ ] **Step 7: Fix all compilation errors from ClientTeams change**

Every place that does `client_teams.map.get(&entity)` and expects a `&Team` now gets `&PlayerSlot`. Add `.team` accessors. Key locations:
- `validate_ownership` — check `client_teams.map.contains_key` (unchanged)
- `server_setup_game` — use `slot.team` instead of `*team`
- `server_update_visibility` — use `slot.team` instead of `*client_team`
- Lobby handlers — iterate with `.team` access

Run: `cargo check`

- [ ] **Step 8: Update client-side TeamAssignment handler**

In `src/net/client.rs`, the `on_team_assignment` observer now receives a `slot` field too. Store it if needed for display, or ignore the slot (client only needs team for rendering).

- [ ] **Step 9: Commit**

```bash
git add src/net/server.rs src/net/commands.rs src/bin/server.rs src/net/client.rs
git commit -m "feat: dynamic team assignment with GameConfig, PlayerSlot"
```

---

### Task 3: Lobby — Creator-Launched, Multi-Team Submissions

**Files:**
- Modify: `src/fleet/lobby.rs`

The lobby no longer auto-starts at 2 submissions. Instead, the countdown is triggered by a new `LaunchCommand` from the creator. Minimum: at least 1 submitted fleet per team.

- [ ] **Step 1: Add LaunchCommand event**

In `src/net/commands.rs`, add:

```rust
#[derive(Event, Debug, Clone, Serialize, Deserialize)]
pub struct LaunchCommand;

impl MapEntities for LaunchCommand {
    fn map_entities<M: bevy::ecs::entity::EntityMapper>(&mut self, _mapper: &mut M) {}
}
```

Register it as a client event in the server net plugin.

- [ ] **Step 2: Update handle_fleet_submission**

Remove the `if lobby.submissions.len() >= 2` block that auto-starts countdown. Instead, after storing the submission, broadcast `SubmissionCount(lobby.submissions.len() as u32)` to all clients.

- [ ] **Step 3: Add handle_launch_command observer**

New observer in `src/fleet/lobby.rs`:

```rust
fn handle_launch_command(
    trigger: On<FromClient<LaunchCommand>>,
    mut commands: Commands,
    mut lobby: ResMut<LobbyTracker>,
    client_teams: Res<ClientTeams>,
    config: Res<GameConfig>,
) {
    // Verify sender is the first connected client (creator)
    // Check: every team 0..config.team_count has at least 1 submission
    // If valid: lobby.countdown = Some(3.0), broadcast Countdown
    // If not: send Rejected to the launcher
}
```

- [ ] **Step 4: Update handle_cancel_submission**

Remove `OpponentComposing` notification. Instead broadcast `SubmissionCount(lobby.submissions.len() as u32)`.

- [ ] **Step 5: Write tests for lobby submission counting**

Test that submissions from multiple clients on multiple teams are tracked correctly.
Test that launch is rejected when a team has zero submissions.
Test that launch succeeds when every team has at least one.

Run: `cargo test --lib fleet`

- [ ] **Step 6: Commit**

```bash
git add src/fleet/lobby.rs src/net/commands.rs
git commit -m "feat: creator-launched lobby with multi-team submission tracking"
```

---

### Task 4: Win Conditions — Last Team Standing

**Files:**
- Modify: `src/weapon/damage.rs` (check_win_condition)

- [ ] **Step 1: Rewrite check_win_condition for N teams**

Replace the hardcoded `[0u32; 2]` arrays with a `HashMap<u8, (bool, u32)>` (team_seen, alive_count). Logic:

```rust
fn check_win_condition(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    ships: Query<(&Team, Option<&Destroyed>), With<Ship>>,
    config: Res<GameConfig>,
) {
    let mut alive: HashMap<u8, u32> = HashMap::new();
    let mut seen: HashSet<u8> = HashSet::new();

    for (team, destroyed) in &ships {
        seen.insert(team.0);
        if destroyed.is_none() {
            *alive.entry(team.0).or_default() += 1;
        }
    }

    // Wait until all teams have spawned
    if seen.len() < config.team_count as usize { return; }

    // Find teams with alive ships
    let alive_teams: Vec<u8> = (0..config.team_count)
        .filter(|t| alive.get(t).copied().unwrap_or(0) > 0)
        .collect();

    if alive_teams.len() == 1 {
        let winning_team = Team(alive_teams[0]);
        // Broadcast GameResult, transition to GameOver
    }
    // If 0 alive teams (simultaneous kill): no winner, still GameOver
    if alive_teams.is_empty() {
        next_state.set(GameState::GameOver);
    }
}
```

- [ ] **Step 2: Update existing damage tests that use Team::opponent()**

Fix any test that calls `Team(0).opponent()` — replace with explicit `Team(1)` or appropriate logic.

- [ ] **Step 3: Add tests for N-team win condition**

Test: 3 teams, team 0 eliminated → game continues (2 alive).
Test: 3 teams, teams 0 and 2 eliminated → team 1 wins.
Test: 2 teams, team 1 eliminated → team 0 wins (backwards compatible).

Run: `cargo test --lib weapon::damage`

- [ ] **Step 4: Commit**

```bash
git add src/weapon/damage.rs
git commit -m "feat: last-team-standing win condition for N teams"
```

---

### Task 5: Control Points — Plurality Capture for N Teams

**Files:**
- Modify: `src/control_point/mod.rs`

- [ ] **Step 1: Change TeamScores to dynamic**

Replace `scores: [f32; 2]` with `scores: HashMap<u8, f32>` (or `Vec<f32>` — HashMap is simpler since it doesn't need GameConfig at construction).

```rust
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct TeamScores {
    pub scores: HashMap<u8, f32>,
}
```

- [ ] **Step 2: Rewrite compute_next_state for N teams**

Change signature from `(team0_count, team1_count)` to `(team_counts: &HashMap<u8, u32>)`:

```rust
pub fn compute_next_state(
    current: &ControlPointState,
    team_counts: &HashMap<u8, u32>,
    dt: f32,
) -> (ControlPointState, Option<u8>) {
    // Find plurality team: team with most ships, if unique
    // If two teams tied for most, plurality_team = None (freeze)
    // Net advantage = plurality count - second highest count
    // Rest of state machine logic stays the same
}
```

Plurality logic:
1. Sort teams by count descending
2. If top count > second count → plurality_team = Some(top), net = top - second
3. If top count == second count → plurality_team = None, net = 0
4. Empty zone (all zero): decay as before

- [ ] **Step 3: Update update_control_points system**

Replace `team0_count`/`team1_count` with `HashMap<u8, u32>` built from the ship query:

```rust
let mut team_counts: HashMap<u8, u32> = HashMap::new();
for (ship_tf, team) in &ships {
    let ship_pos = Vec2::new(ship_tf.translation.x, ship_tf.translation.z);
    if ship_pos.distance_squared(center) <= r_sq {
        *team_counts.entry(team.0).or_default() += 1;
    }
}
let (new_state, scoring_team) = compute_next_state(&state, &team_counts, dt);
```

- [ ] **Step 4: Update check_score_victory for N teams**

Replace `[0.0f32; 2]` with a HashMap accumulator. Iterate all teams, find any that hit threshold.

- [ ] **Step 5: Update client-side score display (update_score_display)**

Replace the binary "my score vs enemy score" display. For 2 teams keep the existing format. For 3-4 teams, show all team scores (e.g., "Blue: 15 | Red: 8 | Green: 22"). Use `Team::color()` for text coloring. Remove the `team.opponent()` call.

- [ ] **Step 6: Update team_color helper**

Replace the binary friendly/enemy color lookup with `Team::color()` from Task 1.

- [ ] **Step 7: Fix all control point tests**

Update all `compute_next_state` test calls from `(state, team0, team1, dt)` to `(state, &counts_map, dt)`. Add tests for 3-team scenarios:
- Team 0 has plurality → captures
- Teams 0 and 1 tied, team 2 present → freeze
- All teams equal → freeze

Run: `cargo test --lib control_point`

- [ ] **Step 8: Commit**

```bash
git add src/control_point/mod.rs
git commit -m "feat: plurality-based control point capture for N teams"
```

---

### Task 6: Fleet Spawning — N Teams, Staggered Players

**Files:**
- Modify: `src/net/server.rs` (server_setup_game, build_default_map_data, is_in_asteroid_exclusion_zone)

- [ ] **Step 1: Update build_default_map_data for N spawns**

Generate spawn points evenly around the map perimeter based on `GameConfig::team_count`. For team `i` of `n`:

```rust
let angle = std::f32::consts::TAU * (i as f32 / n as f32) - std::f32::consts::FRAC_PI_4;
let distance = 300.0; // from center
let pos = Vec2::new(angle.cos() * distance, angle.sin() * distance);
```

For 2 teams this produces opposing corners (similar to current). The function needs `team_count: u8` as a parameter. Update all call sites:
- `server_setup_game` (line 360/364): pass `config.team_count`
- Any test that calls `build_default_map_data()`: pass team count arg

- [ ] **Step 2: Update is_in_asteroid_exclusion_zone**

Change from hardcoded 2 corners to dynamic: accept a `&[Vec2]` of spawn positions.

```rust
pub fn is_in_asteroid_exclusion_zone(candidate: Vec2, spawn_positions: &[Vec2]) -> bool {
    spawn_positions.iter().any(|corner| candidate.distance(*corner) < ASTEROID_EXCLUSION_RADIUS)
}
```

- [ ] **Step 3: Update server_setup_game for N teams + staggered players**

Replace `team_corners: [(Team, Vec2); 2]` with dynamic:

```rust
let team_corners: Vec<(Team, Vec2)> = (0..config.team_count)
    .map(|i| {
        let corner = map_data.spawns.iter()
            .find(|s| s.team == i)
            .map(|s| Vec2::new(s.position.0, s.position.1))
            .unwrap_or_else(|| default_corner(i, config.team_count));
        (Team(i), corner)
    })
    .collect();
```

Replace `team_ship_counts: [usize; 2]` with `HashMap<u8, usize>`.

For staggering players within a team zone: offset each player's fleet block perpendicular to the spawn facing, with spacing between player groups.

- [ ] **Step 4: Remove TEAM0_CORNER / TEAM1_CORNER constants**

These are replaced by the dynamic generation in `build_default_map_data`.

- [ ] **Step 5: Update server_setup_game log message**

Change `"spawned fleets for 2 teams"` to `"spawned fleets for {} teams"`.

- [ ] **Step 6: Fix server.rs tests**

Update `is_in_asteroid_exclusion_zone` tests to pass spawn positions. Update `build_default_map_data` tests for dynamic spawn count.

Run: `cargo test --lib net::server`

- [ ] **Step 7: Commit**

```bash
git add src/net/server.rs
git commit -m "feat: N-team fleet spawning with staggered player positions"
```

---

### Task 7: Materializer & Editor — Team Colors

**Files:**
- Modify: `src/net/materializer.rs`
- Modify: `src/map/editor.rs`

- [ ] **Step 1: Update materialize_ships to use Team::color()**

Replace the binary blue/red color logic with `team.color()` for all teams. Keep the own-team distinction for alpha mode (opaque vs blend):

```rust
let color = team.color();
let is_own_team = team.is_friendly(&local_team);
// Use opaque for own team, blend for enemy — keep existing material logic
```

- [ ] **Step 2: Update map editor spawn colors**

Replace `SPAWN_TEAM0_COLOR` / `SPAWN_TEAM1_COLOR` with `Team(i).color()` calls. Add support for teams 2 and 3 in the editor palette UI (spawn placement buttons for each team).

- [ ] **Step 3: Commit**

```bash
git add src/net/materializer.rs src/map/editor.rs
git commit -m "feat: team colors for up to 4 teams in materializer and editor"
```

---

### Task 8: Firebase Lobby API — N-Team Games

**Files:**
- Modify: `infra/functions/src/games.ts`
- Modify: `src/lobby/mod.rs` (GameInfo, GameDetail)
- Modify: `src/lobby/api.rs`

- [ ] **Step 1: Update createGame endpoint**

Accept `team_count` and `players_per_team` in request body (default to 2/1 for backwards compat):

```typescript
const { creator, map, team_count = 2, players_per_team = 1 } = req.body;
// Store in game doc
const doc = await db.collection("games").add({
    creator,
    status: "waiting",
    team_count,
    players_per_team,
    players: [{ name: creator, team: 0, ready: false }],
    // ...
});
```

- [ ] **Step 2: Update joinGame endpoint**

Remove hardcoded `team: 1`. Assign to first team with an open slot:

```typescript
const maxPerTeam = data.players_per_team || 1;
const teamCounts = new Array(data.team_count || 2).fill(0);
data.players.forEach((p: any) => teamCounts[p.team]++);
const openTeam = teamCounts.findIndex((c: number) => c < maxPerTeam);
if (openTeam === -1) throw new Error("game full");
```

- [ ] **Step 3: Add switchTeam endpoint**

New endpoint for players to switch teams in the lobby:

```typescript
export const switchTeam = onRequest({ region: REGION }, async (req, res) => {
    const { game_id, name, target_team } = req.body;
    // Transaction: verify target_team has space, update player's team field
});
```

- [ ] **Step 4: Update launchGame validation**

Replace `data.players.length < 2` with: every team 0..team_count must have at least 1 ready player.

Remove `allReady` check — creator launches manually, only submitted (ready) players participate.

- [ ] **Step 5: Update listGames to include config**

Add `team_count` and `players_per_team` to the game list response:

```typescript
const games = snapshot.docs.map(doc => ({
    // ...existing fields...
    team_count: doc.data().team_count || 2,
    players_per_team: doc.data().players_per_team || 1,
    max_players: (doc.data().team_count || 2) * (doc.data().players_per_team || 1),
}));
```

- [ ] **Step 6: Pass team_count/players_per_team as Edgegap env vars**

In `launchGame`, add to `environment_variables`:

```typescript
{ key: "GAME_TEAM_COUNT", value: String(data.team_count || 2), is_hidden: false },
{ key: "GAME_PLAYERS_PER_TEAM", value: String(data.players_per_team || 1), is_hidden: false },
```

- [ ] **Step 7: Update Rust lobby data types**

In `src/lobby/mod.rs`, add fields to `GameInfo` and `GameDetail`:

```rust
pub struct GameInfo {
    // ...existing...
    pub team_count: Option<u8>,
    pub players_per_team: Option<u8>,
    pub max_players: Option<usize>,
}
```

- [ ] **Step 8: Update Rust lobby API calls**

In `src/lobby/api.rs`, pass `team_count` and `players_per_team` to `create_game`. Add `switch_team` API call.

- [ ] **Step 9: Commit**

```bash
git add infra/functions/src/games.ts src/lobby/mod.rs src/lobby/api.rs
git commit -m "feat: Firebase lobby API supports N-team games with team switching"
```

---

### Task 9: Client Lobby UI — Game Creation Config & Team Switching

**Files:**
- Modify: `src/lobby/main_menu.rs` (create game dialog)
- Modify: `src/lobby/game_lobby.rs` (player list, team switching)

- [ ] **Step 1: Update create game dialog**

Add dropdowns or buttons for team count (1-4) and players per team (1-3) in the create game dialog. Pass these to the `create_game` API call.

- [ ] **Step 2: Update game list display**

Show game config in the lobby list (e.g., "2v2" or "3 teams × 2 players"). Update `rebuild_game_list` to display `team_count` × `players_per_team`.

- [ ] **Step 3: Update game lobby player list**

Group players by team in the player list panel. Show team color next to each player name. Each team section has a header with the team color.

- [ ] **Step 4: Add team switch button**

In the game lobby, add a button per team that the player can click to switch to (if that team has space). Calls the `switch_team` API.

- [ ] **Step 5: Update launch button validation**

Only enable the launch button when every team has at least 1 ready player. Show status text indicating which teams still need players.

- [ ] **Step 6: Commit**

```bash
git add src/lobby/main_menu.rs src/lobby/game_lobby.rs
git commit -m "feat: client lobby UI for N-team game creation and team switching"
```

---

### Task 10: Integration — Wire GameConfig Through Connection Flow

**Files:**
- Modify: `src/net/server.rs` (register GameConfig for replication or send as event)
- Modify: `src/net/client.rs` (receive and insert GameConfig)
- Modify: `src/lobby/game_lobby.rs` (pass config to AutoFleet / connect flow)

- [ ] **Step 1: Send GameConfig to clients on connect**

Either replicate `GameConfig` as a replicated resource, or send it as part of `TeamAssignment`. The client needs to know the game shape for UI display. Simplest: include `team_count` and `players_per_team` in `TeamAssignment`:

```rust
pub struct TeamAssignment {
    pub team: Team,
    pub slot: u8,
    pub team_count: u8,
    pub players_per_team: u8,
}
```

Client inserts `GameConfig` resource on receiving TeamAssignment.

- [ ] **Step 2: Pass config through lobby → connect flow**

When client connects from lobby, the `GameDetail` already has team_count/players_per_team. Store in a resource so the client knows the config before connecting.

- [ ] **Step 3: Run full integration test**

Start server with `--team-count 2 --players-per-team 2`.
Connect 4 clients. Verify team assignment: 2 per team.
Submit fleets. Creator launches. Verify 4 fleets spawn.

Run: `cargo test` (full suite)
Expected: All 293+ tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/net/server.rs src/net/client.rs src/lobby/game_lobby.rs
git commit -m "feat: wire GameConfig through connection flow to clients"
```

---

### Task 11: Cleanup & Full Test Pass

**Files:**
- All modified files
- Update `CLAUDE.md` with new architecture notes

- [ ] **Step 1: Search for remaining hardcoded 2-team assumptions**

Grep for: `[0u32; 2]`, `[f32; 2]`, `[usize; 2]`, `[false; 2]`, `Team(0).opponent`, `Team::PLAYER`, `Team::ENEMY`, `teams_seen[0]`, `teams_seen[1]`, `alive_counts[0]`, `totals[0]`, `totals[1]`.

Fix any remaining instances.

- [ ] **Step 2: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Run cargo clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 4: Update CLAUDE.md**

Add GameConfig documentation, update team assignment notes, update lobby flow description, add multi-team notes to control points and win conditions sections.

- [ ] **Step 5: Update run_game.sh for testing**

Add a `run_game_4p.sh` or update `run_game.sh` to accept team config flags.

- [ ] **Step 6: Final commit**

```bash
git add -A
git commit -m "feat: multi-team multiplayer complete — up to 4 teams, 3 players each"
```
