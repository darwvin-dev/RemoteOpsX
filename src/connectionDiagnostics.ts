export type ConnectionFailureKind =
  | "dns"
  | "network"
  | "host-key"
  | "auth"
  | "dependency"
  | "credential"
  | "unknown";

export interface ConnectionFailure {
  kind: ConnectionFailureKind;
  title: string;
  detail: string;
  action: string;
}

/**
 * Turn common OpenSSH / RemoteOpsX connection failures into operator-facing
 * guidance. Keep this pure so the mapping is regression-tested independently
 * from Tauri and the host network.
 */
export function classifyConnectionFailure(message: string): ConnectionFailure {
  const detail = message.trim() || "The connection test failed without a diagnostic message.";
  const text = detail.toLowerCase();

  if (
    text.includes("could not resolve hostname") ||
    text.includes("name or service not known") ||
    text.includes("nodename nor servname provided") ||
    text.includes("temporary failure in name resolution")
  ) {
    return {
      kind: "dns",
      title: "DNS resolution failed",
      detail,
      action: "Check the hostname, DNS resolver and whether the required VPN/private network is connected.",
    };
  }

  if (
    text.includes("remote host identification has changed") ||
    text.includes("host key verification failed") ||
    text.includes("offending") && text.includes("known_hosts")
  ) {
    return {
      kind: "host-key",
      title: "Host identity check failed",
      detail,
      action: "Do not bypass this blindly. Verify the server fingerprint with a trusted source, then repair the matching known_hosts entry if the change is legitimate.",
    };
  }

  if (
    text.includes("permission denied") ||
    text.includes("authentication failed") ||
    text.includes("too many authentication failures")
  ) {
    return {
      kind: "auth",
      title: "SSH authentication failed",
      detail,
      action: "Verify the username and selected auth method. For key auth, confirm the configured private key is authorized on the server; for password auth, re-save the credential.",
    };
  }

  if (
    text.includes("sshpass") && text.includes("not installed") ||
    text.includes("failed to spawn ssh") ||
    text.includes("no such file or directory") && text.includes("ssh")
  ) {
    return {
      kind: "dependency",
      title: "Required SSH dependency is unavailable",
      detail,
      action: "Install OpenSSH and, for password-auth profiles, sshpass. Then rerun the test.",
    };
  }

  if (text.includes("no stored password")) {
    return {
      kind: "credential",
      title: "Stored credential is missing",
      detail,
      action: "Edit this server profile and save the password again so RemoteOpsX can place it in the OS keyring.",
    };
  }

  if (
    text.includes("connection refused") ||
    text.includes("connection timed out") ||
    text.includes("operation timed out") ||
    text.includes("no route to host") ||
    text.includes("network is unreachable") ||
    text.includes("connection reset")
  ) {
    return {
      kind: "network",
      title: "SSH endpoint is unreachable",
      detail,
      action: "Check the host/port, firewall/security-group rules, VPN/routing and whether sshd is listening on the target server.",
    };
  }

  return {
    kind: "unknown",
    title: "Connection test failed",
    detail,
    action: "Review the diagnostic text below, then verify the profile, network path and remote SSH service before retrying.",
  };
}
