//! Central redaction for secrets that have entered the RemoteOpsX process.
//!
//! Vault reads/writes register values here. Any output, persisted runbook
//! result, diagnostic export, or error path can then use one deterministic
//! redaction function instead of implementing ad-hoc masking.

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
    let mut secrets = KNOWN_SECRETS.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    if !secrets.iter().any(|known| known == secret) {
        secrets.push(secret.to_string());
        // Longest first prevents a short credential from partially masking a
        // longer one and leaving a recognizable suffix.
        secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
    }
}

pub fn forget_secret(secret: &str) {
    let mut secrets = KNOWN_SECRETS.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    secrets.retain(|known| known != secret);
}

pub fn redact(text: impl AsRef<str>) -> String {
    let mut value = text.as_ref().to_string();
    let secrets = KNOWN_SECRETS.read().unwrap_or_else(|poisoned| poisoned.into_inner());
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

pub fn reset_for_tests() {
    KNOWN_SECRETS
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_registered_secrets_from_arbitrary_text() {
        reset_for_tests();
        register_secret("super-secret-token");
        let value = redact("stdout=super-secret-token stderr=super-secret-token");
        assert_eq!(value, "stdout=•••••• stderr=••••••");
    }

    #[test]
    fn ignores_tiny_values_to_avoid_destroying_normal_output() {
        reset_for_tests();
        register_secret("abc");
        assert_eq!(redact("abc is common text"), "abc is common text");
    }

    #[test]
    fn masks_command_output_and_runbook_command_fields() {
        reset_for_tests();
        register_secret("password123");
        let output = redact_command_output(CommandOutput {
            stdout: "password123".into(),
            stderr: "failed: password123".into(),
            exit_code: 1,
            success: false,
        });
        assert!(!output.stdout.contains("password123"));
        assert!(!output.stderr.contains("password123"));

        let step = redact_step_result(StepResult {
            name: "test".into(),
            command: "curl -u user:password123 host".into(),
            stdout: "ok".into(),
            stderr: String::new(),
            exit_code: 0,
            status: "success".into(),
        });
        assert!(!step.command.contains("password123"));
    }
}
