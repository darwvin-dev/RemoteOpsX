use std::collections::BTreeMap;
use std::fmt::Display;

use serde::Serialize;

use crate::redaction;

/// Stable, safe error payload returned by backend commands.
#[derive(Debug, Clone, Serialize)]
pub struct DomainError {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
    pub correlation_id: String,
    pub context: BTreeMap<String, String>,
}

pub type CommandResult<T> = Result<T, DomainError>;

impl DomainError {
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        let mut context = BTreeMap::new();
        context.insert("field".to_string(), field.into());
        Self {
            code: "validation.invalid_value",
            message: redaction::redact(message.into()),
            retryable: false,
            correlation_id: uuid::Uuid::new_v4().to_string(),
            context,
        }
    }

    /// A remote operation failed in an expected, recoverable way. Diagnostics
    /// remain visible to the operator, but every known in-process secret is
    /// removed before the payload crosses IPC.
    pub fn remote(message: impl Into<String>) -> Self {
        Self {
            code: "remote.operation_failed",
            message: redaction::redact(message.into()),
            retryable: true,
            correlation_id: uuid::Uuid::new_v4().to_string(),
            context: BTreeMap::new(),
        }
    }

    pub fn internal(error: impl Display) -> Self {
        let correlation_id = uuid::Uuid::new_v4().to_string();
        eprintln!(
            "internal backend error [{correlation_id}]: {}",
            redaction::redact(error.to_string())
        );
        Self {
            code: "internal.unexpected",
            message: "An unexpected internal error occurred.".to_string(),
            retryable: false,
            correlation_id,
            context: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DomainError;
    use crate::redaction;

    #[test]
    fn validation_error_serializes_stable_contract_and_field_context() {
        let serialized = serde_json::to_value(DomainError::validation(
            "server.password",
            "a password is required for password authentication",
        ))
        .expect("validation error should serialize");

        assert_eq!(serialized["code"], "validation.invalid_value");
        assert_eq!(serialized["retryable"], false);
        assert!(serialized["correlation_id"]
            .as_str()
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok()));
        assert_eq!(serialized["context"]["field"], "server.password");
    }

    #[test]
    fn remote_error_preserves_diagnostic_but_masks_known_secret() {
        const SECRET: &str = "error-test-remote-secret-canary";
        redaction::register_secret(SECRET);
        let error = DomainError::remote(format!(
            "Permission denied for {SECRET} at 10.0.0.1 port 22"
        ));

        assert_eq!(error.code, "remote.operation_failed");
        assert!(error.retryable);
        assert!(error.message.contains("Permission denied"));
        assert!(!error.message.contains(SECRET));
        assert!(error.message.contains("••••••"));
        assert!(uuid::Uuid::parse_str(&error.correlation_id).is_ok());
    }

    #[test]
    fn internal_error_serialization_never_exposes_diagnostics() {
        const SECRET: &str = "error-test-internal-secret-canary";
        redaction::register_secret(SECRET);
        let error = DomainError::internal(SECRET);
        assert_eq!(error.message, "An unexpected internal error occurred.");
        assert!(!error.retryable);
        assert!(error.context.is_empty());
        assert!(uuid::Uuid::parse_str(&error.correlation_id).is_ok());

        let serialized = serde_json::to_string(&error).expect("internal error should serialize");
        assert!(!serialized.contains(SECRET));
        assert!(serialized.contains("internal.unexpected"));
    }
}
