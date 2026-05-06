use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use pbkdf2::pbkdf2_hmac_array;
use rand::Rng;
use sha2::Sha256;

const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_LENGTH: usize = 16;
const NONCE_LENGTH: usize = 12;

/// Derive a 256-bit key from password using PBKDF2
fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    pbkdf2_hmac_array::<Sha256, 32>(password.as_bytes(), salt, PBKDF2_ITERATIONS)
}

/// Encrypt data using AES-256-GCM
/// Returns: base64(salt + nonce + ciphertext)
pub fn encrypt(plaintext: &str, password: &str) -> Result<String, String> {
    let mut rng = rand::thread_rng();

    // Generate random salt and nonce
    let salt: [u8; SALT_LENGTH] = rng.gen();
    let nonce_bytes: [u8; NONCE_LENGTH] = rng.gen();

    // Derive key from password
    let key = derive_key(password, &salt);

    // Create cipher and encrypt
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Failed to create cipher: {}", e))?;

    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

    // Combine: salt + nonce + ciphertext
    let mut combined = Vec::with_capacity(SALT_LENGTH + NONCE_LENGTH + ciphertext.len());
    combined.extend_from_slice(&salt);
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(combined))
}

/// Decrypt data using AES-256-GCM
pub fn decrypt(encrypted_data: &str, password: &str) -> Result<String, String> {
    let combined = BASE64
        .decode(encrypted_data)
        .map_err(|e| format!("Invalid base64: {}", e))?;

    if combined.len() < SALT_LENGTH + NONCE_LENGTH {
        return Err("Invalid encrypted data".to_string());
    }

    // Extract salt, nonce, and ciphertext
    let salt = &combined[..SALT_LENGTH];
    let nonce_bytes = &combined[SALT_LENGTH..SALT_LENGTH + NONCE_LENGTH];
    let ciphertext = &combined[SALT_LENGTH + NONCE_LENGTH..];

    // Derive key from password
    let key = derive_key(password, salt);

    // Create cipher and decrypt
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Failed to create cipher: {}", e))?;

    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed (wrong password?): {}", e))?;

    String::from_utf8(plaintext)
        .map_err(|e| format!("Invalid UTF-8: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let password = "test_password_123";
        let plaintext = "0x1234567890abcdef...private_key_here";

        let encrypted = encrypt(plaintext, password).unwrap();
        let decrypted = decrypt(&encrypted, password).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_wrong_password() {
        let password = "correct_password";
        let wrong_password = "wrong_password";
        let plaintext = "secret data";

        let encrypted = encrypt(plaintext, password).unwrap();
        let result = decrypt(&encrypted, wrong_password);

        assert!(result.is_err());
    }
}
