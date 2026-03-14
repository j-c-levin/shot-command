# Nebulous Lite — Feature Brainstorm v2

Revised after discussion. This is a simulation-first tactical game where
physics, sensors, and loadouts create emergent gameplay. Nothing is free,
nothing is instant, and every decision has tradeoffs.

---

## 1. Detection & Electronic Warfare

Detection is not a binary — it's layered, and the tools you bring determine
what you can see and what sees you.

### Sensor types

- **Radar (active sensor)** — emits a signal, produces precise *locks* on
  contacts. A lock means other ships in your fleet can fire accurately at
  that target using the radar ship's data. The radar ship doesn't need to
  be the one shooting. Having at least one strong radar in your fleet is
  essential for fire control.
- **Passive sensors** — detect engine exhaust signatures and enemy radar
  emissions. Gives you a *track* (approximate position and heading) but not
  a lock. You can fire at a track but accuracy is poor — rounds will land
  in the general area. Good for knowing where enemies are without revealing
  yourself.
- **Radar warning receiver** — detects that someone is using active radar in
  your direction. You know you're being scanned, but not necessarily by whom
  or from where precisely.

### Signatures

Every ship has two signature types:
- **Thermal (engine)** — increases with speed, decreases when engines are cut.
  Detected by passive sensors.
- **Electromagnetic (radar)** — only present when actively radiating. Detected
  by passive sensors and radar warning receivers.

Active radar should not be discouraged — it's essential for accurate fire
control. The tradeoff is that using it reveals your electromagnetic signature
to enemies with passive sensors. The question isn't "should I use radar?" but
"when do I turn it on, and which ship carries it?"

### Hiding mechanics

- Asteroids block line of sight for all sensor types
- Engine cuts reduce thermal signature (ship drifts on current velocity)
- Going dark (all systems minimal) makes you very hard to detect passively

### Lock vs Track

This distinction drives gameplay:
- **Lock** (from active radar): precise position. Cannons and guided missiles
  can fire accurately. Fire control quality.
- **Track** (from passive sensors): approximate area. Missiles can be fired
  toward the area and activate their own seekers. Cannons fired at a track
  will scatter. Intel quality.

### What this adds

"My radar destroyer has a lock on their fleet, but their passive sensors
just picked up my radar emission — they know I'm here. Do I kill the radar
and go dark, losing the lock, or keep it on and trust my fleet to kill them
before they close?"

---

## 2. Weapons & Armament

Three weapon families, each with distinct range profiles and tactical roles.
Nothing is unlimited.

### Cannons

- Range: mid to long (varies by type)
- Examples: autocannons (rapid, mid-range, moderate damage), railguns (slow
  fire rate, very long range, high penetration)
- Require a lock or track to fire (accuracy depends on fire control quality)
- Ammo is finite but generous
- Fired on a trajectory toward the locked/tracked target — accuracy is
  simulated, not guaranteed. Rounds travel through space and hit whatever
  they intersect.

### Missiles

- Range: mid to long
- Fired toward a target or a waypoint. Can be programmed with a terminal
  phase: fly to point X, then activate onboard seeker and hunt in a
  direction the player chooses.
- Can be fired in swarms from VLS (vertical launch system) mounts
- Limited ammo (magazines deplete fast)
- Torpedoes are a subclass: slower, much higher damage, same launcher system
- Can be intercepted by point defense

### Beam weapons

- Range: very short
- High sustained damage, effective against missiles and close-range targets
- Could double as point defense at close range
- Effectively unlimited ammo (energy-based) but require being dangerously close

### Point defense

- Automated short-range system that intercepts incoming missiles/torpedoes
- Not a separate weapon class — beams and certain cannons can serve as PD
- PD effectiveness depends on how many incoming missiles vs how many PD mounts
- Saturating PD is the core missile gameplay

### Fire control

Weapons fire at a target based on lock/track quality. The simulation
determines trajectory, and what the round/missile hits is what it hits.
This means:
- Missed shots can hit other things (asteroids, friendly fire)
- Accuracy degrades with distance and track quality
- Leading targets is simulated (system predicts where target will be)

### Ammo

Everything has ammo. Missiles deplete fast. Cannons deplete slowly. Beams
are effectively unlimited. Running out of missiles late-game forces you to
close range with cannons — a tense shift in engagement posture.

### What this adds

"I've got a missile salvo inbound — 12 missiles. Their PD can handle maybe
6. I need to fire another salvo to saturate, but I only have 8 missiles
left. If I spend them now and they survive, I've got nothing but cannons
and I'll have to close range against their railguns."

---

## 3. Damage Model

Simulation-first: hits land on specific ship areas and damage specific
systems. Armor angle matters but we simplify it to directional damage.

### Directional damage

Where a hit lands depends on the angle of impact relative to the ship:
- **Head-on** — primarily hull damage. The front is armored but eats structural
  hits.
- **Rear** — primarily engine damage. Engines are exposed at the back.
- **Broadside** — damage spread across all systems. The full profile is
  exposed.

This means ship orientation during combat matters. Presenting your nose
minimizes profile and focuses damage on your strongest armor. Showing
broadside exposes everything. Maneuvering to get behind an enemy targets
their engines.

### Ship systems

- **Hull** — structural integrity. At 0, ship is destroyed.
- **Engines** — speed and acceleration degrade with damage. At 0, adrift.
- **Sensors** — detection range and lock quality degrade. At 0, blind.
- **Weapons** — fire rate and accuracy degrade per mount. At 0 for a mount,
  that weapon is offline.
- **Reactor** — powers everything. Damage here cascades to all other systems.

### Repair

Limited repair resources. When a system is destroyed, you choose what to
bring back online from a finite stockpile of damage control points. This
is a strategic decision: repair engines to flee, or repair weapons to fight?

Low priority for initial implementation — add after the core damage model
is working.

### What this adds

"Their destroyer took a railgun hit to the engines — it's limping. My
scouts can get behind it and finish it off, but their cruiser is turning
to cover. Do I commit the scouts or pull them back?"

---

## 4. Fleet Composition

The game starts at the loadout screen. Ship classes define the chassis,
mount points define the role.

### Ship classes (holy trinity)

- **Battleship** — large, slow, many mount points (6-8). Can carry heavy
  weapons, powerful radar, lots of PD. The fleet anchor. Expensive.
- **Destroyer** — medium speed, medium survivability, moderate mount points
  (3-4). Flexible — can be built as a missile boat, gun platform, or
  sensor picket. The workhorse.
- **Scout** — fast, fragile, few mount points (1-2). Primary role is
  detection and screening. Can carry passive sensors, a small radar, or
  light weapons. Cheap, expendable, essential.

### Mount points

Each class has a fixed number of mount points. Each mount can be fitted with
one system from the available options:
- Cannon (various types)
- Missile launcher / VLS
- Beam weapon
- Radar (active sensor)
- Passive sensor suite
- Point defense system
- ECM suite (jamming, signature reduction)

No dedicated "support class" — a destroyer loaded with radar, ECM, and PD
*becomes* a support platform by loadout choice. The class defines the
chassis, the loadout defines the role.

### Point budget

Each match gives a point budget. Battleships cost the most, scouts the least.
You choose your fleet composition: one battleship and scouts? Three
destroyers? Five scouts and pray? The meta emerges from player choices.

### What this adds

"They brought a battleship — it's going to be hard to kill but slow. If I
flood the map with scouts I can find it, get behind it, and torpedo the
engines. But if they have escort destroyers with PD, my torpedoes won't
get through..."

---

## 5. Maneuvering & Ship Control

Ships are physical objects. Orders are given, and the ship carries them out
through simulated physics. Nothing is instant.

### Movement model

- Ships have a **main engine** (rear-facing, strongest thrust) and
  **maneuvering thrusters** (weaker, all directions)
- Moving forward (direction of main engine) gives highest acceleration and
  top speed
- Moving in other directions uses maneuvering thrusters — slower acceleration,
  lower max speed
- Turning has a rate limit. A 180-degree turn order means the ship must
  decelerate, rotate, and accelerate again. You see this play out.
- Momentum carries: cutting engines doesn't stop you instantly, you drift.

### Waypoint queues

Shift+click to queue waypoints. Ships follow the path in sequence. Essential
for planning routes through asteroid fields and flanking maneuvers.

### Facing control

Separate from movement direction. You can order a ship to move in one
direction while *facing* another. This allows:
- Presenting a narrow profile while maneuvering (nose toward enemy)
- Keeping weapons trained on a target while repositioning
- Showing your armored front while retreating

Turn rate limits how fast facing can change. Large ships turn slowly.

### No autonomous engagement AI

Ships do what you tell them. You give movement orders and you designate
targets. The ship fires when it can (has lock/track, weapon in range, ammo
available). There is no "attack move" or "retreat" behavior — the player
is the tactical AI. The skill is in managing multiple ships, giving the
right orders at the right time.

### What this adds

"My destroyer is in a bad spot — enemy railgun cruiser has a lock. I order
a hard turn to present my nose, queue waypoints behind the asteroid cluster,
and cut engines to reduce signature. The turn takes three seconds and I'm
holding my breath the whole time."

---

## 6. Control Points & Win Conditions

The game needs a reason to fight beyond annihilation. Control points create
territorial pressure and force engagements.

### Control point mechanics

- Map has 1-3 capture zones (marked areas)
- A team controls a point if they have ships inside it and the enemy doesn't
- Contested (both teams present) = no one scores
- Each controlled point generates ticks over time
- First team to reach the tick threshold wins, OR destroy all enemy ships

### Why this matters

Without control points, the optimal strategy is to hide and wait. Control
points force you to move, to contest space, to expose your fleet. They
create the engagements that make everything else matter.

### Map design

Maps should have structure:
- Dense asteroid fields creating chokepoints and cover lanes
- Open spaces between clusters (killing fields)
- Control points placed to force movement through interesting terrain
- Procedurally generated with these structural rules

### What this adds

"They're holding B point behind the asteroid cluster. I can't see them but
they're scoring. I need to push through the chokepoint or send scouts around
the long way to get eyes on their position before committing my fleet."

---

## 7. Multiplayer

Everything above is designed for PvP. Authoritative server fits the
simulation-first approach.

### Why authoritative server

The game is heavily simulated: projectile trajectories, physics-based
movement, sensor models, damage calculations. All of this needs to run in
one place to be consistent. Clients send orders (move here, fire at that,
turn on radar). The server simulates everything and replicates the results.

This also naturally handles fog of war: the server only sends each client
the entities their team can detect. Undetected enemies don't exist on the
client. No client-side cheating for information.

### Latency

Tactical games are order-based, not twitch. You give an order, the ship
begins executing it — this inherently masks network latency. A 100ms delay
before your ship starts turning is invisible because the turn itself takes
seconds. This is ideal for authoritative server architecture.

### Tech

`bevy_replicon` for entity replication and event transport. It handles the
hard parts (entity mapping, component sync, client/server world separation)
and lets us focus on game logic.

### Phases

1. **Shared-screen prototype** — two teams controlled from one instance,
   camera switches between turns or split-screen. Proves PvP works.
2. **Client/server split** — server runs simulation, clients render and send
   commands. LAN first.
3. **Online** — relay server, matchmaking, handle real-world latency.

---

## Priority Recommendation

Given the simulation-first philosophy and what creates the most gameplay
per unit of work:

### Phase 1: Core Simulation Foundation
1. **Physics-based movement** — momentum, turn rates, acceleration. This is
   the foundation everything else builds on. Without it, nothing feels like
   Nebulous.
2. **Control points + win condition** — gives the game a loop beyond "find
   and kill." Forces engagement. Small scope, massive gameplay impact.

### Phase 2: Fleet & Loadouts
3. **Ship classes (holy trinity)** — battleship, destroyer, scout with
   different stats. Even with identical weapons this changes gameplay.
4. **Mount point system + weapon variety** — cannons and missiles as distinct
   families, ammo, basic PD. This is where fleet composition becomes
   meaningful.
5. **Fleet composition screen** — pre-game loadout selection with point budget.

### Phase 3: Sensors & EW
6. **Detection model** (radar/passive/signatures) — replaces simple vision
   range with the layered sensor game. Lock vs track distinction.
7. **Fire control integration** — weapon accuracy depends on lock quality.
   Makes the sensor game directly affect combat.

### Phase 4: Depth
8. **Directional damage model** — system damage based on hit angle. Ship
   facing becomes tactically critical.
9. **Facing control** — separate movement and facing orders.
10. **Waypoint queuing** — shift-click path planning.

### Phase 5: Multiplayer
11. **Shared-screen PvP prototype** — proves the game works as PvP.
12. **Client/server with bevy_replicon** — real networked multiplayer.

### Rationale

Physics-based movement comes first because it changes how *everything* feels.
Control points come next because without them there's no game loop. Fleet
composition and weapons follow because they're the content that makes the
simulation interesting. Sensors and EW are phase 3 because they add the
deepest strategic layer but require the combat foundation. Multiplayer is
last because every feature before it makes PvP better, and building PvP
on a thin foundation wastes effort.
