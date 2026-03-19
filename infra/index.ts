import * as pulumi from "@pulumi/pulumi";
import * as gcp from "@pulumi/gcp";
import * as command from "@pulumi/command";
import * as path from "path";

const project = gcp.config.project!;
const region = gcp.config.region || "europe-west2";

// Resolve paths relative to this file (infra/index.ts)
const infraDir = __dirname;
const functionsDir = path.join(infraDir, "functions");

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

// Firestore database (Native mode)
const firestore = new gcp.firestore.Database("default", {
  locationId: region,
  type: "FIRESTORE_NATIVE",
}, { dependsOn: [firestoreApi], protect: true });

// Install, build, and deploy Cloud Functions via firebase CLI
const functionsInstall = new command.local.Command("functions-install", {
  dir: functionsDir,
  create: "npm ci",
});

const functionsBuild = new command.local.Command("functions-build", {
  dir: functionsDir,
  create: "npm run build",
}, { dependsOn: [functionsInstall] });

const functionsDeploy = new command.local.Command("functions-deploy", {
  dir: infraDir,
  create: `npx firebase deploy --only functions,firestore --project ${project} --force`,
  environment: {
    GOOGLE_APPLICATION_CREDENTIALS: process.env.GOOGLE_APPLICATION_CREDENTIALS || "",
  },
}, { dependsOn: [functionsBuild, cloudfunctionsApi, cloudbuildApi, artifactRegistryApi, firestore] });

// Export the functions base URL
export const functionsBaseUrl = pulumi.interpolate`https://${region}-${project}.cloudfunctions.net`;
export const projectId = project;
