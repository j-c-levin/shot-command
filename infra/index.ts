import * as pulumi from "@pulumi/pulumi";
import * as gcp from "@pulumi/gcp";
import * as command from "@pulumi/command";

const config = new pulumi.Config();
const project = gcp.config.project!;
const region = gcp.config.region || "us-central1";

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

// Firestore database (Native mode)
const firestore = new gcp.firestore.Database("default", {
  locationId: region,
  type: "FIRESTORE_NATIVE",
}, { dependsOn: [firestoreApi] });

// Deploy Cloud Functions via firebase CLI (simpler than individual gcp.cloudfunctionsv2.Function resources)
const functionsInstall = new command.local.Command("functions-install", {
  dir: `${pulumi.getProject()}/../infra/functions`,
  create: "npm ci",
});

const functionsBuild = new command.local.Command("functions-build", {
  dir: `${pulumi.getProject()}/../infra/functions`,
  create: "npm run build",
}, { dependsOn: [functionsInstall] });

// Deploy functions using firebase CLI
const functionsDeploy = new command.local.Command("functions-deploy", {
  dir: `${pulumi.getProject()}/../infra`,
  create: `npx firebase deploy --only functions --project ${project} --force`,
  environment: {
    GOOGLE_APPLICATION_CREDENTIALS: process.env.GOOGLE_APPLICATION_CREDENTIALS || "",
  },
}, { dependsOn: [functionsBuild, cloudfunctionsApi, firestore] });

// Export the functions base URL
export const functionsBaseUrl = pulumi.interpolate`https://${region}-${project}.cloudfunctions.net`;
export const projectId = project;
