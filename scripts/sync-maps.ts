#!/usr/bin/env npx ts-node
/**
 * Syncs available map names from assets/maps/*.ron to Firestore config/maps document.
 * Run by CI/CD after deployment.
 *
 * Usage: GOOGLE_APPLICATION_CREDENTIALS=key.json FIREBASE_PROJECT_ID=xxx npx ts-node scripts/sync-maps.ts
 */
import * as admin from "firebase-admin";
import * as fs from "fs";
import * as path from "path";

const projectId = process.env.FIREBASE_PROJECT_ID;
if (!projectId) {
  console.error("FIREBASE_PROJECT_ID required");
  process.exit(1);
}

admin.initializeApp({ projectId });
const db = admin.firestore();

const mapsDir = path.resolve(__dirname, "../assets/maps");
const mapFiles = fs.readdirSync(mapsDir)
  .filter(f => f.endsWith(".ron"))
  .map(f => f.replace(".ron", ""));

async function main() {
  await db.collection("config").doc("maps").set({ maps: mapFiles });
  console.log(`Synced ${mapFiles.length} maps: ${mapFiles.join(", ")}`);
}

main().catch(e => { console.error(e); process.exit(1); });
