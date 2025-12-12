use aes_gcm::aead::{Aead, KeyInit, OsRng, rand_core::RngCore};
use aes_gcm::{Aes256Gcm, Nonce};
use snap_coin::crypto::Hash;
use snap_coin::crypto::keys::Private;
use std::collections::HashMap;

/// Compute hash of a PIN (used as encryption key)
fn compute_pin_hash(pin: &str) -> [u8; 32] {
    Hash::new(format!("snap-coin-wallet-{}", pin).as_bytes()).dump_buf()
}

/// Encrypt multiple wallets using a PIN
/// Serialized as: [name_len(u8)|name|private_key(32 bytes)] repeated
pub fn encrypt_wallets(wallets: &HashMap<String, Private>, pin: &str) -> Option<Vec<u8>> {
    let mut serialized = Vec::new();
    for (name, key) in wallets {
        let name_bytes = name.as_bytes();
        if name_bytes.len() > 255 { return None; }
        serialized.push(name_bytes.len() as u8);
        serialized.extend_from_slice(name_bytes);
        serialized.extend_from_slice(key.dump_buf());
    }

    let cipher = Aes256Gcm::new_from_slice(&compute_pin_hash(pin)).ok()?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, serialized.as_ref()).ok()?;
    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Some(out)
}

/// Decrypt multiple wallets using a PIN
pub fn decrypt_wallets(data: &[u8], pin: &str) -> Option<HashMap<String, Private>> {
    if data.len() < 12 { return None; }
    let cipher = Aes256Gcm::new_from_slice(&compute_pin_hash(pin)).ok()?;
    let nonce = Nonce::from_slice(&data[..12]);
    let ciphertext = &data[12..];
    let decrypted = cipher.decrypt(nonce, ciphertext.as_ref()).ok()?;

    let mut wallets = HashMap::new();
    let mut i = 0;
    while i < decrypted.len() {
        let name_len = decrypted[i] as usize;
        i += 1;
        if i + name_len + 32 > decrypted.len() { return None; }
        let name = String::from_utf8_lossy(&decrypted[i..i + name_len]).to_string();
        i += name_len;
        let mut buf = [0u8; 32];
        buf.copy_from_slice(&decrypted[i..i+32]);
        i += 32;
        wallets.insert(name, Private::new_from_buf(&buf));
    }
    Some(wallets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use snap_coin::crypto::keys::Private;
    use std::collections::HashMap;

    #[test]
    fn test_encrypt_decrypt_multi() {
        let mut wallets = HashMap::new();
        wallets.insert("alice".to_string(), Private::new_random());
        wallets.insert("bob".to_string(), Private::new_random());
        let pin = "123456";

        let encrypted = encrypt_wallets(&wallets, pin).expect("encryption failed");
        let decrypted = decrypt_wallets(&encrypted, pin).expect("decryption failed");

        assert_eq!(wallets.len(), decrypted.len());
        for (name, key) in wallets {
            let dec_key = decrypted.get(&name).unwrap();
            assert_eq!(key.dump_buf(), dec_key.dump_buf());
        }
    }
}
