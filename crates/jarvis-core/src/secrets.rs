//! API keys and the memory DB key, in the OS keychain (service `jarvis`).
//!
//! Provider ids used everywhere: `anthropic`, `openai`, `gemini`, `openrouter`,
//! `xai`, `groq`, `deepseek`, `mistral`, `elevenlabs`, `deepgram`, `voicestudio`.
//! Key values are never logged or included in errors.

const SERVICE: &str = "jarvis";
const MEMORY_KEY_ACCOUNT: &str = "memory-key";

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("keychain: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("random number generator: {0}")]
    Random(#[from] getrandom::Error),
    #[error("stored memory key is not 32 hex-encoded bytes")]
    BadMemoryKey,
}

fn entry(account: &str) -> Result<keyring::Entry, SecretError> {
    Ok(keyring::Entry::new(SERVICE, account)?)
}

/// `JARVIS_<PROVIDER>_API_KEY` (dev/CI override) first, then the keychain.
pub fn get_key(provider: &str) -> Option<String> {
    let var = format!("JARVIS_{}_API_KEY", provider.to_ascii_uppercase());
    if let Some(k) = std::env::var(var).ok().filter(|k| !k.is_empty()) {
        return Some(k);
    }
    entry(provider).ok()?.get_password().ok()
}

pub fn set_key(provider: &str, key: &str) -> Result<(), SecretError> {
    Ok(entry(provider)?.set_password(key)?)
}

/// Deleting a key that isn't stored is not an error.
pub fn delete_key(provider: &str) -> Result<(), SecretError> {
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// The SQLCipher key for the memory DB, created on first use.
pub fn memory_key() -> Result<[u8; 32], SecretError> {
    let e = entry(MEMORY_KEY_ACCOUNT)?;
    match e.get_password() {
        Ok(hex_key) => {
            let bytes = hex::decode(hex_key).map_err(|_| SecretError::BadMemoryKey)?;
            bytes.try_into().map_err(|_| SecretError::BadMemoryKey)
        }
        Err(keyring::Error::NoEntry) => {
            let mut key = [0u8; 32];
            getrandom::fill(&mut key)?;
            e.set_password(&hex::encode(key))?;
            Ok(key)
        }
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    /// Point keyring at an in-memory mock store. keyring 4's `Entry::new`
    /// installs the platform store on first use, so trigger that first,
    /// then replace it.
    fn mock_store() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = keyring::Entry::store_status();
            keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap());
        });
    }

    #[test]
    fn env_var_overrides_keychain() {
        mock_store();
        set_key("testenv", "from-keychain").unwrap();
        // Unique provider name per test, so no other test reads this var.
        unsafe { std::env::set_var("JARVIS_TESTENV_API_KEY", "from-env") };
        let got = get_key("testenv");
        unsafe { std::env::remove_var("JARVIS_TESTENV_API_KEY") };
        assert_eq!(got.as_deref(), Some("from-env"));
        assert_eq!(get_key("testenv").as_deref(), Some("from-keychain"));
    }

    #[test]
    fn set_get_delete_roundtrip() {
        mock_store();
        set_key("testroundtrip", "k1").unwrap();
        assert_eq!(get_key("testroundtrip").as_deref(), Some("k1"));
        set_key("testroundtrip", "k2").unwrap();
        assert_eq!(get_key("testroundtrip").as_deref(), Some("k2"));
        delete_key("testroundtrip").unwrap();
        assert_eq!(get_key("testroundtrip"), None);
    }

    #[test]
    fn memory_key_is_stable_across_calls() {
        mock_store();
        let a = memory_key().unwrap();
        let b = memory_key().unwrap();
        assert_eq!(a, b);
        assert_ne!(a, [0u8; 32]);
    }

    #[test]
    fn get_missing_key_is_none() {
        mock_store();
        assert_eq!(get_key("testmissing"), None);
    }
}
