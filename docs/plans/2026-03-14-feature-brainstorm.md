# Nebulous Lite — Feature Brainstorm

Ideas for fleshing out the core game loop, organized by pillar. Each section
describes the design space, what it adds to gameplay, and rough scope.

---

## 1. See Without Being Seen

**The idea:** Detection should be mutual and manageable. Right now the player
hunts a stationary enemy that can't see back. The game gets interesting when
both sides are trying to find each other while staying hidden.

### Detection profiles

Every ship emits a **signature** — a radius that determines how far away
enemies can detect it. Firing weapons temporarily spikes your signature
(muzzle flash). Moving fast increases it. Sitting still behind an asteroid
shrinks it. This creates a stealth/aggression tradeoff: shooting reveals you.

### Sensor types

- **Passive sensors** — detect enemies by their signature. Long range but
  imprecise (you know "something is over there" but not exactly where).
  Reveals a fuzzy contact on the map.
- **Active sensors (ping)** — send out a radar pulse that precisely locates
  everything in range, but also reveals your position to everyone. High risk,
  high reward. Could be a manual ability with a cooldown.

### Hiding mechanics

- Asteroids block line of sight (already implemented)
- Engine cuts: order a ship to go silent (no movement, reduced signature)
- Decoys: expendable that creates a false signature blip

### What this adds

Mind games. "Do I ping and reveal myself to find their fleet, or stay dark
and risk being flanked?" Every detection decision has consequences.

### Scope

Medium. Signature is a float on ShipStats, modified by state (moving, firing,
silent). Passive detection = compare signature vs distance. Active ping =
temporary full-radius reveal + self-reveal. Decoys are spawnable entities
with a fake signature.

---

## 2. Choose Your Engagement Range

**The idea:** Weapons should have distinct ranges and behaviors that make
distance the central tactical variable. Closing range should be a deliberate,
risky decision.

### Weapon classes

- **Cannons** — short range (~100-150), rapid fire, high damage, hitscan or
  very fast projectiles. Devastating up close but you have to get there.
- **Missiles** — long range (~300-400), slow-moving, fire-and-forget. Can be
  shot down by point defense. Good for opening engagements from safety.
- **Torpedoes** — medium range (~200), slow, very high damage, limited ammo.
  A "commitment" weapon — you launch when you mean it.

### Point defense

Ships can have a PD system that automatically shoots down incoming missiles
within a short radius (~30). Creates a missile vs PD economy: saturate their
PD with enough missiles and some get through. This is the core Nebulous
dynamic.

### Weapon mounts

Each ship has a fixed number of weapon slots. Loadout choices happen before
the mission (fleet composition screen). A destroyer might have 2 slots: you
could go double cannon (brawler), cannon + missile (flexible), or double
missile (standoff).

### Ammo

Missiles and torpedoes have limited ammo. Cannons might be unlimited or have
a large pool. Ammo pressure forces you to make shots count and creates
late-game tension when reserves run low.

### What this adds

Range becomes the game. "Their fleet has missiles, I need to close fast behind
asteroid cover before they whittle me down. But if they have hidden cannons
up close..." Every engagement plays out differently based on loadouts.

### Scope

Large. Weapon component system, multiple projectile types with different
speeds/damage/ranges, PD auto-intercept system, ammo tracking, fleet
composition screen (pre-game UI). Could be broken into phases: first add
weapon variety, then PD, then ammo, then fleet comp screen.

---

## 3. Manage Damage, Not Just HP

**The idea:** Hits should break specific systems, not just reduce a number.
A crippled ship that limps home is more dramatic than one that explodes at
0 HP.

### Ship systems as components

Instead of a single HP pool, ships have discrete systems:
- **Hull** — overall structural integrity. At 0, ship is destroyed.
- **Engines** — when damaged, speed is reduced. At 0, ship is immobilized.
- **Sensors** — when damaged, vision range shrinks. At 0, ship is blind.
- **Weapons** — when damaged, fire rate drops. At 0, ship can't shoot.
- **Reactor** — powers everything. Damage here degrades all systems.

### Damage model

Hits land on a random (or directional) system. Armor could reduce damage to
certain systems. This creates emergent stories: "My destroyer's engines are
shot, but its guns still work — I'll make a last stand here."

### Repair

Slow passive repair on non-destroyed systems. Or a dedicated repair ship class.
Creates decisions: retreat to repair, or press the attack with damaged ships?

### Visual feedback

Damaged ships could show visual indicators — sparks, reduced glow, limping
movement animation. Even simple color shifts (green → yellow → red on the
ship mesh) communicate a lot.

### What this adds

Decisions after contact. Instead of "focus fire until dead," you think about
which systems to target, whether to retreat damaged ships, whether a blind
ship is still useful as bait.

### Scope

Medium-large. Replace Health with a SystemHealth component containing multiple
floats. Damage distribution logic. System degradation (speed = base_speed *
engine_health_pct). Visual feedback. Could start simple: just hull + engines,
add more systems later.

---

## 4. Fleet Composition Tradeoffs

**The idea:** The game starts before the battle — choosing which ships to bring.
Different ship classes with different strengths force strategic decisions.

### Ship classes

- **Scout** — fast, long vision range, low HP, light weapons. Finds the enemy.
- **Destroyer** — medium speed, medium HP, good weapons. The workhorse.
- **Cruiser** — slow, high HP, heavy weapons, short vision. Needs scouts to
  find targets. The hammer.
- **Support** — slow, low HP, repair/ECM abilities. Force multiplier.

### Point budget

Each mission gives you N points. Scouts are cheap, cruisers expensive. You
can't bring everything. A fleet of 5 scouts finds the enemy fast but can't
kill a cruiser. Two cruisers hit hard but get ambushed without scouts.

### Pre-game screen

Simple UI: list of ship classes with stats and costs. Click to add to fleet.
See total points. Confirm and start. No need for deep customization at first —
class = fixed loadout.

### What this adds

Replayability. Every game plays differently based on your fleet choice. Creates
a metagame: "Last round they brought all missiles, so I'll bring PD-heavy
destroyers this time."

### Scope

Medium. Ship class data (stats per class), fleet selection UI (2D overlay),
spawning multiple ships from selection, group selection/movement commands.
The UI is the biggest piece. Could start with fixed fleets (3 scouts + 1
destroyer) and add the selection screen later.

---

## 5. Maneuvering as Survival

**The idea:** Movement should feel tactical, not just "click to go." The map
and positioning should create meaningful decisions every moment.

### Waypoint queues

Shift+click to queue waypoints. Ships follow the path in order. This lets you
plan routes through asteroid fields, flanking maneuvers, patrol routes.

### Speed control

Ships have throttle levels (full, half, stop). Full speed = faster but higher
signature and wider turn radius. Half speed = stealthier, tighter turns.
Creates a tension between urgency and stealth.

### Engagement behaviors

- **Attack move** — move to destination, but stop and engage any enemy in range
- **Retreat** — move to destination, don't stop for anything, even if fired upon
- **Hold position** — stay put, engage anything in range

### Map design

Asteroid fields as terrain features, not random scatter. Chokepoints between
dense clusters. Open "killing fields" where you're exposed. High ground
equivalent: elevated positions (literal in 3D) with better vision.
Procedurally generated maps with these features.

### What this adds

"Positioning is the game." Every second you're deciding: push through the
chokepoint or go the long way around? Hold this asteroid for cover or advance?
The map stops being a flat arena and becomes the tactical puzzle.

### Scope

Small-medium per feature. Waypoint queuing is small. Speed control is small.
Engagement behaviors are medium. Map generation is large. These are mostly
independent and can ship incrementally.

---

## 6. Multiplayer

**The idea:** Everything above is designed for PvP. The real game is Nebulous
Lite against another human.

### Architecture approach

The codebase already uses `Team(u8)` for multiplayer extensibility. The key
architectural decisions:

- **Authoritative server** — one instance runs the simulation, clients send
  commands and receive state. Prevents cheating, simplest mental model. Latency
  is acceptable for a tactical game (not twitch).
- **Lockstep** — both clients run the same simulation, only commands are sent.
  Lower bandwidth but requires deterministic simulation (floats are tricky).

Authoritative server is the safer choice for a first implementation.

### Networking stack

Bevy ecosystem options:
- **`bevy_replicon`** — replication framework, entity sync, works with various
  transports. Most mature Bevy networking crate.
- **`lightyear`** — prediction, interpolation, rollback. More complex but
  handles latency better.
- **Raw WebSocket/WebRTC** — full control, more work.

`bevy_replicon` is likely the right starting point. It handles entity
replication (spawn a ship on server, it appears on client) and event
replication (send a move command, server processes it).

### What changes

- Input becomes "send command to server" instead of "directly modify components"
- Fog of war becomes real: each client only sees what their team detects
- Victory condition checks run on the server
- Need a lobby/matchmaking flow (even just "host game / join game")

### Fog of war in multiplayer

This is where the detection system really shines. Each client receives only
the entities their team can detect. Undetected enemies simply don't exist on
the client. The server runs detection for all teams and filters replication.

### Phases

1. **Local two-player** — both players on same machine, split input. Proves
   the game works as PvP without networking complexity.
2. **Networked** — server/client split with `bevy_replicon`. Start with LAN.
3. **Online** — matchmaking, relay server, handle real latency.

### What this adds

The actual game. Nebulous is fundamentally about outsmarting another human.
AI opponents are a stepping stone, but PvP is the goal.

### Scope

Large. Networking is always the hardest part. But the architecture (flat
plugins, Team component, command-based input, detection system) is designed
for it. Phase 1 (local two-player) is actually quite small — just duplicate
input handling per team and restrict camera/visibility per team.

---

## Priority Recommendation

If I had to pick an order that builds the most compelling game loop fastest:

1. **Fleet composition** (multiple ships + classes) — transforms it from a
   demo into a game. Even without weapon variety, moving a fleet feels good.
2. **See without being seen** (signatures + active ping) — adds the core
   Nebulous tension. Makes maneuvering meaningful.
3. **Engagement range** (weapon types + PD) — gives fleet composition purpose.
   Different ships want different ranges.
4. **Maneuvering** (waypoints, speed control, behaviors) — quality of life
   that makes tactical play feel good.
5. **Damage model** (system damage) — deepens combat but requires the above
   to really shine.
6. **Multiplayer** — the endgame. Everything above makes PvP worth playing.

But you know your vision best — these can be reordered based on what excites
you.
