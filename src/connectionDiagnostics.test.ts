import { describe, expect, it } from "vitest";
import { classifyConnectionFailure } from "./connectionDiagnostics";

describe("classifyConnectionFailure", () => {
  it("classifies DNS failures", () => {
    const result = classifyConnectionFailure("ssh: Could not resolve hostname db.internal: Name or service not known");
    expect(result.kind).toBe("dns");
  });

  it("treats changed host keys as an identity failure, not a generic network error", () => {
    const result = classifyConnectionFailure("WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED! Host key verification failed.");
    expect(result.kind).toBe("host-key");
    expect(result.action).toContain("fingerprint");
  });

  it("classifies authentication failures", () => {
    const result = classifyConnectionFailure("root@example: Permission denied (publickey,password).");
    expect(result.kind).toBe("auth");
  });

  it("classifies missing sshpass as a dependency problem", () => {
    const result = classifyConnectionFailure("This server uses password auth but `sshpass` is not installed.");
    expect(result.kind).toBe("dependency");
  });

  it("classifies a missing keyring password separately from bad authentication", () => {
    const result = classifyConnectionFailure("No stored password for this server. Re-save the profile with a password.");
    expect(result.kind).toBe("credential");
  });

  it("classifies refused and timed-out connections as network failures", () => {
    expect(classifyConnectionFailure("ssh: connect to host 10.0.0.4 port 22: Connection refused").kind).toBe("network");
    expect(classifyConnectionFailure("ssh: connect to host 10.0.0.4 port 22: Connection timed out").kind).toBe("network");
  });

  it("preserves unknown diagnostics for the operator", () => {
    const result = classifyConnectionFailure("unexpected transport failure 77");
    expect(result.kind).toBe("unknown");
    expect(result.detail).toBe("unexpected transport failure 77");
  });
});
