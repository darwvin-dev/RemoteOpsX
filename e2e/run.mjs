import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const binary = process.env.REMOTEOPSX_E2E_BINARY
  ? path.resolve(process.env.REMOTEOPSX_E2E_BINARY)
  : path.join(root, "src-tauri", "target", "debug", process.platform === "win32" ? "remoteopsx.exe" : "remoteopsx");
const webdriverBase = `http://127.0.0.1:${process.env.TAURI_WEBDRIVER_PORT || "4445"}`;
const ELEMENT_KEY = "element-6066-11e4-a52e-4f735466cecf";

let appProcess;
let sessionId;
let appExited = false;
let appExitCode = null;

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function request(method, pathname, body, { allowProtocolError = false } = {}) {
  const response = await fetch(`${webdriverBase}${pathname}`, {
    method,
    headers: body === undefined ? undefined : { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  let payload = {};
  if (text) {
    try {
      payload = JSON.parse(text);
    } catch {
      throw new Error(`${method} ${pathname} returned non-JSON HTTP ${response.status}: ${text.slice(0, 500)}`);
    }
  }
  const protocolError = payload?.value?.error;
  if (!response.ok || (protocolError && !allowProtocolError)) {
    const message = payload?.value?.message || protocolError || text || `HTTP ${response.status}`;
    const error = new Error(`${method} ${pathname}: ${message}`);
    error.protocolError = protocolError;
    error.status = response.status;
    throw error;
  }
  return payload;
}

async function waitForDriver(timeoutMs = 20_000) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    if (appExited) throw new Error(`RemoteOpsX exited before WebDriver became ready (code ${appExitCode}).`);
    try {
      const status = await request("GET", "/status");
      if (status?.value?.ready !== false) return;
    } catch (error) {
      lastError = error;
    }
    await sleep(150);
  }
  throw new Error(`Timed out waiting for embedded WebDriver at ${webdriverBase}: ${lastError ?? "no response"}`);
}

async function createSession() {
  const response = await request("POST", "/session", {
    capabilities: { alwaysMatch: {} },
  });
  sessionId = response.sessionId || response?.value?.sessionId;
  if (!sessionId) throw new Error(`Embedded WebDriver did not return a session id: ${JSON.stringify(response)}`);
}

function sessionPath(suffix = "") {
  if (!sessionId) throw new Error("WebDriver session has not been created.");
  return `/session/${encodeURIComponent(sessionId)}${suffix}`;
}

function elementId(payload) {
  const id = payload?.value?.[ELEMENT_KEY] || payload?.value?.ELEMENT;
  if (!id) throw new Error(`WebDriver response did not contain an element id: ${JSON.stringify(payload)}`);
  return id;
}

async function find(selector, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      return elementId(await request("POST", sessionPath("/element"), {
        using: "css selector",
        value: selector,
      }));
    } catch (error) {
      lastError = error;
      if (error.protocolError && error.protocolError !== "no such element") throw error;
    }
    await sleep(100);
  }
  throw new Error(`Timed out finding ${selector}: ${lastError ?? "not found"}`);
}

async function findAll(selector) {
  const payload = await request("POST", sessionPath("/elements"), {
    using: "css selector",
    value: selector,
  });
  return (payload.value || []).map((entry) => entry[ELEMENT_KEY] || entry.ELEMENT).filter(Boolean);
}

async function text(id) {
  return (await request("GET", sessionPath(`/element/${encodeURIComponent(id)}/text`))).value ?? "";
}

async function attribute(id, name) {
  return (await request("GET", sessionPath(`/element/${encodeURIComponent(id)}/attribute/${encodeURIComponent(name)}`))).value;
}

async function click(id) {
  await request("POST", sessionPath(`/element/${encodeURIComponent(id)}/click`), {});
}

async function clear(id) {
  await request("POST", sessionPath(`/element/${encodeURIComponent(id)}/clear`), {});
}

async function type(id, value) {
  await request("POST", sessionPath(`/element/${encodeURIComponent(id)}/value`), {
    text: value,
    value: [...value],
  });
}

async function setValue(id, value) {
  await clear(id);
  await type(id, value);
}

async function findByText(selector, expected, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    for (const id of await findAll(selector)) {
      if ((await text(id)).includes(expected)) return id;
    }
    await sleep(100);
  }
  throw new Error(`Timed out finding ${selector} containing ${JSON.stringify(expected)}.`);
}

async function waitGone(selector, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const elements = await findAll(selector);
    if (elements.length === 0) return;
    await sleep(100);
  }
  throw new Error(`Timed out waiting for ${selector} to disappear.`);
}

async function step(name, fn) {
  process.stdout.write(`E2E ${name} ... `);
  await fn();
  console.log("ok");
}

async function main() {
  console.log(`Launching packaged RemoteOpsX E2E binary: ${binary}`);
  appProcess = spawn(binary, [], {
    cwd: root,
    env: {
      ...process.env,
      TAURI_WEBDRIVER_PORT: new URL(webdriverBase).port,
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  appProcess.stdout.on("data", (chunk) => process.stdout.write(`[app stdout] ${chunk}`));
  appProcess.stderr.on("data", (chunk) => process.stderr.write(`[app stderr] ${chunk}`));
  appProcess.once("exit", (code) => {
    appExited = true;
    appExitCode = code;
  });

  await waitForDriver();
  await createSession();

  await step("boots real Tauri operations workspace", async () => {
    const brand = await find(".brand");
    assert.match(await text(brand), /RemoteOpsX/);
    const dashboard = await find(".operations-dashboard");
    assert.ok(dashboard);
    const heading = await find(".operations-dashboard h1");
    assert.ok((await text(heading)).trim().length > 0);
  });

  await step("universal palette opens Runbook Studio", async () => {
    await click(await find(".command-trigger"));
    const palette = await find("[aria-label='Command palette']");
    assert.ok(palette);
    const input = await find("[aria-label='Command palette'] input");
    await setValue(input, "Runbook Studio");
    await click(await findByText(".palette-item", "Open Runbook Studio"));
    assert.ok(await find("[aria-label='Runbook Studio']"));
  });

  await step("Runbook dry-run crosses real Rust IPC without SSH", async () => {
    await click(await findByText("[aria-label='Runbook Studio'] button", "Validate / Dry run"));
    const valid = await find("[aria-label='Runbook Studio'] .warn-banner.ok");
    assert.match(await text(valid), /Valid/);
    const steps = await findAll("[aria-label='Runbook Studio'] .studio-step");
    assert.ok(steps.length >= 1, "expected backend preview steps");
    await click(await findByText("[aria-label='Runbook Studio'] button", "Close"));
    await waitGone("[aria-label='Runbook Studio']");
  });

  await step("settings persist through real Tauri IPC and SQLite", async () => {
    await click(await findByText(".topbar button", "Settings"));
    await find(".settings-modal");
    const current = await attribute(await find("#settings-theme option:checked"), "value");
    const target = current === "nord" ? "dracula" : "nord";
    await click(await find(`#settings-theme option[value='${target}']`));
    await click(await findByText(".settings-modal button", "Save settings"));
    await waitGone(".settings-modal");

    await click(await findByText(".topbar button", "Settings"));
    await find(".settings-modal");
    const restored = await attribute(await find("#settings-theme option:checked"), "value");
    assert.equal(restored, target);
    await click(await find("[aria-label='Close settings']"));
  });
}

async function cleanup() {
  if (sessionId) {
    try {
      await request("DELETE", sessionPath(), undefined, { allowProtocolError: true });
    } catch (error) {
      console.warn(`Could not close WebDriver session cleanly: ${error}`);
    }
    sessionId = undefined;
  }
  if (appProcess && !appExited) {
    appProcess.kill("SIGTERM");
    const deadline = Date.now() + 3_000;
    while (!appExited && Date.now() < deadline) await sleep(50);
    if (!appExited) appProcess.kill("SIGKILL");
  }
}

try {
  await main();
  console.log("Packaged desktop E2E passed.");
} catch (error) {
  console.error("Packaged desktop E2E failed:", error);
  process.exitCode = 1;
} finally {
  await cleanup();
}
