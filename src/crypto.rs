use std::sync::atomic::{AtomicI64, AtomicU16, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, bail, Result};
use chacha20poly1305::{ChaCha20Poly1305, Nonce as ChaChaNonce};
use hkdf::Hkdf;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::Sha256;

static START: OnceLock<Instant> = OnceLock::new();
static SERVER_TS: AtomicI64 = AtomicI64::new(i64::MIN);
static CLIENT_SYNC_TIME: AtomicU64 = AtomicU64::new(0);
static HTTP_SEQ: AtomicU16 = AtomicU16::new(0);

pub fn init_clock() {
    START.get_or_init(Instant::now);
}

fn uptime_secs() -> u64 {
    START.get_or_init(Instant::now).elapsed().as_secs()
}

pub fn mark_server_time(server_ts: i64) {
    SERVER_TS.store(server_ts, Ordering::Relaxed);
    CLIENT_SYNC_TIME.store(uptime_secs(), Ordering::Relaxed);
}

pub fn reset_http_seq(value: u16) {
    HTTP_SEQ.store(value, Ordering::Relaxed);
}

fn make_header10() -> [u8; 10] {
    let server_ts = SERVER_TS.load(Ordering::Relaxed);
    let now = if server_ts == i64::MIN {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0)
    } else {
        server_ts + uptime_secs() as i64 - CLIENT_SYNC_TIME.load(Ordering::Relaxed) as i64
    };
    let seq = HTTP_SEQ.fetch_add(1, Ordering::Relaxed);

    let mut header = [0u8; 10];
    header[..8].copy_from_slice(&(now as u64).to_be_bytes());
    header[8..].copy_from_slice(&seq.to_be_bytes());
    header
}

pub fn encrypt_gcm(plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| anyhow!("Invalid AES-256 key length"))?;
    let mut iv = [0u8; 12];
    OsRng.fill_bytes(&mut iv);
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&iv),
            Payload {
                msg: plaintext,
                aad: &iv,
            },
        )
        .map_err(|_| anyhow!("AES-GCM encryption failed"))?;
    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&iv);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt_gcm(data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 12 + 16 {
        bail!("Invalid GCM payload");
    }
    let (iv, encrypted) = data.split_at(12);
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| anyhow!("Invalid AES-256 key length"))?;
    cipher
        .decrypt(
            Nonce::from_slice(iv),
            Payload {
                msg: encrypted,
                aad: iv,
            },
        )
        .map_err(|_| anyhow!("AES-GCM decryption failed"))
}

pub fn encrypt_chacha(plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| anyhow!("Invalid ChaCha20 key length"))?;
    let mut iv = [0u8; 12];
    OsRng.fill_bytes(&mut iv);
    let ciphertext = cipher
        .encrypt(
            ChaChaNonce::from_slice(&iv),
            Payload {
                msg: plaintext,
                aad: &iv,
            },
        )
        .map_err(|_| anyhow!("ChaCha20-Poly1305 encryption failed"))?;
    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&iv);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt_chacha(data: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 12 + 16 {
        bail!("Invalid ChaCha payload");
    }
    let (iv, encrypted) = data.split_at(12);
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| anyhow!("Invalid ChaCha20 key length"))?;
    cipher
        .decrypt(
            ChaChaNonce::from_slice(iv),
            Payload {
                msg: encrypted,
                aad: iv,
            },
        )
        .map_err(|_| anyhow!("ChaCha20-Poly1305 decryption failed"))
}

pub fn encrypt_basic(buf: &[u8], key: &[u8]) -> Vec<u8> {
    let mut out = buf.to_vec();
    let len = out.len() as u8;
    for index in 0..out.len() {
        out[index] ^= key[index % key.len()];
        let byte = out[index];
        let rotated = (byte << 1) | (byte >> 7);
        out[index] = rotated ^ len;
    }
    out
}

pub fn decrypt_basic(buf: &[u8], key: &[u8]) -> Vec<u8> {
    let mut out = buf.to_vec();
    let len = out.len() as u8;
    for index in 0..out.len() {
        out[index] ^= len;
        let byte = out[index];
        let rotated = (byte >> 1) | (byte << 7);
        out[index] = rotated ^ key[index % key.len()];
    }
    out
}

pub fn hkdf_sha256(ikm: &[u8], salt: &[u8], info: &[u8], length: usize) -> Result<Vec<u8>> {
    let hkdf = Hkdf::<Sha256>::new(Some(salt), ikm);
    let mut out = vec![0u8; length];
    hkdf.expand(info, &mut out)
        .map_err(|_| anyhow!("HKDF expand failed"))?;
    Ok(out)
}

pub fn generate_session_key(
    shared_key: &[u8],
    server_pub: &[u8],
    client_pub: &[u8],
) -> Result<Vec<u8>> {
    if server_pub.is_empty() {
        bail!("Server public key is empty");
    }

    let mut ikm = [0u8; 32];
    let count = shared_key.len().min(32);
    ikm[32 - count..].copy_from_slice(&shared_key[..count]);

    let mut info = vec![0u8; client_pub.len()];
    for (index, client_byte) in client_pub.iter().copied().enumerate() {
        let mut salt = server_pub[index % server_pub.len()];
        if client_byte > salt {
            salt = (salt << 1) & 0xff;
        } else {
            salt = (salt >> 1) & 0xff;
        }
        info[index] = salt ^ client_byte;
    }

    hkdf_sha256(&ikm, server_pub, &info, 32)
}

pub fn build_nova_message(
    msg_id: u16,
    body: &[u8],
    cipher: i32,
    session_key: &[u8],
    garble_key: &[u8],
) -> Result<Vec<u8>> {
    let mut packet = Vec::with_capacity(2 + body.len());
    packet.extend_from_slice(&msg_id.to_be_bytes());
    packet.extend_from_slice(body);

    let mut plaintext = Vec::with_capacity(10 + packet.len());
    plaintext.extend_from_slice(&make_header10());
    plaintext.extend_from_slice(&packet);

    let use_garble = msg_id == 1;
    let key = if use_garble { garble_key } else { session_key };
    let encrypted = if cipher == 1 {
        encrypt_chacha(&plaintext, key)?
    } else {
        encrypt_gcm(&plaintext, key)?
    };
    Ok(if use_garble {
        encrypt_basic(&encrypted, garble_key)
    } else {
        encrypted
    })
}

pub fn decode_nova_message(buf: &[u8], cipher: i32, session_key: &[u8]) -> Result<Vec<u8>> {
    if cipher == 1 {
        decrypt_chacha(buf, session_key)
    } else {
        decrypt_gcm(buf, session_key)
    }
}
