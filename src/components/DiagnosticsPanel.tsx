import { useState } from "react";
import * as api from "../api";
import { classifyConnectionFailure, type ConnectionFailure } from "../connectionDiagnostics";
import { useStore } from "../store";
import type { Server } from "../types";

interface DiagnosticResult {
  status: "success" | "failure";
  latencyMs: number;
  failure?: ConnectionFailure;
}

const PROBE_TOKEN = "__REMOTEOPSX_CONNECTION_OK__";

/**
 * Read-only connection diagnostics using the exact SSH execution path used by
 * health, services and runbooks. This intentionally does not use a separate
 * socket probe: the result must exercise the saved auth/keyring/known_hosts
 * configuration the operator will actually rely on.
 */
export function DiagnosticsPanel({ server }: { server: Server }) {
  const pushAlert = useStore((s) => s.pushAlert);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<DiagnosticResult | null>(null);

  async function runConnectionTest() {
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

  return (
    <div>
      <div className="health-head">
        <div>
          <div className="host">Connection diagnostics</div>
          <div className="os">Read-only SSH probe through the saved RemoteOpsX profile</div>
        </div>
        <button className="tiny" disabled={running} onClick={() => void runConnectionTest()}>
          {running ? "Testing…" : "Run test"}
        </button>
      </div>

      <div className="metric-card" style={{ marginBottom: 10 }}>
        <div className="mc-label">Target</div>
        <div className="mono" style={{ marginTop: 5 }}>{server.username}@{server.host}:{server.port}</div>
        <div className="mc-sub" style={{ marginTop: 5 }}>
          Auth: {server.auth_type === "key" ? "SSH key" : "stored password"} · Environment: {server.environment}
        </div>
      </div>

      <div className="panel-hint" style={{ marginBottom: 10 }}>
        The probe only runs <span className="mono">printf</span> remotely. It does not change server state.
        It exercises the same OpenSSH, credential and host-key path used by live operations.
      </div>

      {!result && !running && (
        <div className="warn-banner">Run the test before operating on an unfamiliar or production host.</div>
      )}

      {running && <div className="panel-hint">Checking SSH reachability and authentication…</div>}

      {result?.status === "success" && (
        <>
          <div className="warn-banner ok">✓ SSH reachable and authenticated · {result.latencyMs} ms</div>
          <div className="section-title">Verified by this test</div>
          <table className="data">
            <tbody>
              <tr><td>SSH transport</td><td>✓ connected</td></tr>
              <tr><td>Authentication</td><td>✓ accepted</td></tr>
              <tr><td>Remote command execution</td><td>✓ working</td></tr>
              <tr><td>Host-key policy</td><td>✓ passed current OpenSSH policy</td></tr>
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
