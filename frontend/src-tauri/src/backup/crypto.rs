//! Contenitore cifrato dei backup (`.tsbackup`).
//!
//! Formato:
//! ```text
//! "TSBKUP01" | lunghezza intestazione (u32 big-endian) | intestazione JSON | blocchi cifrati
//! ```
//! - L'intestazione è in chiaro ma **autenticata**: qualsiasi modifica rende il file illeggibile.
//!   Contiene solo versione, data, parametri della chiave e nonce: nessun dato clinico.
//! - La chiave deriva dalla password con Argon2id (sale casuale per ogni backup).
//! - I dati sono cifrati a blocchi da 1 MiB con XChaCha20-Poly1305 (STREAM): un file troncato,
//!   riordinato o alterato viene rifiutato.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chacha20poly1305::aead::stream::{DecryptorBE32, EncryptorBE32};
use chacha20poly1305::aead::{KeyInit, Payload};
use chacha20poly1305::XChaCha20Poly1305;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use zeroize::Zeroizing;

pub const MAGIC: &[u8; 8] = b"TSBKUP01";
const CIPHER_NAME: &str = "xchacha20poly1305-stream-be32";
const TAG_LEN: usize = 16;
const STREAM_NONCE_LEN: usize = 19; // 24 (XChaCha) - 5 (contatore + flag di ultimo blocco)
pub const DEFAULT_CHUNK: u32 = 1024 * 1024;
const MAX_HEADER_LEN: u32 = 16 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    pub alg: String,
    pub m_kib: u32,
    pub t: u32,
    pub p: u32,
    pub salt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub format: u8,
    pub app: String,
    pub app_version: String,
    pub created_at: String,
    pub kdf: KdfParams,
    pub cipher: String,
    pub nonce: String,
    pub chunk_size: u32,
}

#[derive(Debug)]
pub enum CryptoError {
    NotABackup,
    UnsupportedVersion,
    BadHeader,
    Io(io::Error),
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotABackup => write!(f, "Il file non è un backup di Trascrivi Seduta."),
            Self::UnsupportedVersion => write!(f, "Questo backup è stato creato da una versione più recente dell'app."),
            Self::BadHeader => write!(f, "L'intestazione del backup non è valida."),
            Self::Io(e) => write!(f, "Errore di lettura o scrittura: {e}"),
        }
    }
}

impl From<io::Error> for CryptoError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Deriva la chiave a 256 bit dalla password. Costoso di proposito (resiste ai tentativi a forza bruta).
pub fn derive_key(password: &str, kdf: &KdfParams) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    if kdf.alg != "argon2id" || kdf.m_kib > 512 * 1024 || kdf.t == 0 || kdf.t > 10 || kdf.p == 0 || kdf.p > 8 {
        return Err(CryptoError::BadHeader);
    }
    let salt = B64.decode(&kdf.salt).map_err(|_| CryptoError::BadHeader)?;
    let params = Params::new(kdf.m_kib, kdf.t, kdf.p, Some(32)).map_err(|_| CryptoError::BadHeader)?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), &salt, key.as_mut())
        .map_err(|_| CryptoError::BadHeader)?;
    Ok(key)
}

fn new_header(app_version: &str, created_at: &str, chunk_size: u32) -> Header {
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; STREAM_NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);
    Header {
        format: 1,
        app: "Trascrivi Seduta".to_string(),
        app_version: app_version.to_string(),
        created_at: created_at.to_string(),
        kdf: KdfParams {
            alg: "argon2id".to_string(),
            // nei test si usano parametri leggeri per non rallentarli; in produzione 64 MiB e 3 passate
            m_kib: if cfg!(test) { 1024 } else { 64 * 1024 },
            t: if cfg!(test) { 1 } else { 3 },
            p: 1,
            salt: B64.encode(salt),
        },
        cipher: CIPHER_NAME.to_string(),
        nonce: B64.encode(nonce),
        chunk_size,
    }
}

/// Scrive i blocchi cifrati. Usare `finish()` alla fine per sigillare l'ultimo blocco.
pub struct EncryptWriter<W: Write> {
    inner: W,
    enc: Option<EncryptorBE32<XChaCha20Poly1305>>,
    buf: Vec<u8>,
    chunk: usize,
    aad: Vec<u8>,
}

impl<W: Write> EncryptWriter<W> {
    /// Scrive intestazione e prepara la cifratura.
    pub fn create(
        mut inner: W,
        password: &str,
        app_version: &str,
        created_at: &str,
        chunk_size: u32,
    ) -> Result<Self, CryptoError> {
        let header = new_header(app_version, created_at, chunk_size);
        let header_json = serde_json::to_vec(&header).map_err(|_| CryptoError::BadHeader)?;
        let key = derive_key(password, &header.kdf)?;
        let nonce = B64.decode(&header.nonce).map_err(|_| CryptoError::BadHeader)?;
        let cipher = XChaCha20Poly1305::new(key.as_ref().into());
        let enc = EncryptorBE32::from_aead(cipher, nonce.as_slice().into());

        inner.write_all(MAGIC)?;
        inner.write_all(&(header_json.len() as u32).to_be_bytes())?;
        inner.write_all(&header_json)?;
        Ok(Self {
            inner,
            enc: Some(enc),
            buf: Vec::with_capacity(chunk_size as usize),
            chunk: chunk_size as usize,
            aad: header_json,
        })
    }

    fn flush_full(&mut self) -> io::Result<()> {
        let enc = self.enc.as_mut().expect("cifratore attivo");
        let ct = enc
            .encrypt_next(Payload { msg: &self.buf, aad: &self.aad })
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "cifratura non riuscita"))?;
        self.inner.write_all(&ct)?;
        self.buf.clear();
        Ok(())
    }

    /// Sigilla l'ultimo blocco e restituisce il writer sottostante.
    pub fn finish(mut self) -> io::Result<W> {
        let enc = self.enc.take().expect("cifratore attivo");
        let ct = enc
            .encrypt_last(Payload { msg: &self.buf, aad: &self.aad })
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "cifratura non riuscita"))?;
        self.inner.write_all(&ct)?;
        self.inner.flush()?;
        Ok(self.inner)
    }
}

impl<W: Write> Write for EncryptWriter<W> {
    fn write(&mut self, mut data: &[u8]) -> io::Result<usize> {
        let total = data.len();
        while !data.is_empty() {
            if self.buf.len() == self.chunk {
                self.flush_full()?;
            }
            let take = (self.chunk - self.buf.len()).min(data.len());
            self.buf.extend_from_slice(&data[..take]);
            data = &data[take..];
        }
        Ok(total)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(()) // un blocco parziale non si può scrivere prima di `finish()`
    }
}

/// Legge e decifra. Se la password è errata o il file è alterato, la lettura restituisce un errore.
pub struct DecryptReader<R: Read> {
    inner: R,
    dec: Option<DecryptorBE32<XChaCha20Poly1305>>,
    aad: Vec<u8>,
    block_len: usize,
    next_ct: Vec<u8>,
    cur: Vec<u8>,
    pos: usize,
    finished: bool,
    pub header: Header,
}

fn read_block<R: Read>(r: &mut R, len: usize) -> io::Result<Vec<u8>> {
    let mut buf = vec![0u8; len];
    let mut got = 0;
    while got < len {
        let n = r.read(&mut buf[got..])?;
        if n == 0 {
            break;
        }
        got += n;
    }
    buf.truncate(got);
    Ok(buf)
}

impl<R: Read> DecryptReader<R> {
    pub fn open(mut inner: R, password: &str) -> Result<Self, CryptoError> {
        let mut magic = [0u8; 8];
        if inner.read_exact(&mut magic).is_err() || &magic != MAGIC {
            return Err(CryptoError::NotABackup);
        }
        let mut len_bytes = [0u8; 4];
        inner.read_exact(&mut len_bytes).map_err(|_| CryptoError::BadHeader)?;
        let len = u32::from_be_bytes(len_bytes);
        if len == 0 || len > MAX_HEADER_LEN {
            return Err(CryptoError::BadHeader);
        }
        let mut header_json = vec![0u8; len as usize];
        inner.read_exact(&mut header_json).map_err(|_| CryptoError::BadHeader)?;
        let header: Header = serde_json::from_slice(&header_json).map_err(|_| CryptoError::BadHeader)?;
        if header.format != 1 {
            return Err(CryptoError::UnsupportedVersion);
        }
        if header.cipher != CIPHER_NAME || header.chunk_size == 0 || header.chunk_size > 16 * 1024 * 1024 {
            return Err(CryptoError::BadHeader);
        }
        let nonce = B64.decode(&header.nonce).map_err(|_| CryptoError::BadHeader)?;
        if nonce.len() != STREAM_NONCE_LEN {
            return Err(CryptoError::BadHeader);
        }
        let key = derive_key(password, &header.kdf)?;
        let cipher = XChaCha20Poly1305::new(key.as_ref().into());
        let dec = DecryptorBE32::from_aead(cipher, nonce.as_slice().into());
        let block_len = header.chunk_size as usize + TAG_LEN;
        let next_ct = read_block(&mut inner, block_len)?;
        Ok(Self {
            inner,
            dec: Some(dec),
            aad: header_json,
            block_len,
            next_ct,
            cur: Vec::new(),
            pos: 0,
            finished: false,
            header,
        })
    }

    fn fill(&mut self) -> io::Result<bool> {
        if self.finished {
            return Ok(false);
        }
        let ct = std::mem::take(&mut self.next_ct);
        if ct.is_empty() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "backup troncato"));
        }
        let lookahead = read_block(&mut self.inner, self.block_len)?;
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "password errata o backup danneggiato");
        if lookahead.is_empty() {
            let dec = self.dec.take().expect("decifratore attivo");
            self.cur = dec.decrypt_last(Payload { msg: &ct, aad: &self.aad }).map_err(|_| bad())?;
            self.finished = true;
        } else {
            let dec = self.dec.as_mut().expect("decifratore attivo");
            self.cur = dec.decrypt_next(Payload { msg: &ct, aad: &self.aad }).map_err(|_| bad())?;
            self.next_ct = lookahead;
        }
        self.pos = 0;
        Ok(true)
    }
}

impl<R: Read> Read for DecryptReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        while self.pos >= self.cur.len() {
            if !self.fill()? {
                return Ok(0);
            }
        }
        let n = (self.cur.len() - self.pos).min(out.len());
        out[..n].copy_from_slice(&self.cur[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // Parametri di test: meno memoria per non rallentare i test (la funzione reale usa 64 MiB).
    fn encrypt(data: &[u8], password: &str, chunk: u32) -> Vec<u8> {
        let mut w = EncryptWriter::create(Vec::new(), password, "test", "2026-01-01T00:00:00Z", chunk).unwrap();
        w.write_all(data).unwrap();
        w.finish().unwrap()
    }

    fn decrypt(blob: &[u8], password: &str) -> io::Result<Vec<u8>> {
        let mut r = DecryptReader::open(Cursor::new(blob), password).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        let mut out = Vec::new();
        r.read_to_end(&mut out)?;
        Ok(out)
    }

    #[test]
    fn roundtrip_various_sizes() {
        for chunk in [16u32, 64] {
            for len in [0usize, 1, 15, 16, 17, 63, 64, 65, 128, 1000] {
                let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
                let blob = encrypt(&data, "pw-di-prova", chunk);
                assert_eq!(decrypt(&blob, "pw-di-prova").unwrap(), data, "chunk={chunk} len={len}");
            }
        }
    }

    #[test]
    fn wrong_password_is_rejected() {
        let blob = encrypt(b"dati clinici riservati", "giusta-password", 64);
        assert!(decrypt(&blob, "sbagliata-password").is_err());
    }

    #[test]
    fn plaintext_is_not_visible() {
        let blob = encrypt(b"MARIO ROSSI ansia", "pw-di-prova", 64);
        assert!(!blob.windows(5).any(|w| w == b"MARIO"));
    }

    #[test]
    fn tampering_is_detected() {
        let data = vec![7u8; 500];
        let blob = encrypt(&data, "pw-di-prova", 64);
        // altera un byte del contenuto cifrato
        let mut altered = blob.clone();
        let last = altered.len() - 20;
        altered[last] ^= 0x01;
        assert!(decrypt(&altered, "pw-di-prova").is_err());
        // altera l'intestazione (autenticata)
        let mut header_altered = blob.clone();
        header_altered[20] ^= 0x01;
        assert!(decrypt(&header_altered, "pw-di-prova").is_err());
    }

    #[test]
    fn truncation_is_detected() {
        let data = vec![9u8; 300];
        let blob = encrypt(&data, "pw-di-prova", 64);
        // taglia l'ultimo blocco intero
        let block = 64 + TAG_LEN;
        let cut = &blob[..blob.len() - block.min(blob.len() / 3)];
        assert!(decrypt(cut, "pw-di-prova").is_err());
        // taglia appena un byte
        assert!(decrypt(&blob[..blob.len() - 1], "pw-di-prova").is_err());
    }

    #[test]
    fn rejects_files_that_are_not_backups() {
        assert!(matches!(
            DecryptReader::open(Cursor::new(b"non e' un backup".to_vec()), "x"),
            Err(CryptoError::NotABackup)
        ));
    }

    #[test]
    fn each_backup_has_a_fresh_salt_and_nonce() {
        let a = encrypt(b"stesso contenuto", "pw", 64);
        let b = encrypt(b"stesso contenuto", "pw", 64);
        assert_ne!(a, b);
    }
}
