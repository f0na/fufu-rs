use hmac::{Hmac, Mac};
use sha1::Sha1;

use crate::error::{AppError, AppResult};

type HmacSha1 = Hmac<Sha1>;

const TOTP_INTERVAL: u64 = 30;
const CODE_DIGITS: u32 = 6;

/// 生成 Base32 编码的 TOTP 密钥（20 字节随机数）
pub fn generate_secret() -> AppResult<String> {
    let mut buf = [0u8; 20];
    getrandom::getrandom(&mut buf)
        .map_err(|e| AppError::Internal(format!("TOTP 密钥生成失败: {}", e)))?;
    Ok(base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &buf))
}

/// 生成当前 TOTP 验证码
pub fn generate_totp(secret_b32: &str) -> AppResult<String> {
    let secret = decode_secret(secret_b32)?;
    let counter = chrono::Utc::now().timestamp() as u64 / TOTP_INTERVAL;
    compute_totp_at_counter(&secret, counter)
}

/// 验证 TOTP 验证码（允许前后各 1 个时间窗口漂移）
pub fn verify_totp(secret_b32: &str, code: &str) -> AppResult<bool> {
    let secret = decode_secret(secret_b32)?;
    let counter = chrono::Utc::now().timestamp() as u64 / TOTP_INTERVAL;

    // 检查 counter-1, counter, counter+1
    for offset in [u64::MAX, 0, 1] {
        let c = counter.wrapping_add(offset);
        if compute_totp_at_counter(&secret, c)? == code {
            return Ok(true);
        }
    }

    Ok(false)
}

fn compute_totp_at_counter(secret: &[u8], counter: u64) -> AppResult<String> {
    let counter_bytes = counter.to_be_bytes();
    let mut mac = HmacSha1::new_from_slice(secret)
        .map_err(|e| AppError::Internal(format!("HMAC 初始化失败: {}", e)))?;
    mac.update(&counter_bytes);
    let result = mac.finalize().into_bytes();

    // RFC 4226 Section 5.3: 动态截断
    let offset = (result[19] & 0x0f) as usize;
    let code = ((u32::from(result[offset]) & 0x7f) << 24)
        | (u32::from(result[offset + 1]) << 16)
        | (u32::from(result[offset + 2]) << 8)
        | u32::from(result[offset + 3]);

    let code = code % 10u32.pow(CODE_DIGITS);
    Ok(format!("{:0width$}", code, width = CODE_DIGITS as usize))
}

/// 生成 otpauth:// URI，用于二维码
pub fn provisioning_uri(secret_b32: &str, email: &str) -> String {
    format!(
        "otpauth://totp/Fufu:{}?secret={}&issuer=Fufu&algorithm=SHA1&digits={}&period={}",
        email, secret_b32, CODE_DIGITS, TOTP_INTERVAL
    )
}

fn decode_secret(secret_b32: &str) -> AppResult<Vec<u8>> {
    base32::decode(base32::Alphabet::Rfc4648 { padding: false }, secret_b32)
        .ok_or_else(|| AppError::Internal("TOTP 密钥解码失败".into()))
}
