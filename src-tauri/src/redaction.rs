//! Central redaction for secrets that have entered the RemoteOpsX process.
//!
//! Vault reads/writes register values here. Output, persisted runbook results,
//! diagnostic exports, and error paths then use one deterministic masking
//! layer. Persistence entry points can also reject user-authored content that
//! contains a known credential instead of silently storing it in SQLite.

use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::models::{CommandOutput, StepResult};

const MASK: &str = "••••••";
const MIN_SECRET_LEN: usize = 4;

static KNOWN_SECRETS: Lazy<RwLock<Vec<String>>> = Lazy::new(|| RwLock::new(Vec::new()));

pub fn register_secret(secret: &str) {
    if secret.len() < MIN_SECRET_LEN {
        return;
    }
    let mut secrets = KNOWN_SECRETS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !secrets.iter().any(|known| known == secret) {
        secrets.push(secret.to_string());
        secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
    }
}

pub fn forget_secret(secret: &str) {
    let mut secrets = KNOWN_SECRETS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    secrets.retain(|known| known != secret);
}

pub fn contains_known_secret(text: impl AsRef<str>) -> bool {
    let value = text.as_ref();
    let secrets = KNOWN_SECRETS
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    secrets.iter().any(|secret| value.contains(secret))
}

pub fn redact(text: impl AsRef<str>) -> String {
    let mut value = text.as_ref().to_string();
    let secrets = KNOWN_SECRETS
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for secret in secrets.iter() {
        if value.contains(secret) {
            value = value.replace(secret, MASK);
        }
    }
    value
}

pub fn redact_command_output(mut output: CommandOutput) -> CommandOutput {
    output.stdout = redact(output.stdout);
    output.stderr = redact(output.stderr);
    output
}

pub fn redact_step_result(mut result: StepResult) -> StepResult {
    result.command = redact(result.command);
    result.stdout = redact(result.stdout);
    result.stderr = redact(result.stderr);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_registered_secrets_from_arbitrary_text() {
        register_secret("redaction-test-super-secret-token");
        let value = redact(
            "stdout=redaction-test-super-secret-token stderr=redaction-test-super-secret-token",
        );
        assert_eq!(value, "stdout=•••••• stderr=••••••");
    }

    #[test]
    fn detects_known_secret_before_persistence() {
        register_secret("redaction-test-database-canary-secret");
        assert!(contains_known_secret(
            "echo redaction-test-database-canary-secret"
        ));
        assert!(!contains_known_secret("echo safe"));
    }

    #[test]
    fn ignores_tiny_values_to_avoid_destroying_normal_output() {
        register_secret("abc");
        assert_eq!(redact("abc is common text"), "abc is common text");
        assert!(!contains_known_secret("abc is common text"));
    }

    #[test]
    fn masks_command_output_and_runbook_command_fields() {
        register_secret("redaction-test-password-123");
        let output = redact_command_output(CommandOutput {
            stdout: "redaction-test-password-123".into(),
            stderr: "failed: redaction-test-password-123".into(),
            exit_code: 1,
            success: false,
        });
        assert!(!output.stdout.contains("redaction-test-password-123"));
        assert!(!output.stderr.contains("redaction-test-password-123"));

        let step = redact_step_result(StepResult {
            name: "test".into(),
            command: "curl -u user:redaction-test-password-123 host".into(),
            stdout: "ok".into(),
            stderr: String::new(),
            exit_code: 0,
            status: "success".into(),
        });
        assert!(!step.command.contains("redaction-test-password-123"));
    }
}
