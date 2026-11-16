//! Blocco di avvio con password master (facoltativa).
//!
//! IMPORTANTE: in questa versione la password **blocca l'accesso all'interfaccia**,
//! ma NON cifra ancora i file di database e audio. La cifratura vera è prevista in una fase successiva.
//!
//! Password e risposta alla domanda segreta sono salvate solo come hash Argon2id
//! (mai in chiaro) in `app-lock.json` nella cartella dati dell'app.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime, State};

const LOCK_FILE: &str = "app-lock.json";
const MIN_PASSWORD_CHARS: usize = 8;
const MAX_PASSWORD_CHARS: usize = 256;
const MIN_QUESTION_CHARS: usize = 5;
const MAX_QUESTION_CHARS: usize = 200;
const MIN_ANSWER_CHARS: usize = 3;
const MAX_ANSWER_CHARS: usize = 200;

#[derive(Debug, Serialize, Deserialize)]
struct LockFile {
    version: u8,
    password_hash: String,
    question: String,
    answer_hash: String,
}

/// Stato in memoria: sblocco della sessione corrente e tentativi falliti consecutivi.
#[derive(Default)]
pub struct AppLockState {
    unlocked: Mutex<bool>,
    failed_attempts: Mutex<u32>,
}

#[derive(Debug, Serialize)]
pub struct LockStatus {
    pub configured: bool,
    pub unlocked: bool,
}

fn lock_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "Impossibile trovare la cartella dei dati dell'app.".to_string())?;
    Ok(dir.join(LOCK_FILE))
}

fn read_lock<R: Runtime>(app: &AppHandle<R>) -> Result<Option<LockFile>, String> {
    let path = lock_path(app)?;
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<LockFile>(&text)
            .map(Some)
            .map_err(|_| "Il file del blocco password non è valido.".to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Impossibile leggere il file del blocco password.".to_string()),
    }
}

fn write_lock<R: Runtime>(app: &AppHandle<R>, lock: &LockFile) -> Result<(), String> {
    let path = lock_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|_| "Impossibile creare la cartella dei dati.".to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(lock)
        .map_err(|_| "Impossibile preparare il file del blocco password.".to_string())?;
    std::fs::write(&tmp, json).map_err(|_| "Impossibile salvare la password.".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, &path).map_err(|_| "Impossibile salvare la password.".to_string())
}

/// Normalizza la risposta segreta: maiuscole/minuscole e spazi extra non contano.
fn normalize_answer(answer: &str) -> String {
    answer
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub(crate) fn hash_secret(secret: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(secret.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| "Errore interno durante la protezione della password.".to_string())
}

pub(crate) fn verify_secret(secret: &str, phc: &str) -> bool {
    match PasswordHash::new(phc) {
        Ok(parsed) => Argon2::default()
            .verify_password(secret.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

pub(crate) fn validate_password(password: &str) -> Result<(), String> {
    let n = password.chars().count();
    if n < MIN_PASSWORD_CHARS {
        return Err(format!(
            "La password deve avere almeno {MIN_PASSWORD_CHARS} caratteri."
        ));
    }
    if n > MAX_PASSWORD_CHARS {
        return Err("La password è troppo lunga.".to_string());
    }
    Ok(())
}

fn validate_question_answer(question: &str, answer: &str) -> Result<(String, String), String> {
    let question = question.trim().to_string();
    let q = question.chars().count();
    if !(MIN_QUESTION_CHARS..=MAX_QUESTION_CHARS).contains(&q) {
        return Err("La domanda segreta deve avere tra 5 e 200 caratteri.".to_string());
    }
    let answer = normalize_answer(answer);
    let a = answer.chars().count();
    if !(MIN_ANSWER_CHARS..=MAX_ANSWER_CHARS).contains(&a) {
        return Err("La risposta segreta deve avere almeno 3 caratteri.".to_string());
    }
    Ok((question, answer))
}

/// Rallenta i tentativi ripetuti: dal quarto errore consecutivo attende fino a 30 secondi.
async fn throttle(state: &AppLockState) {
    let failed = *state.failed_attempts.lock().unwrap_or_else(|e| e.into_inner());
    throttle_delay(failed).await;
}

/// Attesa in base ai tentativi falliti consecutivi (condivisa con le cartelle dei pazienti).
pub(crate) async fn throttle_delay(failed: u32) {
    if failed >= 3 {
        let secs = u64::from((failed - 2).min(30));
        tokio::time::sleep(Duration::from_secs(secs)).await;
    }
}

fn register_failure(state: &AppLockState) {
    let mut failed = state.failed_attempts.lock().unwrap_or_else(|e| e.into_inner());
    *failed = failed.saturating_add(1);
}

fn register_success(state: &AppLockState) {
    *state.failed_attempts.lock().unwrap_or_else(|e| e.into_inner()) = 0;
    *state.unlocked.lock().unwrap_or_else(|e| e.into_inner()) = true;
}

/// Controlla la password master per operazioni sensibili (per esempio il backup cifrato).
/// Errori: `MASTER_PASSWORD_REQUIRED` (non impostata) e `WRONG_PASSWORD`.
pub async fn require_master_password<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppLockState,
    password: &str,
) -> Result<(), String> {
    let lock = read_lock(app)?.ok_or_else(|| "MASTER_PASSWORD_REQUIRED".to_string())?;
    throttle(state).await;
    let password = password.to_string();
    let ok = tokio::task::spawn_blocking(move || verify_secret(&password, &lock.password_hash))
        .await
        .map_err(|_| "Errore interno durante la verifica.".to_string())?;
    if ok {
        *state.failed_attempts.lock().unwrap_or_else(|e| e.into_inner()) = 0;
        Ok(())
    } else {
        register_failure(state);
        Err("WRONG_PASSWORD".to_string())
    }
}

#[tauri::command]
pub async fn app_lock_status<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppLockState>,
) -> Result<LockStatus, String> {
    let configured = read_lock(&app)?.is_some();
    let unlocked = *state.unlocked.lock().unwrap_or_else(|e| e.into_inner());
    Ok(LockStatus { configured, unlocked })
}

/// Restituisce la domanda segreta (serve alla schermata "password dimenticata").
#[tauri::command]
pub async fn app_lock_question<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    Ok(read_lock(&app)?.map(|l| l.question))
}

/// Imposta la password master e la domanda segreta. Rifiutato se un blocco esiste già.
#[tauri::command]
pub async fn app_lock_setup<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppLockState>,
    password: String,
    question: String,
    answer: String,
) -> Result<(), String> {
    if read_lock(&app)?.is_some() {
        return Err("Una password master è già impostata.".to_string());
    }
    validate_password(&password)?;
    let (question, answer) = validate_question_answer(&question, &answer)?;

    let (password_hash, answer_hash) = tokio::task::spawn_blocking(move || {
        Ok::<_, String>((hash_secret(&password)?, hash_secret(&answer)?))
    })
    .await
    .map_err(|_| "Errore interno durante la protezione della password.".to_string())??;

    write_lock(
        &app,
        &LockFile {
            version: 1,
            password_hash,
            question,
            answer_hash,
        },
    )?;
    register_success(&state); // dopo l'impostazione la sessione corrente resta sbloccata
    info!("Password master impostata");
    Ok(())
}

/// Verifica la password. `Ok(true)` sblocca la sessione.
#[tauri::command]
pub async fn app_lock_verify<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppLockState>,
    password: String,
) -> Result<bool, String> {
    let lock = read_lock(&app)?.ok_or("Nessuna password master impostata.".to_string())?;
    throttle(&state).await;
    let ok = tokio::task::spawn_blocking(move || verify_secret(&password, &lock.password_hash))
        .await
        .map_err(|_| "Errore interno durante la verifica.".to_string())?;
    if ok {
        register_success(&state);
    } else {
        register_failure(&state);
        warn!("Tentativo di sblocco non riuscito");
    }
    Ok(ok)
}

/// Reimposta la password rispondendo alla domanda segreta. I dati non vengono toccati.
#[tauri::command]
pub async fn app_lock_reset<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppLockState>,
    answer: String,
    new_password: String,
) -> Result<bool, String> {
    let mut lock = read_lock(&app)?.ok_or("Nessuna password master impostata.".to_string())?;
    validate_password(&new_password)?;
    throttle(&state).await;

    let answer = normalize_answer(&answer);
    let answer_hash = lock.answer_hash.clone();
    let ok = tokio::task::spawn_blocking(move || verify_secret(&answer, &answer_hash))
        .await
        .map_err(|_| "Errore interno durante la verifica.".to_string())?;
    if !ok {
        register_failure(&state);
        warn!("Risposta segreta errata");
        return Ok(false);
    }

    lock.password_hash = tokio::task::spawn_blocking(move || hash_secret(&new_password))
        .await
        .map_err(|_| "Errore interno durante la protezione della password.".to_string())??;
    write_lock(&app, &lock)?;
    register_success(&state);
    info!("Password master reimpostata con la domanda segreta");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_normalization_ignores_case_and_spaces() {
        assert_eq!(normalize_answer("  Roma   Termini "), "roma termini");
        assert_eq!(normalize_answer("ROMA TERMINI"), normalize_answer("roma termini"));
    }

    #[test]
    fn hash_roundtrip() {
        let h = hash_secret("correct horse").unwrap();
        assert!(verify_secret("correct horse", &h));
        assert!(!verify_secret("wrong horse", &h));
        assert!(!h.contains("correct"));
    }

    #[test]
    fn password_rules() {
        assert!(validate_password("corta").is_err());
        assert!(validate_password("abbastanza lunga").is_ok());
    }

    #[test]
    fn question_answer_rules() {
        assert!(validate_question_answer("Nome del mio primo gatto?", "Micio").is_ok());
        assert!(validate_question_answer("no", "Micio").is_err());
        assert!(validate_question_answer("Nome del mio primo gatto?", " a ").is_err());
    }
}
