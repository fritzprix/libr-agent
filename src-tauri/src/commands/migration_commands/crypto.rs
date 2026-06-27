use aes_gcm::aead::{rand_core::RngCore, Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

pub const ENC_MAGIC: &[u8] = b"LIBRAGENT_ENC_V1";
pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 12;
pub const PBKDF2_ITERATIONS: u32 = 100_000;

pub fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

pub fn encrypt_data(data: &[u8], password: &str) -> Result<Vec<u8>, String> {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);

    let key_bytes = derive_key(password, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| format!("암호화 키 초기화 실패: {}", e))?;

    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| format!("데이터 암호화 실패: {}", e))?;

    let mut result = Vec::with_capacity(ENC_MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
    result.extend_from_slice(ENC_MAGIC);
    result.extend_from_slice(&salt);
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

pub fn decrypt_data(data: &[u8], password: Option<&str>) -> Result<Option<Vec<u8>>, String> {
    if data.len() < ENC_MAGIC.len() {
        return Ok(None);
    }

    if &data[0..ENC_MAGIC.len()] != ENC_MAGIC {
        return Ok(None); // Plain ZIP
    }

    let pwd = password.ok_or_else(|| "PASSWORD_REQUIRED".to_string())?;

    if data.len() < ENC_MAGIC.len() + SALT_LEN + NONCE_LEN {
        return Err("암호화된 백업 파일의 헤더가 손상되었습니다.".to_string());
    }

    let salt = &data[ENC_MAGIC.len()..(ENC_MAGIC.len() + SALT_LEN)];
    let nonce_bytes = &data[(ENC_MAGIC.len() + SALT_LEN)..(ENC_MAGIC.len() + SALT_LEN + NONCE_LEN)];
    let ciphertext = &data[(ENC_MAGIC.len() + SALT_LEN + NONCE_LEN)..];

    let key_bytes = derive_key(pwd, salt);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| format!("복호화 키 초기화 실패: {}", e))?;

    let nonce = Nonce::from_slice(nonce_bytes);
    let decrypted = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "WRONG_PASSWORD".to_string())?;

    Ok(Some(decrypted))
}
