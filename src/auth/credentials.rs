use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{Aead, KeyInit};
use directories::ProjectDirs;
use keyring::Entry;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const CREDENTIALS_FILE: &str = "github_credentials.json";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct StoredCredentials {
    pub github_token: Option<String>,
    pub github_user: Option<super::github_oauth::GitHubUser>,
    pub git_name: Option<String>,
    pub git_email: Option<String>,
    pub git_ssh_passphrase: Option<String>,
    pub setup_completed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EncryptedPayload {
    encrypted_data: String,
    iv: String,
    key_source: String,
}

fn credentials_path() -> Option<PathBuf> {
    ProjectDirs::from("io", "parazeeknova", "Palimpsest")
        .map(|project_dirs| project_dirs.data_dir().join(CREDENTIALS_FILE))
}

fn get_or_create_keyring_key() -> Result<[u8; 32], keyring::Error> {
    let entry = Entry::new("io.parazeeknova.Palimpsest", "credentials_key")?;
    match entry.get_password() {
        Ok(hex_str) => {
            if let Some(bytes) = hex_decode(&hex_str) {
                if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    return Ok(key);
                }
            }
            let mut key = [0u8; 32];
            rand::thread_rng().fill(&mut key);
            let hex_key = hex_encode(&key);
            entry.set_password(&hex_key)?;
            Ok(key)
        }
        Err(keyring::Error::NoEntry) => {
            let mut key = [0u8; 32];
            rand::thread_rng().fill(&mut key);
            let hex_key = hex_encode(&key);
            entry.set_password(&hex_key)?;
            Ok(key)
        }
        Err(e) => Err(e),
    }
}

fn derive_machine_key() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"palimpsest_salt_constant");

    if let Ok(val) = std::env::var("USER") {
        hasher.update(val.as_bytes());
    }
    if let Ok(val) = std::env::var("USERNAME") {
        hasher.update(val.as_bytes());
    }
    if let Ok(val) = std::env::var("HOME") {
        hasher.update(val.as_bytes());
    }
    if let Ok(val) = std::env::var("USERPROFILE") {
        hasher.update(val.as_bytes());
    }

    if let Ok(contents) = std::fs::read_to_string("/etc/machine-id") {
        hasher.update(contents.trim().as_bytes());
    } else if let Ok(contents) = std::fs::read_to_string("/var/lib/dbus/machine-id") {
        hasher.update(contents.trim().as_bytes());
    }

    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

fn encrypt_data(key: &[u8; 32], plaintext: &[u8]) -> Result<(Vec<u8>, [u8; 12]), String> {
    let mut iv = [0u8; 12];
    rand::thread_rng().fill(&mut iv);

    let cipher = Aes256Gcm::new(key.into());
    let ct = cipher
        .encrypt((&iv).into(), plaintext)
        .map_err(|e| format!("Encryption failure: {:?}", e))?;

    Ok((ct, iv))
}

fn decrypt_data(key: &[u8; 32], iv: &[u8; 12], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new(key.into());
    let pt = cipher
        .decrypt(iv.into(), ciphertext)
        .map_err(|e| format!("Decryption failure: {:?}", e))?;

    Ok(pt)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(s.len() / 2);
    for i in 0..(s.len() / 2) {
        let byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
        bytes.push(byte);
    }
    Some(bytes)
}

fn load_token_from_keyring() -> Result<String, keyring::Error> {
    let entry = Entry::new("io.parazeeknova.Palimpsest", "github_token")?;
    entry.get_password()
}

fn delete_token_from_keyring() -> Result<(), keyring::Error> {
    let entry = Entry::new("io.parazeeknova.Palimpsest", "github_token")?;
    match entry.delete_password() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e),
    }
}

fn legacy_deobfuscate(hex_str: &str) -> Option<String> {
    let key = b"palimpsest_secret_key_123";
    let bytes = hex_decode(hex_str)?;
    let deobfuscated: Vec<u8> = bytes
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ key[i % key.len()])
        .collect();
    String::from_utf8(deobfuscated).ok()
}

pub fn load_credentials() -> StoredCredentials {
    let Some(path) = credentials_path() else {
        tracing::warn!("Unable to resolve credentials directory");
        return StoredCredentials::default();
    };

    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return StoredCredentials::default();
        }
        Err(error) => {
            tracing::warn!(path = %path.display(), error = %error, "Failed to read credentials file");
            return StoredCredentials::default();
        }
    };

    if let Ok(payload) = serde_json::from_str::<EncryptedPayload>(&contents) {
        let key_res = match payload.key_source.as_str() {
            "keyring" => get_or_create_keyring_key().map_err(|e| e.to_string()),
            _ => Ok(derive_machine_key()),
        };

        let key = match key_res {
            Ok(k) => k,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load encryption key from keyring, trying derived fallback");
                derive_machine_key()
            }
        };

        let iv_bytes = match hex_decode(&payload.iv) {
            Some(iv) if iv.len() == 12 => {
                let mut a = [0u8; 12];
                a.copy_from_slice(&iv);
                a
            }
            _ => {
                tracing::warn!("Invalid IV length in credentials file");
                return StoredCredentials::default();
            }
        };

        let ct_bytes = match hex_decode(&payload.encrypted_data) {
            Some(ct) => ct,
            _ => {
                tracing::warn!("Invalid hex in encrypted credentials data");
                return StoredCredentials::default();
            }
        };

        match decrypt_data(&key, &iv_bytes, &ct_bytes) {
            Ok(pt) => match serde_json::from_slice::<StoredCredentials>(&pt) {
                Ok(mut credentials) => {
                    if let Some(ref tok) = credentials.github_token {
                        if tok == "keyring" {
                            match load_token_from_keyring() {
                                Ok(t) => credentials.github_token = Some(t),
                                Err(error) => {
                                    tracing::warn!(error = %error, "Failed to load token from legacy keyring entry");
                                    credentials.github_token = None;
                                }
                            }
                        }
                    }
                    credentials
                }
                Err(error) => {
                    tracing::warn!(error = %error, "Failed to deserialize decrypted credentials");
                    StoredCredentials::default()
                }
            },
            Err(error) => {
                tracing::warn!(error = %error, "Failed to decrypt credentials");
                StoredCredentials::default()
            }
        }
    } else {
        match serde_json::from_str::<StoredCredentials>(&contents) {
            Ok(mut credentials) => {
                if let Some(ref tok) = credentials.github_token {
                    if tok == "keyring" {
                        match load_token_from_keyring() {
                            Ok(t) => credentials.github_token = Some(t),
                            Err(error) => {
                                tracing::warn!(error = %error, "Failed to load token from legacy keyring entry");
                                credentials.github_token = None;
                            }
                        }
                    } else if tok.starts_with("obfuscated:") {
                        let hex_str = tok.trim_start_matches("obfuscated:");
                        credentials.github_token = legacy_deobfuscate(hex_str);
                    }
                }
                if let Err(e) = save_credentials(&credentials) {
                    tracing::warn!(error = %e, "Failed to migrate credentials to encrypted format");
                } else {
                    tracing::info!("Successfully migrated legacy credentials to encrypted format");
                }
                credentials
            }
            Err(error) => {
                tracing::warn!(path = %path.display(), error = %error, "Failed to parse legacy credentials file");
                StoredCredentials::default()
            }
        }
    }
}

pub fn save_credentials(credentials: &StoredCredentials) -> Result<(), String> {
    let path =
        credentials_path().ok_or_else(|| "Unable to resolve credentials directory".to_string())?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create credentials directory: {error}"))?;
    }

    let serialized = serde_json::to_vec(credentials)
        .map_err(|error| format!("Failed to serialize credentials: {error}"))?;

    let (key, key_source) = match get_or_create_keyring_key() {
        Ok(k) => (k, "keyring".to_string()),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get/create keyring key, falling back to derived key");
            (derive_machine_key(), "derived".to_string())
        }
    };

    let (ct, iv) = encrypt_data(&key, &serialized)?;

    let payload = EncryptedPayload {
        encrypted_data: hex_encode(&ct),
        iv: hex_encode(&iv),
        key_source,
    };

    let serialized_payload = serde_json::to_string_pretty(&payload)
        .map_err(|error| format!("Failed to serialize encrypted payload: {error}"))?;

    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, &serialized_payload)
        .map_err(|error| format!("Failed to write credentials temp file: {error}"))?;

    std::fs::rename(&temp_path, &path).map_err(|error| {
        if let Err(cleanup_error) = std::fs::remove_file(&temp_path) {
            tracing::warn!(
                path = %temp_path.display(),
                error = %cleanup_error,
                "Failed to clean up temp credentials file after rename failure"
            );
        }
        format!("Failed to commit credentials file: {error}")
    })
}

pub fn clear_github_auth(credentials: &mut StoredCredentials, persist: bool) {
    credentials.github_token = None;
    credentials.github_user = None;
    if persist {
        let _ = delete_token_from_keyring();
        if let Err(error) = save_credentials(credentials) {
            tracing::warn!(error = %error, "Failed to save credentials after clearing auth");
        }
    }
}

pub fn save_git_ssh_passphrase(passphrase: Option<String>) -> Result<(), String> {
    let mut credentials = load_credentials();
    credentials.git_ssh_passphrase = passphrase;
    save_credentials(&credentials)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_credentials_have_no_auth() {
        let credentials = StoredCredentials::default();
        assert!(credentials.github_token.is_none());
        assert!(credentials.github_user.is_none());
        assert!(credentials.git_name.is_none());
        assert!(credentials.git_email.is_none());
        assert!(!credentials.setup_completed);
    }

    #[test]
    fn encryption_roundtrip() {
        let key = [1u8; 32];
        let plaintext = b"Hello, World! Secure local credentials!";
        let (ct, iv) = encrypt_data(&key, plaintext).unwrap();
        assert_ne!(plaintext.as_slice(), ct.as_slice());
        let pt = decrypt_data(&key, &iv, &ct).unwrap();
        assert_eq!(plaintext.as_slice(), pt.as_slice());
    }

    #[test]
    fn credentials_serialization_roundtrip() {
        let credentials = StoredCredentials {
            github_token: Some("gho_test_token".into()),
            github_user: None,
            git_name: Some("Test User".into()),
            git_email: Some("test@example.com".into()),
            git_ssh_passphrase: None,
            setup_completed: true,
        };
        let serialized = serde_json::to_string(&credentials).expect("serialization should succeed");
        let deserialized: StoredCredentials =
            serde_json::from_str(&serialized).expect("deserialization should succeed");
        assert_eq!(credentials, deserialized);
    }

    #[test]
    fn clear_github_auth_removes_token_and_user() {
        let mut credentials = StoredCredentials {
            github_token: Some("token".into()),
            github_user: Some(super::super::github_oauth::GitHubUser {
                login: "testuser".into(),
                name: Some("Test".into()),
                email: Some("test@example.com".into()),
                avatar_url: "https://example.com/avatar.png".into(),
                html_url: "https://github.com/testuser".into(),
                bio: None,
            }),
            git_name: Some("Test".into()),
            git_email: Some("test@example.com".into()),
            git_ssh_passphrase: None,
            setup_completed: true,
        };
        clear_github_auth(&mut credentials, false);
        assert!(credentials.github_token.is_none());
        assert!(credentials.github_user.is_none());
        assert!(credentials.git_name.is_some());
        assert!(credentials.setup_completed);
    }

    #[test]
    fn legacy_migration_and_decryption() {
        let token = "gho_test_token";
        let key = b"palimpsest_secret_key_123";
        let obfuscated: Vec<u8> = token
            .bytes()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()])
            .collect();
        let hex_str = hex_encode(&obfuscated);

        let legacy_json = format!(
            r#"{{
            "github_token": "obfuscated:{}",
            "github_user": null,
            "git_name": "Legacy User",
            "git_email": "legacy@example.com",
            "setup_completed": true
        }}"#,
            hex_str
        );

        let mut credentials = serde_json::from_str::<StoredCredentials>(&legacy_json).unwrap();
        if let Some(ref tok) = credentials.github_token {
            if tok.starts_with("obfuscated:") {
                let hex_slice = tok.trim_start_matches("obfuscated:");
                credentials.github_token = legacy_deobfuscate(hex_slice);
            }
        }

        assert_eq!(credentials.git_name.as_deref(), Some("Legacy User"));
        assert_eq!(credentials.git_email.as_deref(), Some("legacy@example.com"));
        assert!(credentials.setup_completed);
        assert_eq!(credentials.github_token.as_deref(), Some("gho_test_token"));
    }
}
