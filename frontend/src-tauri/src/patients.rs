//! Cartelle per paziente: comandi Tauri e password facoltativa della cartella.
//!
//! IMPORTANTE: la password di un paziente è un **blocco di interfaccia**: finché la cartella non è
//! sbloccata nella sessione corrente l'app non restituisce giorni, sedute e note del paziente.
//! NON cifra ancora i dati su disco (database e audio): la cifratura vera è prevista in una fase
//! successiva. L'hash Argon2id sta nella colonna `patients.password_hash`; la password non viene
//! mai salvata né registrata nei log. Il throttling dei tentativi è lo stesso di `app_lock.rs`.

use crate::app_lock::{hash_secret, throttle_delay, validate_password, verify_secret};
use crate::database::repositories::patient::{PatientRow, PatientsRepository, PatientsTree};
use crate::state::AppState;
use log::{info, warn};
use serde::Serialize;
use std::collections::HashSet;
use std::sync::Mutex;
use tauri::State;

/// Stato in memoria: cartelle sbloccate nella sessione corrente e tentativi falliti consecutivi.
#[derive(Default)]
pub struct PatientLockState {
    unlocked: Mutex<HashSet<String>>,
    failed_attempts: Mutex<u32>,
}

impl PatientLockState {
    pub fn unlocked_ids(&self) -> HashSet<String> {
        self.unlocked.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn is_unlocked(&self, id: &str) -> bool {
        self.unlocked.lock().unwrap_or_else(|e| e.into_inner()).contains(id)
    }

    fn failed(&self) -> u32 {
        *self.failed_attempts.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn register_failure(&self) {
        let mut f = self.failed_attempts.lock().unwrap_or_else(|e| e.into_inner());
        *f = f.saturating_add(1);
    }

    fn register_success(&self, id: &str) {
        *self.failed_attempts.lock().unwrap_or_else(|e| e.into_inner()) = 0;
        self.unlocked.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string());
    }

    fn forget(&self, id: &str) {
        self.unlocked.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    }
}

/// `Ok(())` se la cartella non è protetta oppure è stata sbloccata in questa sessione.
pub async fn ensure_open(
    pool: &sqlx::SqlitePool,
    lock: &PatientLockState,
    patient_id: &str,
) -> Result<(), String> {
    let row = PatientsRepository::get(pool, patient_id)
        .await
        .map_err(|_| "Errore del database.".to_string())?
        .ok_or_else(|| "Paziente non trovato.".to_string())?;
    if row.password_hash.is_some() && !lock.is_unlocked(patient_id) {
        return Err("PATIENT_LOCKED".to_string());
    }
    Ok(())
}

/// Come `ensure_open`, ma partendo dall'id di una seduta (senza paziente = sempre aperta).
pub async fn ensure_meeting_open(
    pool: &sqlx::SqlitePool,
    lock: &PatientLockState,
    meeting_id: &str,
) -> Result<(), String> {
    match PatientsRepository::patient_of_meeting(pool, meeting_id)
        .await
        .map_err(|_| "Errore del database.".to_string())?
    {
        Some(pid) => ensure_open(pool, lock, &pid).await,
        None => Ok(()),
    }
}

/// Verifica la password con throttling. `Ok(true)` = corretta.
async fn check_password(
    lock: &PatientLockState,
    row: &PatientRow,
    password: String,
) -> Result<bool, String> {
    let hash = row
        .password_hash
        .clone()
        .ok_or_else(|| "Questo paziente non ha una password.".to_string())?;
    throttle_delay(lock.failed()).await;
    let ok = tokio::task::spawn_blocking(move || verify_secret(&password, &hash))
        .await
        .map_err(|_| "Errore interno durante la verifica.".to_string())?;
    if ok {
        *lock.failed_attempts.lock().unwrap_or_else(|e| e.into_inner()) = 0;
    } else {
        lock.register_failure();
        warn!("Tentativo di sblocco di una cartella paziente non riuscito");
    }
    Ok(ok)
}

async fn load(state: &AppState, id: &str) -> Result<PatientRow, String> {
    PatientsRepository::get(state.db_manager.pool(), id)
        .await
        .map_err(|_| "Errore del database.".to_string())?
        .ok_or_else(|| "Paziente non trovato.".to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatientCreated {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatientDeleted {
    pub moved_sessions: usize,
}

/// Albero pazienti > giorni > sedute. Le cartelle protette e chiuse non mostrano il contenuto.
#[tauri::command]
pub async fn patients_list(
    state: State<'_, AppState>,
    lock: State<'_, PatientLockState>,
    include_archived: Option<bool>,
) -> Result<PatientsTree, String> {
    PatientsRepository::tree(state.db_manager.pool(), &lock.unlocked_ids(), include_archived.unwrap_or(false)).await
}

#[tauri::command]
pub async fn patient_create(
    state: State<'_, AppState>,
    lock: State<'_, PatientLockState>,
    name: String,
    notes: Option<String>,
) -> Result<PatientCreated, String> {
    let row = PatientsRepository::create(state.db_manager.pool(), &name, notes).await?;
    // una cartella appena creata non ha password: nulla da sbloccare
    let _ = &lock;
    info!("Cartella paziente creata");
    Ok(PatientCreated { id: row.id, name: row.name })
}

#[tauri::command]
pub async fn patient_update(
    state: State<'_, AppState>,
    lock: State<'_, PatientLockState>,
    id: String,
    name: String,
    notes: Option<String>,
) -> Result<(), String> {
    ensure_open(state.db_manager.pool(), &lock, &id).await?;
    PatientsRepository::update(state.db_manager.pool(), &id, &name, notes).await
}

#[tauri::command]
pub async fn patient_set_archived(
    state: State<'_, AppState>,
    lock: State<'_, PatientLockState>,
    id: String,
    archived: bool,
) -> Result<(), String> {
    ensure_open(state.db_manager.pool(), &lock, &id).await?;
    PatientsRepository::set_archived(state.db_manager.pool(), &id, archived).await
}

/// Elimina la cartella: le sedute NON vengono cancellate, tornano in "Senza paziente".
/// La conferma va chiesta nell'interfaccia. Una cartella protetta va prima sbloccata.
#[tauri::command]
pub async fn patient_delete(
    state: State<'_, AppState>,
    lock: State<'_, PatientLockState>,
    id: String,
) -> Result<PatientDeleted, String> {
    ensure_open(state.db_manager.pool(), &lock, &id).await?;
    let moved = PatientsRepository::delete(state.db_manager.pool(), &id).await?;
    lock.forget(&id);
    info!("Cartella paziente eliminata ({moved} sedute spostate in \"Senza paziente\")");
    Ok(PatientDeleted { moved_sessions: moved })
}

/// Imposta o cambia la password della cartella. Se esiste già una password serve quella attuale.
#[tauri::command]
pub async fn patient_set_password(
    state: State<'_, AppState>,
    lock: State<'_, PatientLockState>,
    id: String,
    new_password: String,
    current_password: Option<String>,
) -> Result<bool, String> {
    let row = load(&state, &id).await?;
    validate_password(&new_password)?;
    if row.password_hash.is_some() {
        let current = current_password.ok_or_else(|| "WRONG_PASSWORD".to_string())?;
        if !check_password(&lock, &row, current).await? {
            return Ok(false);
        }
    }
    let hash = tokio::task::spawn_blocking(move || hash_secret(&new_password))
        .await
        .map_err(|_| "Errore interno durante la protezione della password.".to_string())??;
    PatientsRepository::set_password_hash(state.db_manager.pool(), &id, Some(&hash)).await?;
    lock.register_success(&id); // dopo l'impostazione la cartella resta aperta in questa sessione
    info!("Password di una cartella paziente impostata");
    Ok(true)
}

/// Rimuove la password della cartella (serve la password attuale).
#[tauri::command]
pub async fn patient_remove_password(
    state: State<'_, AppState>,
    lock: State<'_, PatientLockState>,
    id: String,
    password: String,
) -> Result<bool, String> {
    let row = load(&state, &id).await?;
    if !check_password(&lock, &row, password).await? {
        return Ok(false);
    }
    PatientsRepository::set_password_hash(state.db_manager.pool(), &id, None).await?;
    lock.forget(&id);
    info!("Password di una cartella paziente rimossa");
    Ok(true)
}

/// Verifica la password: `Ok(true)` sblocca la cartella per la sessione corrente.
#[tauri::command]
pub async fn patient_unlock(
    state: State<'_, AppState>,
    lock: State<'_, PatientLockState>,
    id: String,
    password: String,
) -> Result<bool, String> {
    let row = load(&state, &id).await?;
    let ok = check_password(&lock, &row, password).await?;
    if ok {
        lock.register_success(&id);
    }
    Ok(ok)
}

#[tauri::command]
pub async fn patient_lock(lock: State<'_, PatientLockState>, id: String) -> Result<(), String> {
    lock.forget(&id);
    Ok(())
}

/// Assegna una seduta a un paziente (`patient_id = None` → "Senza paziente") e, se indicato,
/// a un giorno (`AAAA-MM-GG`). Il paziente di destinazione e quello di origine devono essere aperti.
#[tauri::command]
pub async fn meeting_assign_patient(
    state: State<'_, AppState>,
    lock: State<'_, PatientLockState>,
    meeting_id: String,
    patient_id: Option<String>,
    session_date: Option<String>,
) -> Result<(), String> {
    let pool = state.db_manager.pool();
    ensure_meeting_open(pool, &lock, &meeting_id).await?;
    if let Some(pid) = patient_id.as_deref() {
        ensure_open(pool, &lock, pid).await?;
    }
    PatientsRepository::assign_meeting(pool, &meeting_id, patient_id.as_deref(), session_date.as_deref()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlock_state_tracks_folders_independently() {
        let s = PatientLockState::default();
        assert!(!s.is_unlocked("a"));
        s.register_failure();
        s.register_failure();
        assert_eq!(s.failed(), 2);
        s.register_success("a");
        assert_eq!(s.failed(), 0, "un successo azzera i tentativi falliti");
        assert!(s.is_unlocked("a") && !s.is_unlocked("b"));
        s.forget("a");
        assert!(!s.is_unlocked("a"));
    }

    #[tokio::test]
    async fn password_check_and_throttle_counters() {
        let s = PatientLockState::default();
        let hash = hash_secret("password-lunga").unwrap();
        let row = PatientRow {
            id: "p".into(),
            name: "P".into(),
            notes: None,
            created_at: String::new(),
            updated_at: String::new(),
            password_hash: Some(hash),
            archived: false,
        };
        assert!(!check_password(&s, &row, "sbagliata!!".into()).await.unwrap());
        assert_eq!(s.failed(), 1);
        assert!(check_password(&s, &row, "password-lunga".into()).await.unwrap());
        assert_eq!(s.failed(), 0);
        let open = PatientRow { password_hash: None, ..row };
        assert!(check_password(&s, &open, "x".into()).await.is_err());
    }

    #[tokio::test]
    async fn ensure_open_blocks_locked_folders() {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let p = PatientsRepository::create(&pool, "Marta", None).await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at) VALUES ('m1','t','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')")
            .execute(&pool).await.unwrap();
        PatientsRepository::assign_meeting(&pool, "m1", Some(&p.id), None).await.unwrap();
        let lock = PatientLockState::default();
        assert!(ensure_open(&pool, &lock, &p.id).await.is_ok());
        PatientsRepository::set_password_hash(&pool, &p.id, Some("h")).await.unwrap();
        assert_eq!(ensure_open(&pool, &lock, &p.id).await.unwrap_err(), "PATIENT_LOCKED");
        assert_eq!(ensure_meeting_open(&pool, &lock, "m1").await.unwrap_err(), "PATIENT_LOCKED");
        lock.register_success(&p.id);
        assert!(ensure_meeting_open(&pool, &lock, "m1").await.is_ok());
        assert!(ensure_meeting_open(&pool, &lock, "senza-paziente").await.is_ok());
    }
}
