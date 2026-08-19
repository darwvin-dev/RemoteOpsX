import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const appBinaryPath = process.env.REMOTEOPSX_E2E_BINARY
  ? path.resolve(process.env.REMOTEOPSX_E2E_BINARY)
  : path.join(root, "src-tauri", "target", "debug", process.platform === "win32" ? "remoteopsx.exe" : "remoteopsx");

export const config = {
  runner: "local",
  specs: [path.join(root, "e2e", "specs", "**", "*.e2e.mjs")],
  maxInstances: 1,
  logLevel: process.env.CI ? "warn" : "info",
  bail: 0,
  waitforTimeout: 10_000,
  connectionRetryTimeout: 90_000,
  connectionRetryCount: 1,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: {
    ui: "bdd",
    timeout: 60_000,
  },
  services: [["@wdio/tauri-service", {
    appBinaryPath,
    driverProvider: "embedded",
    embeddedPort: 4445,
  }]],
  capabilities: [{
    browserName: "tauri",
    "tauri:options": {
      application: appBinaryPath,
    },
  }],
};
