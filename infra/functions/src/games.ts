import * as admin from "firebase-admin";
import { onRequest } from "firebase-functions/v2/https";

const db = admin.firestore();

export const createGame = onRequest(async (req, res) => {
  if (req.method !== "POST") { res.status(405).send("Method not allowed"); return; }
  const { creator, map } = req.body;
  if (!creator) { res.status(400).send("creator required"); return; }

  const doc = await db.collection("games").add({
    creator,
    status: "waiting",
    players: [{ name: creator, team: 0 }],
    server_address: null,
    edgegap_request_id: null,
    created_at: admin.firestore.FieldValue.serverTimestamp(),
    map: map || null,
  });
  res.json({ gameId: doc.id });
});

export const listGames = onRequest(async (req, res) => {
  if (req.method !== "GET") { res.status(405).send("Method not allowed"); return; }
  const snapshot = await db.collection("games")
    .where("status", "==", "waiting")
    .orderBy("created_at", "desc")
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

export const getGame = onRequest(async (req, res) => {
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

export const joinGame = onRequest(async (req, res) => {
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
      players: [...data.players, { name, team: 1 }],
    });
  });
  res.json({ ok: true });
});

export const launchGame = onRequest(async (req, res) => {
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

  // Call Edgegap Deploy API
  const edgegapToken = process.env.EDGEGAP_API_TOKEN;
  const edgegapApp = process.env.EDGEGAP_APP_NAME;
  const edgegapVersion = process.env.EDGEGAP_APP_VERSION || "latest";
  const webhookUrl = process.env.EDGEGAP_WEBHOOK_URL;

  if (!edgegapToken || !edgegapApp) {
    // Dev mode: no Edgegap, return localhost
    await gameRef.update({
      status: "ready",
      server_address: "127.0.0.1:5000",
    });
    res.json({ ok: true, dev_mode: true });
    return;
  }

  const deployPayload = {
    application: edgegapApp,
    version: edgegapVersion,
    env_vars: data.map ? [{ key: "GAME_MAP", value: data.map, is_hidden: false }] : [],
    webhook_on_ready: webhookUrl ? { url: webhookUrl } : undefined,
  };

  const deployRes = await fetch("https://api.edgegap.com/v2/deployments", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `token ${edgegapToken}`,
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

export const deleteGame = onRequest(async (req, res) => {
  if (req.method !== "DELETE") { res.status(405).send("Method not allowed"); return; }
  const gameId = req.path.split("/").pop();
  if (!gameId) { res.status(400).send("game ID required"); return; }

  // For simplicity: just delete the doc. In production, check authorization.
  await db.collection("games").doc(gameId).delete();
  res.json({ ok: true });
});
