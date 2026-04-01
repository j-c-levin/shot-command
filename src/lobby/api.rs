use std::sync::mpsc::{Receiver, channel};

use super::{GameDetail, GameInfo};

/// Check the server API version. Returns the version string.
pub fn check_version(api_base: &str) -> Receiver<Result<String, String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/version");
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let result = client
            .get(&url)
            .send()
            .map_err(|e| e.to_string())
            .and_then(|r| r.json::<serde_json::Value>().map_err(|e| e.to_string()))
            .and_then(|v| {
                v.get("version")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| "missing version in response".to_string())
            });
        let _ = tx.send(result);
    });
    rx
}

/// List all open games from the lobby API.
pub fn list_games(api_base: &str) -> Receiver<Result<Vec<GameInfo>, String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/listGames");
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let result = client
            .get(&url)
            .send()
            .map_err(|e| e.to_string())
            .and_then(|r| r.json::<Vec<GameInfo>>().map_err(|e| e.to_string()));
        let _ = tx.send(result);
    });
    rx
}

/// Create a new game. Returns the game ID on success.
pub fn create_game(
    api_base: &str,
    creator: &str,
    map: Option<&str>,
    team_count: Option<u8>,
    players_per_team: Option<u8>,
) -> Receiver<Result<String, String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/createGame");
    let mut body = serde_json::json!({
        "creator": creator,
        "map": map,
    });
    if let Some(tc) = team_count {
        body["team_count"] = serde_json::json!(tc);
    }
    if let Some(ppt) = players_per_team {
        body["players_per_team"] = serde_json::json!(ppt);
    }
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let result = client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| e.to_string())
            .and_then(|r| {
                r.json::<serde_json::Value>()
                    .map_err(|e| e.to_string())
                    .and_then(|v| {
                        v.get("gameId")
                            .and_then(|id| id.as_str())
                            .map(|s| s.to_string())
                            .ok_or_else(|| "missing gameId in response".to_string())
                    })
            });
        let _ = tx.send(result);
    });
    rx
}

/// Get full details of a specific game.
pub fn get_game(api_base: &str, game_id: &str) -> Receiver<Result<GameDetail, String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/getGame/{game_id}");
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let result = client
            .get(&url)
            .send()
            .map_err(|e| e.to_string())
            .and_then(|r| r.json::<GameDetail>().map_err(|e| e.to_string()));
        let _ = tx.send(result);
    });
    rx
}

/// Join an existing game.
pub fn join_game(api_base: &str, game_id: &str, name: &str) -> Receiver<Result<(), String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/joinGame/games/{game_id}/join");
    let body = serde_json::json!({ "name": name });
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let result = client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| e.to_string())
            .and_then(|r| {
                if r.status().is_success() {
                    Ok(())
                } else {
                    let status = r.status();
                    let body = r.text().unwrap_or_default();
                    Err(if body.is_empty() {
                        format!("join failed: {status}")
                    } else {
                        body
                    })
                }
            });
        let _ = tx.send(result);
    });
    rx
}

/// Mark this player as ready (or not ready) in the lobby.
pub fn ready_up(api_base: &str, game_id: &str, name: &str, ready: bool) -> Receiver<Result<(), String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/readyUp");
    let body = serde_json::json!({
        "game_id": game_id,
        "name": name,
        "ready": ready,
    });
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let result = client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| e.to_string())
            .and_then(|r| {
                if r.status().is_success() {
                    Ok(())
                } else {
                    Err(format!("ready_up failed: {}", r.status()))
                }
            });
        let _ = tx.send(result);
    });
    rx
}

/// Launch a game (creator only). Triggers server deployment.
pub fn launch_game(
    api_base: &str,
    game_id: &str,
    creator: &str,
) -> Receiver<Result<(), String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/launchGame/games/{game_id}/launch");
    let body = serde_json::json!({ "creator": creator });
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let result = client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| e.to_string())
            .and_then(|r| {
                if r.status().is_success() {
                    Ok(())
                } else {
                    Err(format!("launch failed: {}", r.status()))
                }
            });
        let _ = tx.send(result);
    });
    rx
}

/// Leave or delete a game from the lobby.
/// Creator leaving deletes the game; non-creator just removes themselves.
pub fn delete_game(api_base: &str, game_id: &str, player_name: &str) -> Receiver<Result<(), String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/deleteGame/{game_id}?player={player_name}");
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let result = client
            .delete(&url)
            .send()
            .map_err(|e| e.to_string())
            .and_then(|r| {
                if r.status().is_success() {
                    Ok(())
                } else {
                    Err(format!("delete failed: {}", r.status()))
                }
            });
        let _ = tx.send(result);
    });
    rx
}

/// Close a completed game (delete from Firestore). Fire-and-forget.
pub fn close_game(api_base: &str, game_id: &str) -> Receiver<Result<(), String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/closeGame");
    let body = serde_json::json!({ "game_id": game_id });
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let result = client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| e.to_string())
            .and_then(|r| {
                if r.status().is_success() {
                    Ok(())
                } else {
                    Err(format!("close_game failed: {}", r.status()))
                }
            });
        let _ = tx.send(result);
    });
    rx
}

/// Switch a player to a different team in the lobby.
pub fn switch_team(
    api_base: &str,
    game_id: &str,
    name: &str,
    target_team: u8,
) -> Receiver<Result<(), String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/switchTeam");
    let body = serde_json::json!({
        "game_id": game_id,
        "name": name,
        "target_team": target_team,
    });
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let result = client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| e.to_string())
            .and_then(|r| {
                if r.status().is_success() {
                    Ok(())
                } else {
                    let msg = r.text().unwrap_or_default();
                    Err(msg)
                }
            });
        let _ = tx.send(result);
    });
    rx
}

/// Fetch the list of available map names from the lobby API.
pub fn fetch_maps(api_base: &str) -> Receiver<Result<Vec<String>, String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/maps");
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let result = client
            .get(&url)
            .send()
            .map_err(|e| e.to_string())
            .and_then(|r| r.json::<Vec<String>>().map_err(|e| e.to_string()));
        let _ = tx.send(result);
    });
    rx
}
