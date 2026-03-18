import * as admin from "firebase-admin";
import { onRequest } from "firebase-functions/v2/https";

const db = admin.firestore();

export const edgegapWebhook = onRequest(async (req, res) => {
  if (req.method !== "POST") { res.status(405).send("Method not allowed"); return; }

  const { request_id, fqdn, ports } = req.body;
  if (!request_id) { res.status(400).send("request_id required"); return; }

  // Find the game with this edgegap_request_id
  const snapshot = await db.collection("games")
    .where("edgegap_request_id", "==", request_id)
    .limit(1)
    .get();

  if (snapshot.empty) {
    res.status(404).send("no game found for this deployment");
    return;
  }

  const doc = snapshot.docs[0];

  // Extract connection info from Edgegap response
  const gamePort = ports?.gameport;
  const externalPort = gamePort?.external;
  const publicIp = fqdn || req.body.public_ip;

  if (!publicIp || !externalPort) {
    res.status(400).send("missing connection info");
    return;
  }

  await doc.ref.update({
    status: "ready",
    server_address: `${publicIp}:${externalPort}`,
  });

  res.json({ ok: true });
});
