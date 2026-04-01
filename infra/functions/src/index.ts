import * as admin from "firebase-admin";
admin.initializeApp();

export { version, createGame, listGames, getGame, joinGame, readyUp, launchGame, deleteGame, closeGame, maps } from "./games";
export { edgegapWebhook } from "./webhook";
export { cleanupStaleGames } from "./cleanup";
