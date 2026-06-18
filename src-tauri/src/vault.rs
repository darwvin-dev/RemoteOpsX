//! Secret vault.
//!
//! Secrets (passwords / key passphrases) are stored in the OS Secret Service
//! via the `keyring` crate — NOT in SQLite. SQLite only ever holds a
//! `secret_ref` (the keyring entry name). This satisfies the MVP security
//! requirement: "Do not store plaintext passwords in SQLite."
//!
//! Each server gets a stable keyring entry keyed by its server id.

use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE: &str = "dev.remoteopsx.app";

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
    e.set_password(secret).context("failed to write secret to keyring")?;
    Ok(())
}

/// Fetch a secret. Returns `Ok(None)` when no entry exists.
pub fn get_secret(secret_ref: &str) -> Result<Option<String>> {
    let e = entry(secret_ref)?;
    match e.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err).context("failed to read secret from keyring"),
    }
}

/// Remove a secret if present. Missing entries are treated as success.
pub fn delete_secret(secret_ref: &str) -> Result<()> {
    let e = entry(secret_ref)?;
    match e.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(err).context("failed to delete secret from keyring"),
    }
}
