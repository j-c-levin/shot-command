import * as admin from "firebase-admin";
admin.initializeApp();

export { createGame, listGames, getGame, joinGame, readyUp, launchGame, deleteGame } from "./games";
export { edgegapWebhook } from "./webhook";
export { cleanupStaleGames } from "./cleanup";
