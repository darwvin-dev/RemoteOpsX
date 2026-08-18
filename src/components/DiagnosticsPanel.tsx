import { useEffect, useState } from "react";
import * as api from "../api";
import { classifyConnectionFailure, type ConnectionFailure } from "../connectionDiagnostics";
import { useStore } from "../store";
import type { HostIdentityReport, RuntimePreflightReport, Server } from "../types";

interface DiagnosticResult {
  status: "success" | "failure";
  latencyMs: number;
  failure?: ConnectionFailure;
}

const PROBE_TOKEN = "__REMOTEOPSX_CONNECTION_OK__";

export function DiagnosticsPanel({ server }: { server: Server }) {
  const pushAlert = useStore((s) => s.pushAlert);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<DiagnosticResult | null>(null);
  const [preflight, setPreflight] = useState<RuntimePreflightReport | null>(null);
  const [preflightError, setPreflightError] = useState<string | null>(null);
  const [identity, setIdentity] = useState<HostIdentityReport | null>(null);
  const [identityBusy, setIdentityBusy] = useState(false);
  const [identityError, setIdentityError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setResult(null);
    setPreflight(null);
    setPreflightError(null);
    setIdentity(null);
    setIdentityError(null);

    void api
      .runtimePreflight()
      .then((report) => {
        if (!cancelled) setPreflight(report);
      })
      .catch((error) => {
        if (!cancelled) setPreflightError(String(error));
      });

    setIdentityBusy(true);
    void api
      .sshHostIdentityInspect(server.id)
      .then((report) => {
        if (!cancelled) setIdentity(report);
      })
      .catch((error) => {
        if (!cancelled) setIdentityError(String(error));
      })
      .finally(() => {
        if (!cancelled) setIdentityBusy(false);
      });

    return () => {
      cancelled = true;
    };
  }, [server.id]);

  async function refreshIdentity() {
    setIdentityBusy(true);
    setIdentityError(null);
    try {
      setIdentity(await api.sshHostIdentityInspect(server.id));
    } catch (error) {
      setIdentityError(String(error));
    } finally {
      setIdentityBusy(false);
    }
  }

  async function trustFingerprint(fingerprint: string, replace: boolean) {
    setIdentityBusy(true);
    setIdentityError(null);
    try {
      const report = await api.sshHostIdentityTrust(server.id, fingerprint, replace);
      setIdentity(report);
      pushAlert(
        "info",
        `${server.name}: SSH fingerprint ${replace ? "replaced" : "trusted"}`,
        server.id,
      );
    } catch (error) {
      setIdentityError(String(error));
      pushAlert("error", `${server.name}: SSH trust update failed`, server.id);
    } finally {
      setIdentityBusy(false);
    }
  }

  async function removeTrust() {
    setIdentityBusy(true);
    setIdentityError(null);
    try {
      await api.sshHostIdentityRemove(server.id);
      await refreshIdentity();
      pushAlert("info", `${server.name}: stored SSH identity removed`, server.id);
    } catch (error) {
      setIdentityError(String(error));
      pushAlert("error", `${server.name}: failed to remove SSH identity`, server.id);
      setIdentityBusy(false);
    }
  }

  async function runConnectionTest() {
    if (identity?.status !== "trusted") {
      setIdentityError("Trust a verified SSH fingerprint before running the connection test.");
      return;
    }

    setRunning(true);
    setResult(null);
    const started = performance.now();

    try {
      const output = await api.runRemote(server.id, `printf '${PROBE_TOKEN}'`);
      const latencyMs = Math.max(0, Math.round(performance.now() - started));

      if (output.success && output.stdout.includes(PROBE_TOKEN)) {
        setResult({ status: "success", latencyMs });
        return;
      }

      const failure = classifyConnectionFailure(
        output.stderr || output.stdout || `SSH exited with code ${output.exit_code}`,
      );
      setResult({ status: "failure", latencyMs, failure });
      pushAlert("error", `${server.name}: ${failure.title}`, server.id);
    } catch (error) {
      const latencyMs = Math.max(0, Math.round(performance.now() - started));
      const failure = classifyConnectionFailure(String(error));
      setResult({ status: "failure", latencyMs, failure });
      pushAlert("error", `${server.name}: ${failure.title}`, server.id);
    } finally {
      setRunning(false);
    }
  }

  const relevantOptionalIds = new Set<string>();
  if (server.auth_type === "password") {
    relevantOptionalIds.add("sshpass");
    relevantOptionalIds.add("keyring");
  }
  if (server.protocols.includes("ftp")) relevantOptionalIds.add("curl");
  if (server.protocols.includes("rdp")) relevantOptionalIds.add("freerdp");
  if (server.protocols.includes("vnc")) relevantOptionalIds.add("vnc");
  const profileReady =
    preflight?.dependencies.every(
      (dependency) =>
        dependency.available || (!dependency.required && !relevantOptionalIds.has(dependency.id)),
    ) ?? false;

  return (
    <div>
      <div className="health-head">
        <div>
          <div className="host">Connection diagnostics</div>
          <div className="os">Runtime readiness + explicit SSH identity trust</div>
        </div>
        <button className="tiny" disabled={identityBusy} onClick={() => void refreshIdentity()}>
          {identityBusy ? "Scanning…" : "Rescan key"}
        </button>
      </div>

      <div className="metric-card" style={{ marginBottom: 10 }}>
        <div className="mc-label">Target</div>
        <div className="mono" style={{ marginTop: 5 }}>
          {server.username}@{server.host}:{server.port}
        </div>
        <div className="mc-sub" style={{ marginTop: 5 }}>
          Auth: {server.auth_type === "key" ? "SSH key" : "stored password"} · Environment:{" "}
          {server.environment}
        </div>
      </div>

      <div className="section-title">Runtime preflight</div>
      {preflightError && <div className="warn-banner">⚠ {preflightError}</div>}
      {!preflight && !preflightError && <div className="panel-hint">Checking local dependencies…</div>}
      {preflight && (
        <>
          <div className={`warn-banner${profileReady ? " ok" : ""}`}>
            {profileReady
              ? "✓ Required tools for this profile are available"
              : "⚠ One or more tools required by this profile are unavailable"}
          </div>
          <table className="data">
            <tbody>
              {preflight.dependencies.map((dependency) => {
                const relevant = dependency.required || relevantOptionalIds.has(dependency.id);
                return (
                  <tr key={dependency.id}>
                    <td>{dependency.label}</td>
                    <td title={dependency.detail}>
                      {dependency.available ? "✓ available" : relevant ? "✗ missing" : "— optional"}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </>
      )}

      <div className="section-title">SSH host identity</div>
      <div className="panel-hint" style={{ marginBottom: 10 }}>
        Compare the SHA-256 fingerprint below with a trusted out-of-band source before selecting Trust
        or Replace. RemoteOpsX never silently accepts a first-seen SSH key.
      </div>

      {identityError && <div className="warn-banner">⚠ {identityError}</div>}
      {identity?.status === "trusted" && (
        <div className="warn-banner ok">✓ SSH identity is explicitly trusted</div>
      )}
      {identity?.status === "unseen" && (
        <div className="warn-banner">SSH identity has not been trusted yet.</div>
      )}
      {identity?.status === "changed" && (
        <div className="warn-banner">
          ⚠ SSH identity changed. Do not replace it until the new fingerprint is independently verified.
        </div>
      )}

      {identity && (
        <table className="data">
          <tbody>
            {identity.candidates.map((candidate) => (
              <tr key={`${candidate.key_type}-${candidate.fingerprint}`}>
                <td>
                  <div>{candidate.key_type}</div>
                  <div className="mono" style={{ fontSize: 10 }}>
                    {candidate.fingerprint}
                  </div>
                </td>
                <td>
                  {identity.status === "unseen" ? (
                    <button
                      className="tiny"
                      disabled={identityBusy}
                      onClick={() => void trustFingerprint(candidate.fingerprint, false)}
                    >
                      Trust
                    </button>
                  ) : identity.status === "changed" ? (
                    <button
                      className="tiny"
                      disabled={identityBusy}
                      onClick={() => void trustFingerprint(candidate.fingerprint, true)}
                    >
                      Replace
                    </button>
                  ) : (
                    "verified"
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {identity?.trusted_fingerprints.length ? (
        <div className="metric-card" style={{ marginTop: 8 }}>
          <div className="mc-label">Stored fingerprint(s)</div>
          {identity.trusted_fingerprints.map((fingerprint) => (
            <div key={fingerprint} className="mono" style={{ marginTop: 4, fontSize: 10 }}>
              {fingerprint}
            </div>
          ))}
          <button className="tiny" disabled={identityBusy} onClick={() => void removeTrust()} style={{ marginTop: 8 }}>
            Remove trust
          </button>
        </div>
      ) : null}

      <div className="section-title">Authenticated SSH probe</div>
      <div className="panel-hint" style={{ marginBottom: 10 }}>
        The probe only sends <span className="mono">printf</span> to the remote shell. It does not change
        server state and is disabled until the host identity is trusted.
      </div>
      <button
        className="tiny"
        disabled={running || identity?.status !== "trusted" || !profileReady}
        onClick={() => void runConnectionTest()}
      >
        {running ? "Testing…" : "Run SSH test"}
      </button>

      {!result && !running && identity?.status === "trusted" && (
        <div className="panel-hint" style={{ marginTop: 10 }}>
          Run the test before operating on an unfamiliar or production host.
        </div>
      )}
      {running && <div className="panel-hint">Checking SSH reachability and authentication…</div>}

      {result?.status === "success" && (
        <>
          <div className="warn-banner ok">✓ SSH reachable and authenticated · {result.latencyMs} ms</div>
          <table className="data">
            <tbody>
              <tr><td>SSH transport</td><td>✓ connected</td></tr>
              <tr><td>Authentication</td><td>✓ accepted</td></tr>
              <tr><td>Remote command execution</td><td>✓ working</td></tr>
              <tr><td>Host identity</td><td>✓ strict trusted key</td></tr>
            </tbody>
          </table>
        </>
      )}

      {result?.status === "failure" && result.failure && (
        <>
          <div className="warn-banner">⚠ {result.failure.title} · {result.latencyMs} ms</div>
          <div className="section-title">Suggested action</div>
          <div className="metric-card">
            <div>{result.failure.action}</div>
          </div>
          <div className="section-title">Diagnostic</div>
          <div
            className="mono"
            style={{
              fontSize: 10,
              color: "var(--text-1)",
              whiteSpace: "pre-wrap",
              overflowWrap: "anywhere",
            }}
          >
            {result.failure.detail}
          </div>
        </>
      )}
    </div>
  );
}
