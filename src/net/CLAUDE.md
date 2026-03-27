# net/

Networking layer — replicon transport, commands, visibility filtering, and entity materialization.

## Files

- `mod.rs` — LocalTeam resource, PROTOCOL_ID constant
- `commands.rs` — All client→server commands (MoveCommand, FacingLockCommand, TargetCommand, JoinSquadCommand, RadarToggleCommand, FleetSubmission, etc.) and server→client events (TeamAssignment, GameResult, LobbyStatus, GameStarted, SubmissionCount)
- `server.rs` — ServerNetPlugin: renet transport, connection/auth, team assignment, replication registration, fleet/asteroid spawning, command handlers with team validation, squad move propagation, visibility filtering (LosBit + RadarBit), ShipSecrets sync, disconnection handling
- `client.rs` — ClientNetPlugin: renet transport, team assignment observer, lobby status observer, game started observer, ground plane setup, CurrentLobbyState resource
- `materializer.rs` — Spawns meshes for replicated entities (Ship/Asteroid/Projectile/Missile). Ship/enemy number labels. Squad connection lines. Targeting gizmos. Explosion effects. LaserBeam visuals. `]` key debug visuals toggle.

## Key patterns

- **Team validation**: Server validates team ownership on all commands — clients can only command their own ships
- **Visibility filtering**: LosBit (ships by LOS) + RadarBit (contacts by team, missiles by LOS+radar). Per-client, per-frame.
- **ShipSecrets sync**: Server syncs Ship→ShipSecrets each frame (waypoints, facing, targeting, squad, radar active)
- **Target clearing**: Requires loss of both LOS AND radar track (signature alone not enough)
- **Squad propagation**: Leader move → followers get rotated offset destinations + same facing lock
- **Authorization**: Must use `On<Add, AuthorizedClient>` (not `ConnectedClient`) for sending messages
