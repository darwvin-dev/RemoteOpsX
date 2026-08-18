//! Secret vault.
//!
//! Secrets (passwords / key passphrases) are stored in the OS Secret Service
//! via the `keyring` crate — NOT in SQLite. SQLite only ever holds a
//! `secret_ref` (the keyring entry name).

use anyhow::{Context, Result};
use keyring::Entry;

use crate::redaction;

const SERVICE: &str = "dev.remoteopsx.app";
const PROBE_ACCOUNT: &str = "__remoteopsx_keyring_probe__";

/// Build the keyring reference (account name) for a given server id.
pub fn secret_ref(server_id: &str) -> String {
    format!("server::{server_id}")
}

fn entry(secret_ref: &str) -> Result<Entry> {
    Entry::new(SERVICE, secret_ref).context("failed to open keyring entry")
}

/// Store a secret for the given reference. Overwrites any existing value.
pub fn set_secret(secret_ref: &str, secret: &str) -> Result<()> {
    let e = entry(secret_ref)?;
    e.set_password(secret)
        .context("failed to write secret to keyring")?;
    redaction::register_secret(secret);
    Ok(())
}

/// Fetch a secret. Returns `Ok(None)` when no entry exists.
pub fn get_secret(secret_ref: &str) -> Result<Option<String>> {
    let e = entry(secret_ref)?;
    match e.get_password() {
        Ok(password) => {
            redaction::register_secret(&password);
            Ok(Some(password))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err).context("failed to read secret from keyring"),
    }
}

/// Remove a secret if present. Missing entries are treated as success.
pub fn delete_secret(secret_ref: &str) -> Result<()> {
    let e = entry(secret_ref)?;
    let previous = match e.get_password() {
        Ok(password) => Some(password),
        Err(keyring::Error::NoEntry) => None,
        Err(_) => None,
    };
    match e.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {
            if let Some(secret) = previous {
                redaction::forget_secret(&secret);
            }
            Ok(())
        }
        Err(err) => Err(err).context("failed to delete secret from keyring"),
    }
}

/// Side-effect-free readiness probe used by runtime preflight. Reading a
/// deliberately absent account proves that the platform backend can be opened
/// and queried; `NoEntry` is the expected healthy result.
pub fn probe() -> Result<()> {
    let e = entry(PROBE_ACCOUNT)?;
    match e.get_password() {
        Ok(secret) => {
            // Do not retain a probe credential if one happens to exist.
            redaction::register_secret(&secret);
            Ok(())
        }
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(err).context("failed to query OS keyring backend"),
    }
}
