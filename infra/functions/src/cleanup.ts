import * as admin from "firebase-admin";
import { onSchedule } from "firebase-functions/v2/scheduler";

const db = admin.firestore();

export const cleanupStaleGames = onSchedule({ schedule: "every 10 minutes", region: "europe-west2" }, async () => {
  const cutoff = new Date(Date.now() - 30 * 60 * 1000); // 30 min ago
  const snapshot = await db.collection("games")
    .where("created_at", "<", cutoff)
    .where("status", "in", ["waiting", "launching"])
    .get();

  const batch = db.batch();
  snapshot.docs.forEach(doc => batch.delete(doc.ref));
  await batch.commit();

  console.log(`Cleaned up ${snapshot.size} stale games`);
});
