import * as admin from "firebase-admin";
import { FieldValue } from "firebase-admin/firestore";
import { onRequest } from "firebase-functions/v2/https";

const REGION = "europe-west2";
const db = admin.firestore();

// Edgegap config — set via firebase functions:config or env vars
const EDGEGAP_CONFIG = {
  apiToken: process.env.EDGEGAP_API_TOKEN || "",
  appName: process.env.EDGEGAP_APP_NAME || "",
  appVersion: process.env.EDGEGAP_APP_VERSION || "0.1.0",
  webhookUrl: process.env.EDGEGAP_WEBHOOK_URL || `https://${REGION}-nebulous-shot-command.cloudfunctions.net/edgegapWebhook`,
  lobbyApiUrl: process.env.LOBBY_API_URL || `https://${REGION}-nebulous-shot-command.cloudfunctions.net`,
};

export const createGame = onRequest({ region: REGION }, async (req, res) => {
  if (req.method !== "POST") { res.status(405).send("Method not allowed"); return; }
  const { creator, map } = req.body;
  if (!creator) { res.status(400).send("creator required"); return; }

  const doc = await db.collection("games").add({
    creator,
    status: "waiting",
    players: [{ name: creator, team: 0, ready: false }],
    server_address: null,
    edgegap_request_id: null,
    created_at: FieldValue.serverTimestamp(),
    map: map || null,
  });
  res.json({ gameId: doc.id });
});

export const listGames = onRequest({ region: REGION }, async (req, res) => {
  if (req.method !== "GET") { res.status(405).send("Method not allowed"); return; }
  const snapshot = await db.collection("games")
    .where("status", "==", "waiting")
    .limit(50)
    .get();

  const games = snapshot.docs.map(doc => ({
    game_id: doc.id,
    creator: doc.data().creator,
    player_count: doc.data().players?.length || 0,
    map: doc.data().map,
    status: doc.data().status,
    created_at: doc.data().created_at?.toDate?.()?.toISOString() || null,
  }));
  res.json(games);
});

export const getGame = onRequest({ region: REGION }, async (req, res) => {
  // Extract game ID from path: /games/GAME_ID
  const gameId = req.path.split("/").pop();
  if (!gameId) { res.status(400).send("game ID required"); return; }

  const doc = await db.collection("games").doc(gameId).get();
  if (!doc.exists) { res.status(404).send("game not found"); return; }
  const data = doc.data()!;

  res.json({
    game_id: doc.id,
    creator: data.creator,
    status: data.status,
    players: data.players || [],
    server_address: data.server_address,
    map: data.map,
  });
});

export const joinGame = onRequest({ region: REGION }, async (req, res) => {
  if (req.method !== "POST") { res.status(405).send("Method not allowed"); return; }
  const gameId = req.path.split("/").filter(Boolean).find((_, i, arr) => arr[i - 1] === "games");
  const { name } = req.body;
  if (!gameId || !name) { res.status(400).send("game ID and name required"); return; }

  const gameRef = db.collection("games").doc(gameId);
  await db.runTransaction(async (tx) => {
    const doc = await tx.get(gameRef);
    if (!doc.exists) throw new Error("game not found");
    const data = doc.data()!;
    if (data.status !== "waiting") throw new Error("game not accepting players");
    if (data.players.length >= 2) throw new Error("game full");
    tx.update(gameRef, {
      players: [...data.players, { name, team: 1, ready: false }],
    });
  });
  res.json({ ok: true });
});

export const readyUp = onRequest({ region: REGION }, async (req, res) => {
  if (req.method !== "POST") { res.status(405).send("Method not allowed"); return; }
  const { game_id, name, ready } = req.body;
  if (!game_id || !name) { res.status(400).send("game_id and name required"); return; }

  const gameRef = db.collection("games").doc(game_id);
  await db.runTransaction(async (tx) => {
    const doc = await tx.get(gameRef);
    if (!doc.exists) throw new Error("game not found");
    const data = doc.data()!;
    const players = data.players.map((p: { name: string; team: number; ready?: boolean }) =>
      p.name === name ? { ...p, ready: ready !== false } : p
    );
    tx.update(gameRef, { players });
  });
  res.json({ ok: true });
});

export const launchGame = onRequest({ region: REGION }, async (req, res) => {
  if (req.method !== "POST") { res.status(405).send("Method not allowed"); return; }
  const gameId = req.path.split("/").filter(Boolean).find((_, i, arr) => arr[i - 1] === "games");
  if (!gameId) { res.status(400).send("game ID required"); return; }

  const gameRef = db.collection("games").doc(gameId);
  const doc = await gameRef.get();
  if (!doc.exists) { res.status(404).send("game not found"); return; }
  const data = doc.data()!;

  // Only creator can launch
  if (req.body.creator !== data.creator) {
    res.status(403).send("only creator can launch");
    return;
  }
  if (data.players.length < 2) {
    res.status(400).send("need 2 players to launch");
    return;
  }
  const allReady = data.players.every((p: { ready?: boolean }) => p.ready === true);
  if (!allReady) {
    res.status(400).send("not all players are ready");
    return;
  }

  // Call Edgegap Deploy API
  if (!EDGEGAP_CONFIG.apiToken || !EDGEGAP_CONFIG.appName) {
    // Dev mode: no Edgegap, return localhost
    await gameRef.update({
      status: "ready",
      server_address: "127.0.0.1:5000",
    });
    res.json({ ok: true, dev_mode: true });
    return;
  }

  const deployPayload = {
    app_name: EDGEGAP_CONFIG.appName,
    version_name: EDGEGAP_CONFIG.appVersion,
    env_vars: [
      { key: "GAME_ID", value: gameId, is_hidden: false },
      { key: "LOBBY_API_URL", value: EDGEGAP_CONFIG.lobbyApiUrl, is_hidden: false },
      ...(data.map ? [{ key: "GAME_MAP", value: data.map, is_hidden: false }] : []),
    ],
    webhook_url: EDGEGAP_CONFIG.webhookUrl,
  };

  const deployRes = await fetch("https://api.edgegap.com/v1/deploy", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `token ${EDGEGAP_CONFIG.apiToken}`,
    },
    body: JSON.stringify(deployPayload),
  });

  if (!deployRes.ok) {
    const err = await deployRes.text();
    res.status(502).send(`Edgegap deploy failed: ${err}`);
    return;
  }

  const deployData = await deployRes.json() as { request_id: string };
  await gameRef.update({
    status: "launching",
    edgegap_request_id: deployData.request_id,
  });
  res.json({ ok: true, request_id: deployData.request_id });
});

export const deleteGame = onRequest({ region: REGION }, async (req, res) => {
  if (req.method !== "DELETE") { res.status(405).send("Method not allowed"); return; }
  const gameId = req.path.split("/").filter(Boolean)[0];
  const playerName = req.query.player as string;
  if (!gameId) { res.status(400).send("game ID required"); return; }

  const gameRef = db.collection("games").doc(gameId);
  const doc = await gameRef.get();
  if (!doc.exists) { res.status(404).send("game not found"); return; }
  const data = doc.data()!;

  if (!playerName || playerName === data.creator) {
    // Creator leaving — delete the whole game
    await gameRef.delete();
  } else {
    // Non-creator leaving — just remove from players array
    const updatedPlayers = (data.players || []).filter(
      (p: { name: string }) => p.name !== playerName
    );
    await gameRef.update({ players: updatedPlayers });
  }
  res.json({ ok: true });
});

export const closeGame = onRequest({ region: REGION }, async (req, res) => {
  if (req.method !== "POST") { res.status(405).send("Method not allowed"); return; }
  const { game_id } = req.body;
  if (!game_id) { res.status(400).send("game_id required"); return; }

  const gameRef = db.collection("games").doc(game_id);
  const doc = await gameRef.get();
  if (!doc.exists) { res.status(404).send("game not found"); return; }

  await gameRef.delete();
  res.json({ ok: true });
});
