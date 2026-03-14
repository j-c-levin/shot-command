# Nebulous Lite — Feature Brainstorm v3

Simulation-first tactical game. Physics, sensors, and loadouts create
emergent gameplay. Nothing is free, nothing is instant, every decision has
tradeoffs. All units in meters.

---

## 1. Detection & Electronic Warfare

Detection is layered. The tools you bring determine what you see and what
sees you.

### Sensor types

- **Radar (active sensor)** — emits a signal, produces precise *locks* on
  contacts. A lock means other ships in your fleet can fire accurately at
  that target using the radar ship's data. Essential for fire control.
- **Passive sensors** — detect engine thermal signatures and enemy radar
  emissions. Gives a *track* (approximate position and heading) but not a
  lock. Multiple passive tracks from different ships can be combined for
  improved accuracy. Tracks are primarily intel — useful for knowing where
  enemies are, less useful for firing solutions.
- **Radar warning receiver** — detects incoming radar emissions and gives a
  directional bearing. You know something with a radar is along that bearing,
  but not range or identity. A line on the map pointing toward the source.

### Signatures

Every ship has three signature dimensions:
- **Thermal (engine)** — increases with speed, decreases when engines are
  cut. Detected by passive sensors.
- **Electromagnetic (radar)** — only present when actively radiating.
  Detected by passive sensors and radar warning receivers.
- **Radar cross-section** — intrinsic to ship size. Bigger ships return
  more radar and are detected at greater range. Smaller ships return less
  and won't produce a clear signal until closer. This is passive — it's
  how visible you are to someone else's active radar.

Active radar is essential, not discouraged. The tradeoff is that using it
reveals your EM signature. The question is "when do I turn it on, and
which ship carries it?"

### Hiding mechanics

- Asteroids block line of sight for all sensor types
- Engine cuts reduce thermal signature (ship drifts on current velocity)
- Going dark (all systems minimal) reduces thermal and EM signatures

### Lock vs Track

This distinction is about firing solution accuracy, measured in meters:
- **Lock** (from active radar): ~5m accuracy or better. Fire control
  computer predicts target position accounting for movement. Cannons and
  guided missiles fire accurately.
- **Track** (from passive sensors): ~30-100m accuracy. Missiles can be
  fired toward the area and activate their own seekers. Cannons fired at
  a track will scatter across the uncertainty zone.

Inaccuracy doesn't degrade with distance — it compounds. A 5m error at the
source becomes a larger miss at longer range because the projectile travels
further off-axis. The firing solution quality is fixed; distance amplifies it.

### What this adds

"My radar destroyer has a lock on their fleet, but their RWR just lit up —
they know someone is radiating along our bearing. Do I kill the radar and
go dark, losing the lock, or keep it on and trust my fleet to kill them
before they close?"

---

## 2. Weapons & Armament

Three weapon families. Nothing is unlimited.

### Cannons

- Range: mid to long (varies by type)
- Types: autocannons (rapid, mid-range, moderate damage), railguns (slow
  fire rate, very long range, high penetration)
- Require a lock or track to fire (accuracy depends on fire control quality)
- Ammo is finite but generous
- Fired on a simulated trajectory — rounds travel through space and hit
  whatever they intersect. Missed shots can hit other things (asteroids,
  friendly fire).
- Fire control computer leads targets based on lock movement data

### Missiles

- Range: mid to long
- Can be programmed with a terminal phase: fly to waypoint, then activate
  onboard seeker and hunt in a player-chosen direction
- Fired in swarms from VLS (vertical launch system) mounts
- Limited ammo (magazines deplete fast)
- Torpedoes are a subclass: slower, much higher damage, same launcher system
- Can be intercepted by point defense

### Beam weapons

- Range: very short
- High sustained damage, effective against close-range targets
- Unlimited ammo but have cycle time (fire, cooldown, fire)
- Require being dangerously close to use

### Point defense

- A dedicated class of defensive system sharing small hardpoints
- Automatically intercepts incoming missiles/torpedoes within short radius
- Competes for small mount slots that could hold small cannons, sensors,
  or small missile pods
- Cannot be used offensively
- PD effectiveness = how many PD mounts vs how many incoming missiles
- Saturating PD is the core missile gameplay

### Fire control

Weapons fire based on lock/track quality. The simulation determines
trajectory, and what the projectile hits is what it hits:
- Inaccuracy compounds over distance (small error at source → larger miss
  at range)
- Fire control computer predicts where target will be (requires lock
  movement data for best results)
- Missed shots can collide with anything in their path

### Ammo

Everything has ammo. Missiles deplete fast. Cannons deplete slowly. Beams
are unlimited (energy-based, cycle time only). Running out of missiles
late-game forces closing range with cannons.

### What this adds

"I've got a missile salvo inbound — 12 missiles. Their PD can handle maybe
6. I need to fire another salvo to saturate, but I only have 8 missiles
left. If I spend them now and they survive, I've got nothing but cannons
and I'll have to close range against their railguns."

---

## 3. Damage Model

Hits land on specific ship areas and damage specific systems. Simplified
from Nebulous' full angle-of-incidence model to directional zones.

### Directional damage

Where a hit lands depends on the angle of impact relative to the ship:
- **Head-on** — primarily hull damage. Armored front.
- **Rear** — primarily engine damage. Engines exposed.
- **Broadside** — damage spread across all systems. Full profile exposed.

Ship orientation during combat matters. Presenting your nose minimizes
profile and focuses damage on your strongest area. Getting behind an enemy
targets their engines.

### Ship systems

- **Hull** — structural integrity. At 0, ship destroyed.
- **Engines** — speed and acceleration degrade with damage. At 0, adrift.
- **Sensors** — detection range and lock quality degrade. At 0, blind.
- **Weapons** — fire rate and accuracy degrade per mount. At 0, offline.
- **Reactor** — powers everything. Damage cascades to all systems.

### Repair

Limited damage control resources. When a system is destroyed, choose what
to bring back online from a finite stockpile. Strategic decision: repair
engines to flee, or weapons to fight?

Low priority for initial implementation.

### What this adds

"Their destroyer took a railgun hit to the engines — it's limping. My
scouts can get behind it and finish it off, but their cruiser is turning
to cover."

---

## 4. Fleet Composition

Ship classes define the chassis, mount points define the role.

### Ship classes (holy trinity)

- **Battleship** — large, slow, many mount points (6-8), high radar cross-
  section. Several large mounts for the biggest cannons or missile pods,
  plus medium and small mounts. The fleet anchor. Expensive.
- **Destroyer** — medium speed, medium survivability, moderate mount points
  (3-4). One large mount, several medium and small. Flexible — loadout
  defines role. The workhorse.
- **Scout** — fast, fragile, few mount points (1-2), low radar cross-section.
  Small and medium mounts only. Cheap, expendable, essential.

### Mount classes

Mounts come in sizes that constrain what can be fitted:
- **Large** — biggest cannons (railguns), large missile pods, beam weapons.
  Only battleships have several; destroyers may have one.
- **Medium** — most weapon types, radar, larger sensor suites. Available
  on all classes.
- **Small** — point defense, small sensors, EW suites, small cannons, small
  missile pods. Available on all classes. PD competes for these slots.

Mount class shapes what role a ship can play. A battleship with large mounts
naturally gravitates toward heavy firepower. A scout with only small/medium
mounts is inherently limited. But within those constraints, loadout choice
defines the role — no dedicated "support class."

### Point budget

Each match gives a point budget. Battleships cost the most, scouts the
least. Fleet composition is the first strategic decision.

### What this adds

"They brought a battleship — it's going to be hard to kill but slow, and
its radar cross-section is huge. If I flood the map with scouts I can find
it from far away, get behind it, and torpedo the engines."

---

## 5. Maneuvering & Ship Control

Ships are physical objects. Orders are given, the ship carries them out
through simulated physics. Nothing is instant.

### Movement model

- Ships have a **main engine** (rear-facing, strongest thrust) and
  **maneuvering thrusters** (weaker, all directions)
- Moving in the direction the ship faces (main engine) gives highest
  acceleration and top speed
- Moving in other directions uses maneuvering thrusters — slower
  acceleration, lower max speed
- Turning has a rate limit per class. A 180-degree order means decelerate,
  rotate, accelerate again. You see this play out.
- Momentum carries: cutting engines doesn't stop you, you drift

### Facing control

Separate from movement direction. You can order a ship to move one way
while facing another:
- Present a narrow profile while maneuvering (nose toward enemy)
- Keep weapons trained on a target while repositioning
- Show armored front while retreating
- Facing direction affects acceleration — facing your direction of travel
  means main engine thrust, facing away means thrusters only

Turn rate limits how fast facing changes. Battleships turn slowly. Scouts
are nimble.

### Waypoint queues

Shift+click to queue waypoints. Ships follow the path in sequence.
Essential for planning routes through asteroid fields and flanking maneuvers.

### No autonomous engagement AI

Ships do what you tell them. You give movement orders and designate targets.
The ship fires when it can (has lock/track, weapon in range, ammo
available). There is no "attack move" or "retreat" — the player is the
tactical AI.

### What this adds

"My destroyer is in a bad spot — enemy railgun cruiser has a lock. I order
a hard turn to present my nose, queue waypoints behind the asteroid cluster,
and cut engines to reduce signature. The turn takes three seconds and I'm
holding my breath the whole time."

---

## 6. Control Points & Win Conditions

Control points create territorial pressure and force engagements.

### Control point mechanics

- Map has 1-3 capture zones (marked areas)
- Ships must be present inside a zone long enough to complete capture
- Once captured, the point stays in your control even after ships leave
  (capture and forget)
- Each controlled point generates ticks over time
- First team to reach the tick threshold wins, OR destroy all enemy ships

### Why this matters

Without control points, the optimal strategy is to hide. Control points
force you to move, to expose your fleet, to contest space.

### Map design

Maps have structure:
- Dense asteroid fields creating chokepoints and cover lanes
- Open spaces between clusters (killing fields)
- Control points placed to force movement through interesting terrain
- Procedurally generated with structural rules

### What this adds

"They're holding B point behind the asteroid cluster. I can't see them but
they're scoring. I need to push through the chokepoint or send scouts
around the long way."

---

## 7. Multiplayer

Everything above is designed for PvP. Authoritative server fits the
simulation-first approach.

### Why authoritative server

The game is heavily simulated: projectile trajectories, physics-based
movement, sensor models, damage calculations. All runs in one place for
consistency. Clients send orders. Server simulates and replicates results.

Fog of war is natural: the server only sends each client the entities their
team can detect. Undetected enemies don't exist on the client.

### Architecture: even single player is client+host

Multiplayer is an arse to retrofit. From the start, even single player
runs as a local client connected to a local host. All game logic runs on
the server/host side, all input goes through the command channel. This
means every feature is multiplayer-compatible by default.

### Latency

Tactical games are order-based. You give an order, the ship begins
executing it over seconds — this inherently masks network latency. A 100ms
delay before your ship starts turning is invisible.

### Tech

`bevy_replicon` for entity replication and event transport.

### Development approach

No shared-screen phase. Spin up multiple client windows against one host.
Each client controls one team. This is the development and testing model
from day one.

### What this adds

The actual game. Nebulous is fundamentally about outsmarting another human.

---

## Priority — Revised Phases

### Phase 1: Core Simulation (the big one)

The foundation everything else builds on. A scene with one of each ship
class, moving them about, turning, waypoints, drifting through space like
warships.

1. **Physics-based movement** — momentum, turn rates, acceleration,
   main engine vs thrusters, deceleration-rotate-accelerate for turns
2. **Facing control** — separate movement direction from facing, facing
   affects acceleration (main engine vs thrusters)
3. **Waypoint queuing** — shift+click to queue, ships follow path in order
4. **Ship classes (holy trinity)** — battleship, destroyer, scout with
   distinct physics profiles (mass, engine power, turn rate, thruster power)

**Milestone:** Three ships on screen. Give them movement and facing orders.
Watch them execute. They feel like warships, not video game units.

### Phase 2: Multiplayer Foundation

Build it now so we never retrofit it. Even single player is client+host.

5. **Client/server architecture with bevy_replicon** — host runs simulation,
   client renders and sends commands. Input goes through command channel.
6. **Multi-window testing** — spin up two clients against one host, each
   controlling a team. This is the dev/test workflow going forward.

**Milestone:** Two players in separate windows, each moving their own fleet.
Orders sent as commands, server simulates, clients see results.

### Phase 3: Fleet & Loadouts

Content that makes the simulation interesting.

7. **Mount point system** — large/medium/small mounts per ship class
8. **Weapon variety** — cannons (autocannon, railgun) and missiles as
   distinct families. Ammo. Basic projectile simulation.
9. **Point defense** — dedicated small-mount systems that intercept missiles
10. **Fleet composition screen** — pre-game loadout with point budget

**Milestone:** Build a fleet, choose loadouts, fight. Missiles vs PD.
Cannons at range. Ammo matters.

### Phase 4: Sensors, EW & Win Conditions

The strategic layer.

11. **Detection model** — radar/passive/RWR, signatures (thermal, EM, radar
    cross-section), lock vs track
12. **Fire control integration** — weapon accuracy depends on lock quality,
    inaccuracy compounds over distance
13. **Control points + win condition** — capture zones, tick scoring,
    capture-and-forget, map design with chokepoints

**Milestone:** Sensor warfare matters. Turning on radar is a decision.
Control points force engagements.

### Phase 5: Depth

14. **Directional damage model** — system damage based on hit angle
15. **Repair system** — limited damage control resources
16. **Beam weapons** — short range, cycle time, unlimited ammo

### Rationale

Phase 1 is large but non-negotiable — physics movement is the first thing
players experience and it defines how the game feels. Facing and waypoints
are inseparable from movement because facing affects acceleration and
waypoints are how you actually play.

Multiplayer is phase 2 because retrofitting networking is a trap. Every
feature after this point is automatically multiplayer-compatible. The
client/server split also forces clean architecture: commands in, state out.

Fleet and weapons are phase 3 because they give the simulation content.
Physics without weapons is a screensaver.

Sensors and control points are phase 4 because they add strategic depth
on top of a working combat game. Control points moved here because they
need multiple ships and weapons to be meaningful.

Depth features are phase 5 because they refine rather than define the
experience.
