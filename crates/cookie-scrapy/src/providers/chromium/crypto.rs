use aes::cipher::{block_padding::NoPadding, BlockDecryptMut, KeyIvInit};
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use pbkdf2::pbkdf2_hmac;
use sha1::Sha1;

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

pub fn derive_aes128_cbc_key(password: &str, iterations: u32) -> Vec<u8> {
    let mut key = vec![0u8; 16];
    pbkdf2_hmac::<Sha1>(password.as_bytes(), b"saltysalt", iterations, &mut key);
    key
}

pub fn decrypt_chromium_aes128_cbc(
    encrypted_value: &[u8],
    key_candidates: &[Vec<u8>],
    strip_hash_prefix: bool,
    treat_unknown_prefix_as_plaintext: bool,
) -> Option<String> {
    if encrypted_value.len() < 3 {
        return None;
    }

    let prefix = &encrypted_value[..3];
    let has_version_prefix = prefix.len() == 3
        && prefix[0] == b'v'
        && prefix[1].is_ascii_digit()
        && prefix[2].is_ascii_digit();

    if !has_version_prefix {
        if !treat_unknown_prefix_as_plaintext {
            return None;
        }
        return decode_cookie_value_bytes(encrypted_value, false);
    }

    let ciphertext = &encrypted_value[3..];
    if ciphertext.is_empty() {
        return Some(String::new());
    }

    for key in key_candidates {
        if let Some(decrypted) = try_decrypt_aes128_cbc(ciphertext, key) {
            if let Some(decoded) = decode_cookie_value_bytes(&decrypted, strip_hash_prefix) {
                return Some(decoded);
            }
        }
    }

    None
}

pub fn decrypt_chromium_aes256_gcm(
    encrypted_value: &[u8],
    key: &[u8],
    strip_hash_prefix: bool,
) -> Option<String> {
    if encrypted_value.len() < 3 {
        return None;
    }

    let prefix = &encrypted_value[..3];
    let has_version_prefix =
        prefix[0] == b'v' && prefix[1].is_ascii_digit() && prefix[2].is_ascii_digit();

    if !has_version_prefix {
        return None;
    }

    let payload = &encrypted_value[3..];
    // 12 byte nonce + at least 16 byte tag
    if payload.len() < 28 {
        return None;
    }

    let nonce_bytes = &payload[..12];
    let auth_tag = &payload[payload.len() - 16..];
    let ciphertext = &payload[12..payload.len() - 16];

    // Combine ciphertext + auth tag as aes-gcm expects
    let mut combined = Vec::with_capacity(ciphertext.len() + auth_tag.len());
    combined.extend_from_slice(ciphertext);
    combined.extend_from_slice(auth_tag);

    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher.decrypt(nonce, combined.as_ref()).ok()?;

    decode_cookie_value_bytes(&plaintext, strip_hash_prefix)
}

fn try_decrypt_aes128_cbc(ciphertext: &[u8], key: &[u8]) -> Option<Vec<u8>> {
    // Chromium's legacy AES-128-CBC uses an IV of 16 spaces (0x20)
    let iv = [0x20u8; 16];

    // ciphertext must be a multiple of 16
    if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(16) {
        return None;
    }

    let mut buf = ciphertext.to_vec();
    let decryptor = Aes128CbcDec::new_from_slices(key, &iv).ok()?;
    decryptor.decrypt_padded_mut::<NoPadding>(&mut buf).ok()?;

    Some(remove_pkcs7_padding(&buf))
}

fn remove_pkcs7_padding(value: &[u8]) -> Vec<u8> {
    if value.is_empty() {
        return value.to_vec();
    }
    let padding = value[value.len() - 1] as usize;
    if padding == 0 || padding > 16 || padding > value.len() {
        return value.to_vec();
    }
    // Verify padding bytes are all the same
    let start = value.len() - padding;
    if value[start..].iter().all(|&b| b as usize == padding) {
        value[..start].to_vec()
    } else {
        value.to_vec()
    }
}

fn decode_cookie_value_bytes(value: &[u8], strip_hash_prefix: bool) -> Option<String> {
    let bytes = if strip_hash_prefix && value.len() >= 32 {
        &value[32..]
    } else {
        value
    };
    let s = std::str::from_utf8(bytes).ok()?;
    Some(strip_leading_control_chars(s))
}

fn strip_leading_control_chars(value: &str) -> String {
    let trimmed = value.trim_start_matches(|c: char| (c as u32) < 0x20);
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key() {
        let key = derive_aes128_cbc_key("peanuts", 1);
        assert_eq!(key.len(), 16);
    }

    #[test]
    fn test_derive_key_macos_iterations() {
        let key = derive_aes128_cbc_key("test_password", 1003);
        assert_eq!(key.len(), 16);
    }

    #[test]
    fn test_aes128_cbc_roundtrip() {
        use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
        type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;

        let password = "test_password";
        let key = derive_aes128_cbc_key(password, 1003);
        let iv = [0x20u8; 16];
        let plaintext = b"hello_cookie_value";

        let encryptor = Aes128CbcEnc::new_from_slices(&key, &iv).unwrap();
        let mut buf = vec![0u8; plaintext.len() + 16]; // room for padding
        buf[..plaintext.len()].copy_from_slice(plaintext);
        let ciphertext = encryptor
            .encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext.len())
            .unwrap()
            .to_vec();

        // Prepend v10 prefix
        let mut encrypted = b"v10".to_vec();
        encrypted.extend_from_slice(&ciphertext);

        let result = decrypt_chromium_aes128_cbc(&encrypted, &[key], false, false);
        assert_eq!(result, Some("hello_cookie_value".to_string()));
    }

    #[test]
    fn test_aes256_gcm_roundtrip() {
        let key_bytes = [0x42u8; 32];
        let nonce_bytes = [0x01u8; 12];
        let plaintext = b"gcm_cookie_value";

        let cipher = Aes256Gcm::new_from_slice(&key_bytes).unwrap();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext_with_tag = cipher.encrypt(nonce, plaintext.as_ref()).unwrap();

        // Layout: v10 + nonce(12) + ciphertext + tag(16)
        let mut encrypted = b"v10".to_vec();
        encrypted.extend_from_slice(&nonce_bytes);
        // ciphertext_with_tag already has tag appended
        encrypted.extend_from_slice(&ciphertext_with_tag);

        let result = decrypt_chromium_aes256_gcm(&encrypted, &key_bytes, false);
        assert_eq!(result, Some("gcm_cookie_value".to_string()));
    }

    #[test]
    fn test_unknown_prefix_as_plaintext() {
        let data = b"plain_cookie_value";
        let result = decrypt_chromium_aes128_cbc(data, &[], false, true);
        assert_eq!(result, Some("plain_cookie_value".to_string()));
    }

    #[test]
    fn test_unknown_prefix_strict() {
        let data = b"plain_cookie_value";
        let result = decrypt_chromium_aes128_cbc(data, &[], false, false);
        assert!(result.is_none());
    }

    #[test]
    fn test_strip_hash_prefix() {
        let mut data = vec![0u8; 32]; // 32-byte hash prefix
        data.extend_from_slice(b"actual_value");
        let result = decode_cookie_value_bytes(&data, true);
        assert_eq!(result, Some("actual_value".to_string()));
    }
}
