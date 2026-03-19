import * as pulumi from "@pulumi/pulumi";
import * as gcp from "@pulumi/gcp";
import * as command from "@pulumi/command";
import * as path from "path";

const project = gcp.config.project!;
const region = gcp.config.region || "europe-west2";

// Resolve paths relative to this file (infra/index.ts)
const infraDir = __dirname;
const functionsDir = path.join(infraDir, "functions");

// Force re-run of commands on every deploy
const deployTimestamp = new Date().toISOString();

// Enable required GCP APIs
const firestoreApi = new gcp.projects.Service("firestore-api", {
  service: "firestore.googleapis.com",
  disableOnDestroy: false,
});

const cloudfunctionsApi = new gcp.projects.Service("cloudfunctions-api", {
  service: "cloudfunctions.googleapis.com",
  disableOnDestroy: false,
});

const cloudbuildApi = new gcp.projects.Service("cloudbuild-api", {
  service: "cloudbuild.googleapis.com",
  disableOnDestroy: false,
});

const artifactRegistryApi = new gcp.projects.Service("artifactregistry-api", {
  service: "artifactregistry.googleapis.com",
  disableOnDestroy: false,
});

// Firestore database (Native mode) — already created manually, import it
const firestore = new gcp.firestore.Database("default", {
  locationId: region,
  type: "FIRESTORE_NATIVE",
}, {
  dependsOn: [firestoreApi],
  protect: true,
  import: "(default)",
});

// Install, build, and deploy Cloud Functions via firebase CLI
// triggers: force re-run on every pulumi up
const functionsInstall = new command.local.Command("functions-install", {
  dir: functionsDir,
  create: "npm ci",
  triggers: [deployTimestamp],
});

const functionsBuild = new command.local.Command("functions-build", {
  dir: functionsDir,
  create: "npm run build",
  triggers: [deployTimestamp],
}, { dependsOn: [functionsInstall] });

// Write .env file for Cloud Functions with Edgegap credentials
const functionsEnv = new command.local.Command("functions-env", {
  dir: functionsDir,
  create: [
    `echo "EDGEGAP_API_TOKEN=${process.env.EDGEGAP_API_TOKEN || ""}" > .env`,
    `echo "EDGEGAP_APP_NAME=${process.env.EDGEGAP_APP_NAME || ""}" >> .env`,
    `echo "EDGEGAP_APP_VERSION=${process.env.EDGEGAP_APP_VERSION || "0.1.0"}" >> .env`,
  ].join(" && "),
  triggers: [deployTimestamp],
}, { dependsOn: [functionsBuild] });

const functionsDeploy = new command.local.Command("functions-deploy", {
  dir: infraDir,
  create: `npx firebase deploy --only functions,firestore --project ${project} --force`,
  environment: {
    GOOGLE_APPLICATION_CREDENTIALS: process.env.GOOGLE_APPLICATION_CREDENTIALS || "",
  },
  triggers: [deployTimestamp],
}, { dependsOn: [functionsEnv, cloudfunctionsApi, cloudbuildApi, artifactRegistryApi, firestore] });

// Export the functions base URL
export const functionsBaseUrl = pulumi.interpolate`https://${region}-${project}.cloudfunctions.net`;
export const projectId = project;
