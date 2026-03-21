# Multi-Team Multiplayer Design

## Overview

Extend the game from fixed 2-player/2-team to configurable N-team, M-players-per-team
multiplayer. Game creator sets team count (1-4) and players per team (1-3) at game
creation, supporting up to 12 players.

## Game Configuration

- Game creator specifies **team count** (1-4) and **players per team** (1-3) when creating
  a game.
- Configuration is fixed for the lifetime of the game.
- Default: 2 teams, 1 player per team (backwards compatible with current behavior).

## Team Assignment

- Players join a game from the lobby and are assigned to teams in order: first N players
  fill team 0, next N fill team 1, etc.
- **Players can switch teams** freely in the lobby as long as the destination team has an
  open slot.
- Each player has a **team** and a **slot index** within that team.
- Server rejects connections once all slots are full.

## Fleet & Spawning

- Each player independently builds and submits a 1000pt fleet.
- Each team has one spawn zone (defined by map or generated).
- Players on the same team spawn staggered within the zone — offset perpendicular to the
  facing direction, spaced by player slot index.

## Ship Ownership & Control

- Each ship belongs to the player who built it — teammates cannot control each other's ships.
- All existing command validation (move, target, facing, radar, missiles, squads) is
  unchanged — ownership is per-player, not per-team.

## Visibility & Detection

- **Friendly ships** (same team, any player): always visible.
- **Enemy ships**: visible if any friendly ship on your team has LOS.
- **Radar contacts**: shared across the whole team — any teammate's radar detection benefits
  all.
- **ShipSecrets**: visible to all players on the owning team.

## Control Points

- **Plurality captures**: the team with the most ships in the zone makes progress. Ties
  freeze.
- **Decapture still required**: a non-owning team must decapture to neutral before capturing.
- **Scoring**: captured point scores 1pt/s for the owning team, first to threshold wins.
- Works identically for 2, 3, or 4 teams.

## Win Conditions

- **Annihilation**: a team is eliminated when ALL ships from ALL players on that team are
  destroyed. Last team standing wins.
- **Control point scoring**: first team to score threshold wins.
- Both conditions active simultaneously in all game sizes.

## Lobby Flow

- Game creator sets team count + players per team at creation.
- Players join and are slotted into teams.
- Players can switch teams if the destination team has space.
- Each player builds their fleet independently in the game lobby.
- **Creator launches manually** when satisfied — minimum requirement: at least 1 player
  with a submitted fleet on each team.
- Empty slots on a team are simply unused (fewer ships for that team).
- 3-second countdown after launch, then game starts.

## Map Compatibility

- Existing maps have 2 spawn points (team 0 and team 1). Games with 2 teams use these
  directly.
- For 3-4 team games, maps need additional spawn points. The map editor gains the ability
  to place spawns for teams 2 and 3.
- Random map generation places spawn points evenly around the map perimeter for the
  configured team count.

## Ship Numbering

- Ship numbers (1-9) are assigned **per team across all players** — first player's ships
  get 1, 2, 3, second player's continue at 4, 5, 6, etc.
- Number key selection works as before within your own ships.

## Constraints

- Maximum 4 teams.
- Maximum 3 players per team.
- Maximum 12 players total.
- `Team::opponent()` is removed — with N teams there is no single opponent. Code uses
  "team != my_team" checks instead.
- All hardcoded 2-element arrays (`[_; 2]`) become dynamic (Vec or HashMap keyed by team).

## Approach

Introduce a `GameConfig` resource (team_count, players_per_team) as the single source of
truth. This resource flows through team assignment, lobby logic, fleet spawning, visibility
filtering, win conditions, control point capture, and map generation. All systems that
currently assume 2 teams are updated to read from GameConfig.
