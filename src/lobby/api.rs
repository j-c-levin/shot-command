use std::sync::mpsc::{Receiver, channel};

use super::{GameDetail, GameInfo};

/// List all open games from the lobby API.
pub fn list_games(api_base: &str) -> Receiver<Result<Vec<GameInfo>, String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/games");
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
) -> Receiver<Result<String, String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/games");
    let body = serde_json::json!({
        "creator": creator,
        "map": map,
    });
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
    let url = format!("{api_base}/games/{game_id}");
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
    let url = format!("{api_base}/games/{game_id}/join");
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
                    Err(format!("join failed: {}", r.status()))
                }
            });
        let _ = tx.send(result);
    });
    rx
}

/// Launch a game (creator only). Triggers server deployment.
pub fn launch_game(api_base: &str, game_id: &str) -> Receiver<Result<(), String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/games/{game_id}/launch");
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let result = client
            .post(&url)
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

/// Delete a game from the lobby.
pub fn delete_game(api_base: &str, game_id: &str) -> Receiver<Result<(), String>> {
    let (tx, rx) = channel();
    let url = format!("{api_base}/games/{game_id}");
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
