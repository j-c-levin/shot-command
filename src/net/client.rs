use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    RenetChannelsExt, RenetClient,
    netcode::{ClientAuthentication, NetcodeClientTransport},
    renet::ConnectionConfig,
};

use crate::game::{GameState, Team};
use crate::input::on_ground_clicked;
use crate::map::{GroundPlane, MapBounds};
use crate::net::commands::{GameResult, GameStarted, LobbyStatus, TeamAssignment};
use crate::net::{LocalTeam, PROTOCOL_ID};

/// Resource containing the server address to connect to.
#[derive(Resource, Debug, Clone)]
pub struct ClientConnectAddress(pub String);

pub struct ClientNetPlugin;

impl Plugin for ClientNetPlugin {
    fn build(&self, app: &mut App) {
        // Replication registration is handled by SharedReplicationPlugin
        // (must be added before this plugin in both server and client).

        // Systems
        app.init_resource::<super::materializer::DebugVisuals>();
        app.add_systems(OnEnter(GameState::Connecting), setup_renet_client);
        app.add_systems(OnEnter(GameState::Playing), client_setup_scene);
        app.add_systems(
            Update,
            (
                super::materializer::materialize_ships,
                super::materializer::materialize_asteroids,
                super::materializer::materialize_projectiles,
                super::materializer::materialize_missiles,
                super::materializer::materialize_explosions,
                super::materializer::materialize_laser_beams,
                super::materializer::update_laser_beam_meshes,
                super::materializer::draw_targeting_gizmos,
                super::materializer::update_ship_number_labels,
                super::materializer::update_squad_connection_lines,
                super::materializer::toggle_debug_visuals,
                super::materializer::spawn_debug_seeker_cones,
                super::materializer::update_debug_seeker_cones,
                super::materializer::draw_pd_range_gizmos,
                super::materializer::update_enemy_number_labels,
            )
                .run_if(in_state(GameState::Playing)),
        );

        // Init lobby state resource
        app.init_resource::<CurrentLobbyState>();

        // Observer for team assignment from server
        app.add_observer(on_team_assignment);

        // Observer for lobby status updates from server
        app.add_observer(on_lobby_status);

        // Observer for game started from server
        app.add_observer(on_game_started);

        // Observer for game result from server
        app.add_observer(on_game_result);

        // Game over UI
        app.add_systems(OnEnter(GameState::GameOver), show_game_over_ui);
        app.add_systems(OnExit(GameState::GameOver), despawn_game_over_ui);
        app.add_systems(
            Update,
            handle_return_to_menu.run_if(in_state(GameState::GameOver)),
        );
    }
}

/// Sets up the renet client and transport when entering Connecting state.
fn setup_renet_client(
    mut commands: Commands,
    channels: Res<RepliconChannels>,
    connect_address: Res<ClientConnectAddress>,
) {
    let server_addr: SocketAddr = connect_address
        .0
        .parse()
        .expect("Invalid server address format");

    let server_channels_config = channels.server_configs();
    let client_channels_config = channels.client_configs();

    let client = RenetClient::new(ConnectionConfig {
        server_channels_config,
        client_channels_config,
        ..Default::default()
    });

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();

    // Use current time as a unique client ID
    let client_id = current_time.as_millis() as u64;

    let socket =
        UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).expect("Failed to bind client UDP socket");

    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };

    let transport = NetcodeClientTransport::new(current_time, authentication, socket)
        .expect("Failed to create client transport");

    commands.insert_resource(client);
    commands.insert_resource(transport);

    info!("Client connecting to {}...", server_addr);
}

/// Observer that fires when the server sends a TeamAssignment event.
fn on_team_assignment(
    trigger: On<TeamAssignment>,
    mut local_team: ResMut<LocalTeam>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let assignment = &*trigger;
    let team = assignment.team;

    info!("Received team assignment: Team({})", team.0);
    local_team.0 = Some(team);

    next_state.set(GameState::FleetComposition);
    info!("Transitioning to FleetComposition state");
}

/// Tracks the current lobby state as reported by the server.
#[derive(Resource, Debug, Clone, Default)]
pub struct CurrentLobbyState(pub Option<crate::net::commands::LobbyState>);

/// Observer that fires when the server sends a LobbyStatus event.
fn on_lobby_status(trigger: On<LobbyStatus>, mut lobby_state: ResMut<CurrentLobbyState>) {
    let status = &*trigger;
    info!("Lobby status update: {:?}", status.state);
    lobby_state.0 = Some(status.state.clone());
}

/// Observer that fires when the server sends a GameStarted event.
fn on_game_started(
    _trigger: On<GameStarted>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    info!("Game started — transitioning to Playing");
    next_state.set(GameState::Playing);
}

/// Sets up the client scene when entering Playing state:
/// ground plane (for click-to-move picking) and MapBounds resource.
/// Note: Camera, ambient light, and directional light are handled by CameraPlugin.
fn client_setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Insert MapBounds resource (same values as MapPlugin)
    let bounds = MapBounds {
        half_extents: Vec2::splat(500.0),
    };
    commands.insert_resource(bounds.clone());

    // Ground plane for click-to-move picking (3x map bounds so edge clicks always register)
    let size = bounds.size() * 3.0;
    commands.spawn((
        GroundPlane,
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::new(size.x / 2.0, size.y / 2.0)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.0, 0.0, 0.0, 0.0),
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Pickable::default(),
    ));

    // Register the global ground-click observer for move commands
    commands.add_observer(on_ground_clicked);

    info!("Client scene setup complete (ground plane + map bounds + observers)");
}

/// Stores the game outcome once the server announces it.
#[derive(Resource, Debug)]
pub struct GameOutcome(pub Team);

/// Observer that fires when the server sends a GameResult event.
fn on_game_result(
    trigger: On<GameResult>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let result = &*trigger;
    info!("Game result received: Team {} wins!", result.winning_team.0);
    commands.insert_resource(GameOutcome(result.winning_team));
    next_state.set(GameState::GameOver);
}

/// Marker for the game over UI root (for despawn on state exit).
#[derive(Component)]
struct GameOverRoot;

/// Marker for the "Return to Menu" button.
#[derive(Component)]
struct ReturnToMenuButton;

/// Display Victory/Defeat UI text when entering GameOver state.
fn show_game_over_ui(
    mut commands: Commands,
    game_outcome: Option<Res<GameOutcome>>,
    local_team: Res<LocalTeam>,
) {
    let Some(outcome) = game_outcome else {
        warn!("GameOver entered but no GameOutcome resource found");
        return;
    };

    let is_victory = local_team
        .0
        .map(|t| t == outcome.0)
        .unwrap_or(false);

    let (text, color) = if is_victory {
        ("Victory!", Color::srgb(0.2, 1.0, 0.2))
    } else {
        ("Defeat", Color::srgb(1.0, 0.2, 0.2))
    };

    commands
        .spawn((
            GameOverRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(30.0),
                ..default()
            },
            GlobalZIndex(5),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new(text),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(color),
            ));

            // "Return to Menu" button
            root.spawn((
                ReturnToMenuButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.2, 0.35)),
            ))
            .with_child((
                Text::new("Return to Menu"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });

    info!("Game over: {}", text);
}

/// Handle clicking "Return to Menu" in GameOver screen.
fn handle_return_to_menu(
    query: Query<&Interaction, (Changed<Interaction>, With<ReturnToMenuButton>)>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for interaction in &query {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::MainMenu);
        }
    }
}

/// Despawn game over UI when leaving GameOver state.
fn despawn_game_over_ui(mut commands: Commands, roots: Query<Entity, With<GameOverRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

