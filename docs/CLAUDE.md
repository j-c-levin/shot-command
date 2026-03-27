# Roadmap

See `docs/plans/2026-03-14-feature-brainstorm-v3.md` for full details.

## Completed phases

| Phase | Summary | Design doc |
|---|---|---|
| 1: Core Simulation | Physics movement, facing, waypoints, 3 ship classes | `plans/2026-03-14-phase1-core-simulation-design.md` |
| 2: Multiplayer | Headless server, replicon replication, LOS visibility, ghost fade | `plans/2026-03-15-phase2-multiplayer-design.md` |
| 3a: Weapons | 3 cannon types, K-key targeting, projectiles, HP damage, win/lose | `plans/2026-03-15-phase3a-weapons-design.md` |
| 3b: Missiles & PD | VLS launchers, flat-flight missiles, LaserPD/CWIS, M-key mode | `plans/2026-03-15-phase3b-missiles-pd-design.md` |
| 3c: Fleet Composition | 1000pt budget builder, server lobby, mount downsizing | `plans/2026-03-16-phase3c-fleet-composition-design.md` |
| QoL | Squads, cannon stagger, ship numbers, move mode, gizmo indicators | `plans/2026-03-16-qol-features-design.md` + others |
| 4a: Radar & Detection | SearchRadar/NavRadar, SNR, RWR, RadarContact entities | `plans/2026-03-17-phase4a-radar-detection-design.md` |
| 4b: Fire Control | WON'T DO — current radar already gates at Track level | — |
| 4c: Control Points | Presence-based capture, scoring to 300, annihilation still wins | `plans/2026-03-17-phase4c-control-points-design.md` |
| 5: Damage & Repair | 3 HP pools, directional zones, offline/repair, fleet status sidebar | `plans/2026-03-17-phase5-damage-repair-design.md` |
| 6: Maps & Editor | RON map files, `--editor` flag, entity palette, save/load | `plans/2026-03-18-phase6-maps-editor-design.md` |
| 7: Cloud Deployment | Firebase lobby, Edgegap servers, Pulumi infra, CI/CD | `plans/2026-03-18-phase7-cloud-deployment-design.md` |
| 8: Multi-Team | N-team (2-8), GameConfig, PlayerSlot, dynamic spawns, CLI flags | — |

**Dropped:** Beam weapons (from original Phase 5 brainstorm).

## Known bugs

- (none currently)

## Recently completed

- Phase 8: Multi-team multiplayer (N-team support, GameConfig, PlayerSlot, Team::color(), plurality capture, last-team-standing, dynamic spawns, CLI flags)
