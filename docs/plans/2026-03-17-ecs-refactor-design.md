# ECS Refactor Design

Audit and fix ECS antipatterns across the codebase. Prep for multi-player-per-team.

## Changes

### 1. Team helpers + Player component (game/mod.rs)

Team(u8) stays, gains helper methods:
- `opponent()` → `Team(1 - self.0)`
- `index()` → `self.0 as usize`
- `is_friendly(local: &LocalTeam)` → bool

New `Player(Entity)` component on every ship, referencing the ClientEntity that
owns it. Command validation checks Player instead of Team. Visibility stays
team-level. Same-team players see each other's ships but can't command them.

Remove `Team::PLAYER` / `Team::ENEMY` constants (2-player assumptions).

### 2. InputMode enum (input/mod.rs)

Replace 5 bool resources (LockMode, TargetMode, MissileMode, JoinMode, MoveMode)
with single `enum InputMode { Normal, Move, Lock, Target, Missile, Join }`.

EnemyNumbers.active field removed — derived from `mode == Target || mode == Missile`.

### 3. EngineOffline marker (ship/mod.rs, weapon/damage.rs)

Insert `EngineOffline` marker when engine hp hits 0. Remove when offline timer
expires. `apply_thrust` uses `Without<EngineOffline>` filter.

### 4. Helper extractions

- `best_sensor_range(mounts, has_radar) -> f32` — dedup from pd.rs laser/cwis
- `is_friendly_missile(ship_team, owner, team_query) -> bool` — dedup from pd.rs
- `team_color()` uses `Team::is_friendly()` — dedup between control_point + materializer

### 5. Server-side Player validation

`validate_ownership` checks `Player` component matches client entity, not Team.
`on_client_connected` assigns team via ClientTeams (unchanged) + all ships for
that client get `Player(client_entity)`.

### 6. LobbyTracker multi-player

LobbyTracker already uses `HashMap<Entity, Vec<ShipSpec>>` keyed by client entity.
Countdown triggers when all connected clients have submitted (not just 2).
`server_setup_game` iterates all submissions, spawns each player's fleet at the
appropriate team corner.

### 7. Magic number cleanup

- `control_point::update_control_points` → `team.index()`
- `control_point::check_score_victory` → `Team(idx)` with index helper
- `damage::check_win_condition` → use `team.index()` instead of raw u8
- `materializer::materialize_ships` → use `team.is_friendly()`
- `control_point::team_color` → use `team.is_friendly()` pattern
