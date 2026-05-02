// src/auth/hash.rs - Password Hashing

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Hash password using HMAC-SHA256
///
/// # Arguments
/// * `password` - Plain text password
/// * `secret` - Secret key for HMAC
///
/// # Returns
/// * `String` - Base64 encoded hash
///
/// # Note
/// This is suitable for demo purposes.
/// For production, consider using argon2 or bcrypt.
pub fn hash_password(password: &str, secret: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(password.as_bytes());
    let result = mac.finalize();
    BASE64.encode(result.into_bytes())
}

/// Verify password against stored hash
///
/// # Arguments
/// * `password` - Plain text password to verify
/// * `secret` - Secret key used for hashing
/// * `hash` - Stored hash to compare against
///
/// # Returns
/// * `bool` - True if password matches
pub fn verify_password(password: &str, secret: &str, hash: &str) -> bool {
    let expected = hash_password(password, secret);
    // Use constant-time comparison in production
    expected == hash
}