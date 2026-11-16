//! Backup cifrato di sedute, trascrizioni, riassunti, appunti e registrazioni audio.
//!
//! Il file `.tsbackup` si può salvare dove si vuole (disco, chiavetta, cartella Dropbox/Drive):
//! è cifrato con una chiave derivata dalla password master, quindi è illeggibile senza di essa.
//! L'app non si collega a nessun servizio cloud: scegli tu la destinazione.
//!
//! Contenuto dell'archivio (dentro il contenitore cifrato, compresso con gzip):
//! `manifest.json`, `db.json` e `audio/<id seduta>/<file audio>`.
//!
//! Pazienti: `db.json` contiene anche la tabella `patients` (con `password_hash`, mai la password)
//! e le colonne `patient_id`/`session_date` delle sedute. Il ripristino non sovrascrive mai
//! pazienti o sedute già presenti. Le password delle cartelle restano un blocco di interfaccia.

pub mod crypto;

use crate::app_lock::{require_master_password, AppLockState};
use crate::audio::recording_preferences::load_recording_preferences;
use crate::state::AppState;
use crypto::{CryptoError, DecryptReader, EncryptWriter, DEFAULT_CHUNK};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::sqlite::SqliteRow;
use sqlx::{Column, Row, SqlitePool, TypeInfo, ValueRef};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Runtime, State};

const AUDIO_EXTENSIONS: [&str; 7] = ["mp4", "m4a", "wav", "mp3", "flac", "ogg", "opus"];
const FILE_EXTENSION: &str = "tsbackup";
const MAX_JSON_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct BackupMeetingInfo {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub audio_files: u32,
    pub audio_bytes: u64,
    pub patient_id: Option<String>,
    pub session_date: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPatientInfo {
    pub id: String,
    pub name: String,
    pub protected: bool,
    pub session_count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Include {
    pub transcripts: bool,
    pub summaries: bool,
    pub notes: bool,
    pub audio: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRequest {
    pub password: String,
    /// `None` = tutte le sedute
    pub meeting_ids: Option<Vec<String>>,
    /// Pazienti da includere (anche senza sedute). `None` con `meeting_ids` `None` = tutti i pazienti;
    /// con `meeting_ids` `None` e `patient_ids` presente = tutte le sedute di quei pazienti.
    #[serde(default)]
    pub patient_ids: Option<Vec<String>>,
    pub include: Include,
    pub destination: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub path: String,
    pub meetings: usize,
    pub audio_files: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestMeeting {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub audio_files: u32,
    #[serde(default)]
    pub patient_id: Option<String>,
    #[serde(default)]
    pub session_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestPatient {
    pub id: String,
    pub name: String,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u8,
    pub app_version: String,
    pub created_at: String,
    pub include: Include,
    pub meetings: Vec<ManifestMeeting>,
    #[serde(default)]
    pub patients: Vec<ManifestPatient>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRequest {
    pub path: String,
    pub password: String,
    /// `None` = tutte le sedute presenti nel backup
    pub meeting_ids: Option<Vec<String>>,
    /// Pazienti da ripristinare anche se senza sedute scelte. Con `meeting_ids` `None` e
    /// `patient_ids` `None` si ripristinano tutti i pazienti del backup.
    #[serde(default)]
    pub patient_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub restored: usize,
    pub skipped_existing: usize,
    pub audio_files: usize,
    pub patients_restored: usize,
}

#[derive(Serialize, Clone)]
struct Progress<'a> {
    stage: &'a str,
    done: u64,
    total: u64,
}

fn emit<R: Runtime>(app: &AppHandle<R>, stage: &str, done: u64, total: u64) {
    let _ = app.emit("backup-progress", Progress { stage, done, total });
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// File audio nella cartella di una seduta (solo il primo livello).
fn audio_files_in(folder: &Path) -> Vec<(PathBuf, u64)> {
    let mut out = Vec::new();
    if let Ok(read) = fs::read_dir(folder) {
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_file() && is_audio(&path) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !name.starts_with('.') {
                        let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        out.push((path, len));
                    }
                }
            }
        }
    }
    out.sort();
    out
}

fn friendly(e: &CryptoError) -> String {
    e.to_string()
}

fn io_message(e: &std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::InvalidData || e.kind() == std::io::ErrorKind::UnexpectedEof {
        "Password errata oppure il file di backup è danneggiato.".to_string()
    } else {
        format!("Errore di lettura o scrittura: {e}")
    }
}

// ---------- database <-> JSON ----------

fn row_to_json(row: &SqliteRow) -> Value {
    let mut map = Map::new();
    for (i, col) in row.columns().iter().enumerate() {
        let value = match row.try_get_raw(i) {
            Ok(raw) if !raw.is_null() => match raw.type_info().name() {
                "INTEGER" | "BOOLEAN" => row.try_get::<i64, _>(i).map(Value::from).unwrap_or(Value::Null),
                "REAL" => row.try_get::<f64, _>(i).map(Value::from).unwrap_or(Value::Null),
                "BLOB" => row
                    .try_get::<Vec<u8>, _>(i)
                    .map(|b| Value::from(base64_encode(&b)))
                    .unwrap_or(Value::Null),
                _ => row.try_get::<String, _>(i).map(Value::from).unwrap_or(Value::Null),
            },
            _ => Value::Null,
        };
        map.insert(col.name().to_string(), value);
    }
    Value::Object(map)
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

async fn select_by_ids(pool: &SqlitePool, table: &str, column: &str, ids: &[String]) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for batch in ids.chunks(400) {
        let marks = vec!["?"; batch.len()].join(",");
        let sql = format!("SELECT * FROM \"{table}\" WHERE \"{column}\" IN ({marks})");
        let mut q = sqlx::query(&sql);
        for id in batch {
            q = q.bind(id);
        }
        let rows = q.fetch_all(pool).await.map_err(|e| format!("Errore del database ({table}): {e}"))?;
        out.extend(rows.iter().map(row_to_json));
    }
    Ok(out)
}

/// Pazienti da salvare: quelli delle sedute scelte, più quelli richiesti esplicitamente
/// (oppure tutti, se non si è scelto nulla).
fn select_patient_ids(
    referenced: &[Option<String>],
    requested: Option<&[String]>,
    all_meetings: bool,
    all_patients: &[String],
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |id: &String| {
        if !out.contains(id) {
            out.push(id.clone());
        }
    };
    for id in referenced.iter().flatten() {
        push(id);
    }
    match requested {
        Some(req) => req.iter().filter(|id| all_patients.contains(id)).for_each(&mut push),
        None if all_meetings => all_patients.iter().for_each(&mut push),
        None => {}
    }
    out
}

async fn export_db(pool: &SqlitePool, ids: &[String], patient_ids: &[String], include: &Include) -> Result<Value, String> {
    let mut db = Map::new();
    db.insert("patients".into(), Value::Array(select_by_ids(pool, "patients", "id", patient_ids).await?));
    db.insert("meetings".into(), Value::Array(select_by_ids(pool, "meetings", "id", ids).await?));
    if include.transcripts {
        db.insert("transcripts".into(), Value::Array(select_by_ids(pool, "transcripts", "meeting_id", ids).await?));
        db.insert("transcript_chunks".into(), Value::Array(select_by_ids(pool, "transcript_chunks", "meeting_id", ids).await?));
    }
    if include.summaries {
        db.insert("summary_processes".into(), Value::Array(select_by_ids(pool, "summary_processes", "meeting_id", ids).await?));
    }
    if include.notes {
        db.insert("meeting_notes".into(), Value::Array(select_by_ids(pool, "meeting_notes", "meeting_id", ids).await?));
    }
    Ok(Value::Object(db))
}

/// Tabelle ripristinabili con la colonna che le lega alla seduta. L'ordine rispetta le chiavi esterne.
const RESTORE_TABLES: [(&str, &str); 5] = [
    ("meetings", "id"),
    ("transcripts", "meeting_id"),
    ("transcript_chunks", "meeting_id"),
    ("summary_processes", "meeting_id"),
    ("meeting_notes", "meeting_id"),
];

async fn insert_rows(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    table: &str,
    columns: &HashSet<String>,
    row: &Map<String, Value>,
) -> Result<(), String> {
    let keys: Vec<&String> = row.keys().filter(|k| columns.contains(*k)).collect();
    if keys.is_empty() {
        return Ok(());
    }
    let cols = keys.iter().map(|k| format!("\"{k}\"")).collect::<Vec<_>>().join(",");
    let marks = vec!["?"; keys.len()].join(",");
    let sql = format!("INSERT INTO \"{table}\" ({cols}) VALUES ({marks})");
    let mut q = sqlx::query(&sql);
    for k in &keys {
        q = match &row[*k] {
            Value::Null => q.bind(None::<String>),
            Value::String(s) => q.bind(s.clone()),
            Value::Bool(b) => q.bind(*b),
            Value::Number(n) => match n.as_i64() {
                Some(i) => q.bind(i),
                None => q.bind(n.as_f64()),
            },
            other => q.bind(other.to_string()),
        };
    }
    q.execute(&mut **tx).await.map_err(|e| format!("Errore nel ripristino ({table}): {e}"))?;
    Ok(())
}

async fn table_columns(pool: &SqlitePool, table: &str) -> Result<HashSet<String>, String> {
    let rows = sqlx::query(&format!("PRAGMA table_info(\"{table}\")"))
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Errore del database: {e}"))?;
    Ok(rows.iter().map(|r| r.get::<String, _>("name")).collect())
}

// ---------- comandi ----------

/// Elenco delle sedute con le dimensioni dell'audio, per la scelta nel pannello Backup.
#[tauri::command]
pub async fn backup_list_meetings(state: State<'_, AppState>) -> Result<Vec<BackupMeetingInfo>, String> {
    let pool = state.db_manager.pool();
    let rows = sqlx::query("SELECT id, title, created_at, folder_path, patient_id, session_date FROM meetings ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Errore del database: {e}"))?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let folder: Option<String> = row.try_get("folder_path").ok().flatten();
        let files = folder.as_deref().map(|f| audio_files_in(Path::new(f))).unwrap_or_default();
        out.push(BackupMeetingInfo {
            id: row.get("id"),
            title: row.get("title"),
            created_at: row.try_get::<String, _>("created_at").unwrap_or_default(),
            audio_files: files.len() as u32,
            audio_bytes: files.iter().map(|(_, n)| *n).sum(),
            patient_id: row.try_get::<Option<String>, _>("patient_id").ok().flatten(),
            session_date: row.try_get::<Option<String>, _>("session_date").ok().flatten(),
        });
    }
    Ok(out)
}

/// Elenco dei pazienti per la scelta nel pannello Backup (solo nome e conteggi, mai note o password).
#[tauri::command]
pub async fn backup_list_patients(state: State<'_, AppState>) -> Result<Vec<BackupPatientInfo>, String> {
    let rows = sqlx::query(
        "SELECT p.id, p.name, p.password_hash IS NOT NULL AS protected,
                (SELECT COUNT(*) FROM meetings m WHERE m.patient_id = p.id) AS n
         FROM patients p ORDER BY lower(p.name)",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|e| format!("Errore del database: {e}"))?;
    Ok(rows
        .iter()
        .map(|r| BackupPatientInfo {
            id: r.get("id"),
            name: r.get("name"),
            protected: r.get::<i64, _>("protected") != 0,
            session_count: r.get("n"),
        })
        .collect())
}

#[tauri::command]
pub async fn backup_pick_save_path<R: Runtime>(app: AppHandle<R>, suggested_name: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app
        .dialog()
        .file()
        .set_file_name(&suggested_name)
        .add_filter("Backup di Trascrivi Seduta", &[FILE_EXTENSION])
        .blocking_save_file();
    Ok(picked.map(|p| p.to_string()))
}

#[tauri::command]
pub async fn backup_pick_open_path<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app
        .dialog()
        .file()
        .add_filter("Backup di Trascrivi Seduta", &[FILE_EXTENSION])
        .blocking_pick_file();
    Ok(picked.map(|p| p.to_string()))
}

#[tauri::command]
pub async fn backup_create<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    lock: State<'_, AppLockState>,
    request: BackupRequest,
) -> Result<BackupResult, String> {
    // La password master è sempre richiesta: serve anche come chiave di cifratura.
    require_master_password(&app, &lock, &request.password).await?;

    let include = request.include;
    if !(include.transcripts || include.summaries || include.notes || include.audio) {
        return Err("Scegli almeno un tipo di contenuto da salvare.".to_string());
    }
    let pool = state.db_manager.pool().clone();
    emit(&app, "preparing", 0, 1);

    // sedute scelte
    let all: Vec<ChosenMeeting> =
        sqlx::query("SELECT id, title, created_at, folder_path, patient_id, session_date FROM meetings ORDER BY created_at")
            .fetch_all(&pool)
            .await
            .map_err(|e| format!("Errore del database: {e}"))?
            .iter()
            .map(|r| ChosenMeeting {
                id: r.get::<String, _>("id"),
                title: r.get::<String, _>("title"),
                created_at: r.try_get::<String, _>("created_at").unwrap_or_default(),
                folder: r.try_get::<Option<String>, _>("folder_path").ok().flatten(),
                patient_id: r.try_get::<Option<String>, _>("patient_id").ok().flatten(),
                session_date: r.try_get::<Option<String>, _>("session_date").ok().flatten(),
            })
            .collect();
    let chosen = filter_meetings(all, request.meeting_ids.as_deref(), request.patient_ids.as_deref());

    let all_patients: Vec<(String, String, bool)> = sqlx::query("SELECT id, name, password_hash IS NOT NULL AS protected FROM patients ORDER BY lower(name)")
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Errore del database: {e}"))?
        .iter()
        .map(|r| (r.get::<String, _>("id"), r.get::<String, _>("name"), r.get::<i64, _>("protected") != 0))
        .collect();
    let all_patient_ids: Vec<String> = all_patients.iter().map(|(id, ..)| id.clone()).collect();
    let referenced: Vec<Option<String>> = chosen.iter().map(|m| m.patient_id.clone()).collect();
    let patient_ids = select_patient_ids(
        &referenced,
        request.patient_ids.as_deref(),
        request.meeting_ids.is_none(),
        &all_patient_ids,
    );
    if chosen.is_empty() && patient_ids.is_empty() {
        return Err("Nessuna seduta da salvare.".to_string());
    }
    let manifest_patients: Vec<ManifestPatient> = all_patients
        .into_iter()
        .filter(|(id, ..)| patient_ids.contains(id))
        .map(|(id, name, protected)| ManifestPatient { id, name, protected })
        .collect();
    let ids: Vec<String> = chosen.iter().map(|m| m.id.clone()).collect();

    emit(&app, "database", 0, ids.len() as u64);
    let db_json = export_db(&pool, &ids, &patient_ids, &include).await?;

    let created_at = chrono::Utc::now().to_rfc3339();
    let version = env!("CARGO_PKG_VERSION").to_string();
    let destination = PathBuf::from(&request.destination);
    let password = request.password.clone();
    let app_for_task = app.clone();

    let outcome = tokio::task::spawn_blocking(move || -> Result<BackupResult, String> {
        let partial = destination.with_extension(format!("{FILE_EXTENSION}.partial"));
        let result = write_backup(&app_for_task, &partial, &password, &version, &created_at, &chosen, &manifest_patients, &db_json, include);
        match result {
            Ok(mut res) => {
                fs::rename(&partial, &destination).map_err(|e| format!("Impossibile salvare il backup: {e}"))?;
                res.path = destination.to_string_lossy().to_string();
                Ok(res)
            }
            Err(e) => {
                let _ = fs::remove_file(&partial);
                Err(e)
            }
        }
    })
    .await
    .map_err(|_| "Errore interno durante il backup.".to_string())??;

    emit(&app, "done", 1, 1);
    info!("Backup cifrato creato: {} sedute, {} file audio", outcome.meetings, outcome.audio_files);
    Ok(outcome)
}

struct ChosenMeeting {
    id: String,
    title: String,
    created_at: String,
    folder: Option<String>,
    patient_id: Option<String>,
    session_date: Option<String>,
}

/// Sedute da salvare: per id se indicati, altrimenti quelle dei pazienti indicati, altrimenti tutte.
fn filter_meetings(all: Vec<ChosenMeeting>, meeting_ids: Option<&[String]>, patient_ids: Option<&[String]>) -> Vec<ChosenMeeting> {
    all.into_iter()
        .filter(|m| match (meeting_ids, patient_ids) {
            (Some(ids), _) => ids.contains(&m.id),
            (None, Some(pids)) => m.patient_id.as_ref().map(|p| pids.contains(p)).unwrap_or(false),
            (None, None) => true,
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn write_backup<R: Runtime>(
    app: &AppHandle<R>,
    partial: &Path,
    password: &str,
    version: &str,
    created_at: &str,
    chosen: &[ChosenMeeting],
    manifest_patients: &[ManifestPatient],
    db_json: &Value,
    include: Include,
) -> Result<BackupResult, String> {
    let file = File::create(partial).map_err(|e| format!("Impossibile creare il file di backup: {e}"))?;
    let writer = EncryptWriter::create(BufWriter::new(file), password, version, created_at, DEFAULT_CHUNK)
        .map_err(|e| friendly(&e))?;
    let mut tar = tar::Builder::new(GzEncoder::new(writer, Compression::default()));

    // elenco audio
    let mut audio_plan: Vec<(String, PathBuf)> = Vec::new();
    let mut manifest_meetings = Vec::new();
    for m in chosen {
        let files = if include.audio {
            m.folder.as_deref().map(|f| audio_files_in(Path::new(f))).unwrap_or_default()
        } else {
            Vec::new()
        };
        manifest_meetings.push(ManifestMeeting {
            id: m.id.clone(),
            title: m.title.clone(),
            created_at: m.created_at.clone(),
            audio_files: files.len() as u32,
            patient_id: m.patient_id.clone(),
            session_date: m.session_date.clone(),
        });
        for (path, _) in files {
            audio_plan.push((m.id.clone(), path));
        }
    }

    let manifest = Manifest {
        format: 1,
        app_version: version.to_string(),
        created_at: created_at.to_string(),
        include,
        meetings: manifest_meetings,
        patients: manifest_patients.to_vec(),
    };
    append_bytes(&mut tar, "manifest.json", &serde_json::to_vec(&manifest).map_err(|e| e.to_string())?)?;
    append_bytes(&mut tar, "db.json", &serde_json::to_vec(db_json).map_err(|e| e.to_string())?)?;

    let total = audio_plan.len() as u64;
    for (i, (id, path)) in audio_plan.iter().enumerate() {
        emit(app, "audio", i as u64, total);
        let name = path.file_name().and_then(|n| n.to_str()).ok_or("Nome file non valido")?;
        tar.append_path_with_name(path, format!("audio/{id}/{name}"))
            .map_err(|e| format!("Impossibile aggiungere un file audio: {e}"))?;
    }
    emit(app, "finalizing", 0, 1);

    let gz = tar.into_inner().map_err(|e| format!("Errore nella creazione del backup: {e}"))?;
    let encrypted = gz.finish().map_err(|e| format!("Errore nella compressione: {e}"))?;
    let mut buffered = encrypted.finish().map_err(|e| format!("Errore nella cifratura: {e}"))?;
    std::io::Write::flush(&mut buffered).map_err(|e| e.to_string())?;
    let bytes = fs::metadata(partial).map(|m| m.len()).unwrap_or(0);

    Ok(BackupResult {
        path: String::new(),
        meetings: manifest.meetings.len(),
        audio_files: audio_plan.len(),
        bytes,
    })
}

fn append_bytes<W: std::io::Write>(tar: &mut tar::Builder<W>, name: &str, data: &[u8]) -> Result<(), String> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o600);
    header.set_mtime(0);
    header.set_cksum();
    tar.append_data(&mut header, name, data)
        .map_err(|e| format!("Errore nella creazione del backup: {e}"))
}

/// Apre il backup e restituisce l'elenco delle sedute (serve a scegliere cosa ripristinare).
#[tauri::command]
pub async fn backup_inspect(path: String, password: String) -> Result<Manifest, String> {
    tokio::task::spawn_blocking(move || read_manifest(Path::new(&path), &password))
        .await
        .map_err(|_| "Errore interno.".to_string())?
}

fn open_archive(path: &Path, password: &str) -> Result<tar::Archive<GzDecoder<DecryptReader<BufReader<File>>>>, String> {
    let file = File::open(path).map_err(|e| format!("Impossibile aprire il file: {e}"))?;
    let reader = DecryptReader::open(BufReader::new(file), password).map_err(|e| match e {
        CryptoError::Io(io) => io_message(&io),
        other => friendly(&other),
    })?;
    Ok(tar::Archive::new(GzDecoder::new(reader)))
}

fn read_manifest(path: &Path, password: &str) -> Result<Manifest, String> {
    let mut archive = open_archive(path, password)?;
    let entries = archive.entries().map_err(|e| io_message(&e))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| io_message(&e))?;
        let name = entry.path().map_err(|e| io_message(&e))?.to_string_lossy().to_string();
        if name == "manifest.json" {
            let mut buf = Vec::new();
            (&mut entry).take(MAX_JSON_BYTES).read_to_end(&mut buf).map_err(|e| io_message(&e))?;
            let manifest: Manifest = serde_json::from_slice(&buf).map_err(|_| "Il backup non è valido.".to_string())?;
            if manifest.format != 1 {
                return Err("Questo backup è stato creato da una versione più recente dell'app.".to_string());
            }
            return Ok(manifest);
        }
    }
    Err("Il backup non contiene l'elenco delle sedute.".to_string())
}

fn sanitize_component(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "seduta".to_string()
    } else {
        trimmed.chars().take(60).collect()
    }
}

fn valid_meeting_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[tauri::command]
pub async fn backup_restore<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    request: RestoreRequest,
) -> Result<RestoreResult, String> {
    let pool = state.db_manager.pool().clone();
    let save_folder = load_recording_preferences(&app)
        .await
        .map_err(|e| format!("Impossibile leggere la cartella delle registrazioni: {e}"))?
        .save_folder;

    // sedute già presenti: non vengono mai sovrascritte
    let existing: HashSet<String> = sqlx::query("SELECT id FROM meetings")
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Errore del database: {e}"))?
        .iter()
        .map(|r| r.get::<String, _>("id"))
        .collect();

    let path = PathBuf::from(&request.path);
    let password = request.password.clone();
    let wanted: Option<HashSet<String>> = request.meeting_ids.clone().map(|v| v.into_iter().collect());
    let app_for_task = app.clone();
    let existing_for_task = existing.clone();

    let extracted = tokio::task::spawn_blocking(move || {
        extract_backup(&app_for_task, &path, &password, &save_folder, wanted.as_ref(), &existing_for_task)
    })
    .await
    .map_err(|_| "Errore interno durante il ripristino.".to_string())??;

    // inserimento nel database in un'unica transazione
    let result = restore_into_db(&pool, &extracted, request.meeting_ids.is_none(), request.patient_ids.as_deref()).await;
    let patients_restored = match result {
        Ok(n) => n,
        Err(e) => {
            for folder in extracted.folders.values() {
                let _ = fs::remove_dir_all(folder);
            }
            return Err(e);
        }
    };
    emit(&app, "done", 1, 1);
    Ok(RestoreResult {
        restored: extracted.to_restore.len(),
        skipped_existing: extracted.skipped,
        audio_files: extracted.audio_files,
        patients_restored,
    })
}

struct Extracted {
    db: Map<String, Value>,
    to_restore: Vec<String>,
    folders: HashMap<String, PathBuf>,
    skipped: usize,
    audio_files: usize,
}

fn extract_backup<R: Runtime>(
    app: &AppHandle<R>,
    path: &Path,
    password: &str,
    save_folder: &Path,
    wanted: Option<&HashSet<String>>,
    existing: &HashSet<String>,
) -> Result<Extracted, String> {
    let mut archive = open_archive(path, password)?;
    let mut manifest: Option<Manifest> = None;
    let mut db: Option<Map<String, Value>> = None;
    let mut to_restore: Vec<String> = Vec::new();
    let mut titles: HashMap<String, String> = HashMap::new();
    let mut folders: HashMap<String, PathBuf> = HashMap::new();
    let mut skipped = 0usize;
    let mut audio_files = 0usize;

    let cleanup = |folders: &HashMap<String, PathBuf>| {
        for f in folders.values() {
            let _ = fs::remove_dir_all(f);
        }
    };

    let entries = archive.entries().map_err(|e| io_message(&e))?;
    for entry in entries {
        let mut entry = match entry {
            Ok(e) => e,
            Err(e) => {
                cleanup(&folders);
                return Err(io_message(&e));
            }
        };
        let name = entry.path().map_err(|e| io_message(&e))?.to_string_lossy().replace('\\', "/");

        let step = (|| -> Result<(), String> {
            match name.as_str() {
                "manifest.json" => {
                    let mut buf = Vec::new();
                    (&mut entry).take(MAX_JSON_BYTES).read_to_end(&mut buf).map_err(|e| io_message(&e))?;
                    let m: Manifest = serde_json::from_slice(&buf).map_err(|_| "Il backup non è valido.".to_string())?;
                    if m.format != 1 {
                        return Err("Questo backup è stato creato da una versione più recente dell'app.".to_string());
                    }
                    for mm in &m.meetings {
                        if !valid_meeting_id(&mm.id) {
                            return Err("Il backup contiene identificativi non validi.".to_string());
                        }
                        titles.insert(mm.id.clone(), mm.title.clone());
                        let chosen = wanted.map(|w| w.contains(&mm.id)).unwrap_or(true);
                        if chosen {
                            if existing.contains(&mm.id) {
                                skipped += 1;
                            } else {
                                to_restore.push(mm.id.clone());
                            }
                        }
                    }
                    manifest = Some(m);
                }
                "db.json" => {
                    if manifest.is_none() {
                        return Err("Il backup non è valido (manca l'elenco delle sedute).".to_string());
                    }
                    let mut buf = Vec::new();
                    (&mut entry).take(MAX_JSON_BYTES).read_to_end(&mut buf).map_err(|e| io_message(&e))?;
                    let v: Value = serde_json::from_slice(&buf).map_err(|_| "Il backup non è valido.".to_string())?;
                    db = v.as_object().cloned();
                }
                other if other.starts_with("audio/") => {
                    let parts: Vec<&str> = other.split('/').collect();
                    if parts.len() != 3 || !valid_meeting_id(parts[1]) || manifest.is_none() {
                        return Err("Il backup contiene un file non valido.".to_string());
                    }
                    let (id, file_name) = (parts[1], parts[2]);
                    let file_path = Path::new(file_name);
                    if file_name.starts_with('.') || !is_audio(file_path) || file_path.file_name().and_then(|n| n.to_str()) != Some(file_name) {
                        return Err("Il backup contiene un file non valido.".to_string());
                    }
                    if !to_restore.iter().any(|r| r == id) {
                        return Ok(()); // seduta non scelta oppure già presente: non si estrae
                    }
                    let folder = match folders.get(id) {
                        Some(f) => f.clone(),
                        None => {
                            let base = format!("{}_{}", sanitize_component(titles.get(id).map(|s| s.as_str()).unwrap_or("seduta")), &id[..id.len().min(8)]);
                            let mut candidate = save_folder.join(&base);
                            let mut n = 1;
                            while candidate.exists() {
                                candidate = save_folder.join(format!("{base}_{n}"));
                                n += 1;
                            }
                            fs::create_dir_all(&candidate).map_err(|e| format!("Impossibile creare la cartella: {e}"))?;
                            folders.insert(id.to_string(), candidate.clone());
                            candidate
                        }
                    };
                    let mut out = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(folder.join(file_name))
                        .map_err(|e| format!("Impossibile scrivere un file audio: {e}"))?;
                    std::io::copy(&mut entry, &mut out).map_err(|e| io_message(&e))?;
                    audio_files += 1;
                    emit(app, "audio", audio_files as u64, 0);
                }
                other => {
                    warn!("Elemento sconosciuto nel backup ignorato: {other}");
                }
            }
            Ok(())
        })();
        if let Err(e) = step {
            cleanup(&folders);
            return Err(e);
        }
    }

    match (manifest, db) {
        (Some(_), Some(db)) => Ok(Extracted { db, to_restore, folders, skipped, audio_files }),
        _ => {
            cleanup(&folders);
            Err("Il backup è incompleto.".to_string())
        }
    }
}

/// Pazienti da ripristinare: quelli delle sedute scelte, quelli richiesti, oppure tutti.
/// Mai quelli già presenti (non si sovrascrive nulla).
fn patients_to_restore(
    db: &Map<String, Value>,
    restore_ids: &HashSet<&String>,
    all: bool,
    requested: Option<&[String]>,
    existing: &HashSet<String>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |id: &str| {
        if valid_meeting_id(id) && !existing.contains(id) && !out.iter().any(|o| o == id) {
            out.push(id.to_string());
        }
    };
    let in_backup: Vec<&str> = match db.get("patients") {
        Some(Value::Array(rows)) => rows.iter().filter_map(|r| r.get("id").and_then(|v| v.as_str())).collect(),
        _ => Vec::new(),
    };
    if let Some(Value::Array(rows)) = db.get("meetings") {
        for r in rows {
            let mid = r.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if restore_ids.contains(&mid.to_string()) {
                if let Some(pid) = r.get("patient_id").and_then(|v| v.as_str()) {
                    if in_backup.contains(&pid) {
                        add(pid);
                    }
                }
            }
        }
    }
    match requested {
        Some(req) => req.iter().filter(|id| in_backup.contains(&id.as_str())).for_each(|id| add(id)),
        None if all => in_backup.iter().for_each(|id| add(id)),
        None => {}
    }
    out
}

async fn restore_into_db(
    pool: &SqlitePool,
    extracted: &Extracted,
    all: bool,
    requested_patients: Option<&[String]>,
) -> Result<usize, String> {
    let restore_ids: HashSet<&String> = extracted.to_restore.iter().collect();
    let existing_patients: HashSet<String> = sqlx::query("SELECT id FROM patients")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Errore del database: {e}"))?
        .iter()
        .map(|r| r.get::<String, _>("id"))
        .collect();
    let new_patients = patients_to_restore(&extracted.db, &restore_ids, all, requested_patients, &existing_patients);
    // pazienti noti dopo il ripristino: se una seduta punta a un paziente sconosciuto resta "Senza paziente"
    let known_patients: HashSet<String> = existing_patients.iter().cloned().chain(new_patients.iter().cloned()).collect();
    // colonne lette prima della transazione (il pool può avere una sola connessione)
    let mut table_cols: HashMap<&str, HashSet<String>> = HashMap::new();
    for table in ["patients", "meetings", "transcripts", "transcript_chunks", "summary_processes", "meeting_notes"] {
        table_cols.insert(table, table_columns(pool, table).await?);
    }
    let mut tx = pool.begin().await.map_err(|e| format!("Errore del database: {e}"))?;
    if let Some(Value::Array(rows)) = extracted.db.get("patients") {
        let columns = &table_cols["patients"];
        for row in rows {
            let Some(obj) = row.as_object() else { continue };
            let Some(id) = obj.get("id").and_then(|v| v.as_str()) else { continue };
            if new_patients.iter().any(|p| p == id) {
                insert_rows(&mut tx, "patients", columns, obj).await?;
            }
        }
    }
    for (table, key) in RESTORE_TABLES {
        let Some(Value::Array(rows)) = extracted.db.get(table) else { continue };
        let columns = &table_cols[table];
        for row in rows {
            let Some(obj) = row.as_object() else { continue };
            let Some(id) = obj.get(key).and_then(|v| v.as_str()) else { continue };
            if !restore_ids.contains(&id.to_string()) {
                continue;
            }
            let mut obj = obj.clone();
            if table == "meetings" {
                // la cartella audio originale può non esistere più: punta a quella ripristinata
                let new_folder = extracted.folders.get(id).map(|p| Value::from(p.to_string_lossy().to_string()));
                obj.insert("folder_path".into(), new_folder.unwrap_or(Value::Null));
                let keeps = obj.get("patient_id").and_then(|v| v.as_str()).map(|p| known_patients.contains(p)).unwrap_or(false);
                if !keeps {
                    obj.insert("patient_id".into(), Value::Null);
                }
            }
            insert_rows(&mut tx, table, columns, &obj).await?;
        }
    }
    tx.commit().await.map_err(|e| format!("Errore del database: {e}"))?;
    Ok(new_patients.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn meeting_ids_and_folder_names_are_sanitized() {
        assert!(valid_meeting_id("a1b2-c3_d4"));
        assert!(!valid_meeting_id("../x"));
        assert!(!valid_meeting_id("a/b"));
        assert!(!valid_meeting_id(""));
        assert_eq!(sanitize_component("Seduta: 12/03?"), "Seduta_ 12_03_");
        assert_eq!(sanitize_component("..."), "___");
        assert_eq!(sanitize_component("   "), "seduta");
        assert!(!sanitize_component("../../etc").contains('/'));
        assert!(is_audio(Path::new("audio.mp4")));
        assert!(!is_audio(Path::new("notes.txt")));
    }

    #[test]
    fn archive_roundtrip_through_encryption() {
        // più di un blocco da 1 MiB, per provare il passaggio tra blocchi
        let audio: Vec<u8> = (0..(DEFAULT_CHUNK as usize * 2 + 12345)).map(|i| (i * 31 % 251) as u8).collect();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prova.tsbackup");

        {
            let file = File::create(&path).unwrap();
            let writer = EncryptWriter::create(BufWriter::new(file), "password-di-prova", "0.0.0", "2026-01-01T00:00:00Z", DEFAULT_CHUNK).unwrap();
            let mut tar = tar::Builder::new(GzEncoder::new(writer, Compression::fast()));
            append_bytes(&mut tar, "manifest.json", br#"{"format":1}"#).unwrap();
            append_bytes(&mut tar, "audio/abc/audio.mp4", &audio).unwrap();
            let mut buffered = tar.into_inner().unwrap().finish().unwrap().finish().unwrap();
            buffered.flush().unwrap();
        }

        // il contenuto non deve comparire in chiaro
        let raw = fs::read(&path).unwrap();
        assert!(!raw.windows(8).any(|w| w == b"manifest"));

        let mut archive = open_archive(&path, "password-di-prova").unwrap();
        let mut seen = Vec::new();
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let name = entry.path().unwrap().to_string_lossy().to_string();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            seen.push((name, data));
        }
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].0, "manifest.json");
        assert_eq!(seen[1].1, audio);

        assert!(open_archive(&path, "password-sbagliata").is_ok(), "l'apertura controlla solo l'intestazione");
        let mut wrong = open_archive(&path, "password-sbagliata").unwrap();
        let first = wrong.entries().unwrap().next();
        assert!(first.is_none() || first.unwrap().is_err(), "con la password errata la lettura deve fallire");
    }

    fn chosen(id: &str, patient: Option<&str>) -> ChosenMeeting {
        ChosenMeeting {
            id: id.into(),
            title: id.into(),
            created_at: String::new(),
            folder: None,
            patient_id: patient.map(String::from),
            session_date: None,
        }
    }

    #[test]
    fn selection_modes_for_meetings_and_patients() {
        let all = || vec![chosen("m1", Some("p1")), chosen("m2", Some("p2")), chosen("m3", None)];
        assert_eq!(filter_meetings(all(), None, None).len(), 3, "tutti");
        let by_patient = filter_meetings(all(), None, Some(&["p2".to_string()]));
        assert_eq!(by_patient.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["m2"], "solo alcuni pazienti");
        let by_meeting = filter_meetings(all(), Some(&["m3".to_string(), "m1".to_string()]), None);
        assert_eq!(by_meeting.len(), 2, "solo alcune sedute");

        let all_p = vec!["p1".to_string(), "p2".to_string(), "p3".to_string()];
        // tutti: anche i pazienti senza sedute
        assert_eq!(select_patient_ids(&[], None, true, &all_p).len(), 3);
        // solo alcune sedute: solo i pazienti di quelle sedute
        assert_eq!(select_patient_ids(&[Some("p1".into()), None], None, false, &all_p), ["p1"]);
        // pazienti richiesti (anche vuoti), id sconosciuti ignorati
        let got = select_patient_ids(&[Some("p1".into())], Some(&["p3".to_string(), "zz".to_string()]), false, &all_p);
        assert_eq!(got, ["p1", "p3"]);
    }

    async fn test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    fn backup_db() -> Map<String, Value> {
        let v = serde_json::json!({
            "patients": [
                {"id":"p-new","name":"Nuovo","notes":null,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","password_hash":"$argon2id$x","archived":0},
                {"id":"p-old","name":"DaBackup","notes":null,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","password_hash":null,"archived":0},
                {"id":"p-empty","name":"Vuoto","notes":null,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","password_hash":null,"archived":0}
            ],
            "meetings": [
                {"id":"m-new","title":"A","created_at":"2026-01-02T10:00:00Z","updated_at":"2026-01-02T10:00:00Z","folder_path":"/vecchio","patient_id":"p-new","session_date":"2026-01-02"},
                {"id":"m-old","title":"B","created_at":"2026-01-03T10:00:00Z","updated_at":"2026-01-03T10:00:00Z","folder_path":null,"patient_id":"p-old","session_date":"2026-01-03"},
                {"id":"m-orph","title":"C","created_at":"2026-01-04T10:00:00Z","updated_at":"2026-01-04T10:00:00Z","folder_path":null,"patient_id":"p-ghost","session_date":"2026-01-04"}
            ]
        });
        v.as_object().unwrap().clone()
    }

    #[tokio::test]
    async fn restore_adds_patients_without_overwriting() {
        let pool = test_pool().await;
        // paziente già presente con lo stesso id ma dati diversi: non va toccato
        sqlx::query("INSERT INTO patients (id,name,created_at,updated_at,archived) VALUES ('p-old','Locale','2026-02-01T00:00:00Z','2026-02-01T00:00:00Z',0)")
            .execute(&pool).await.unwrap();
        let extracted = Extracted {
            db: backup_db(),
            to_restore: vec!["m-new".into(), "m-old".into(), "m-orph".into()],
            folders: HashMap::new(),
            skipped: 0,
            audio_files: 0,
        };
        // modalità "solo alcune sedute": nessun paziente richiesto, quelli referenziati si ripristinano
        let n = restore_into_db(&pool, &extracted, false, None).await.unwrap();
        assert_eq!(n, 1, "solo p-new è nuovo; p-old esiste già; p-empty non è referenziato");

        let name: String = sqlx::query_scalar("SELECT name FROM patients WHERE id='p-old'").fetch_one(&pool).await.unwrap();
        assert_eq!(name, "Locale", "il paziente esistente non viene sovrascritto");
        let hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM patients WHERE id='p-new'").fetch_one(&pool).await.unwrap();
        assert_eq!(hash.as_deref(), Some("$argon2id$x"), "l'hash della password viaggia nel backup");
        let (p, d): (Option<String>, Option<String>) = sqlx::query_as("SELECT patient_id, session_date FROM meetings WHERE id='m-new'").fetch_one(&pool).await.unwrap();
        assert_eq!((p.as_deref(), d.as_deref()), (Some("p-new"), Some("2026-01-02")));
        let orphan: Option<String> = sqlx::query_scalar("SELECT patient_id FROM meetings WHERE id='m-orph'").fetch_one(&pool).await.unwrap();
        assert_eq!(orphan, None, "paziente sconosciuto: la seduta torna in \"Senza paziente\"");
        let patients: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM patients").fetch_one(&pool).await.unwrap();
        assert_eq!(patients, 2);
    }

    #[test]
    fn patients_to_restore_modes() {
        let db = backup_db();
        let ids = vec!["m-new".to_string()];
        let restore: HashSet<&String> = ids.iter().collect();
        let none = HashSet::new();
        assert_eq!(patients_to_restore(&db, &restore, false, None, &none), ["p-new"]);
        assert_eq!(patients_to_restore(&db, &restore, true, None, &none).len(), 3, "tutti");
        let req = ["p-empty".to_string()];
        assert_eq!(patients_to_restore(&db, &restore, false, Some(&req), &none), ["p-new", "p-empty"]);
        let existing: HashSet<String> = ["p-new".to_string()].into();
        assert_eq!(patients_to_restore(&db, &restore, false, None, &existing).len(), 0);
    }
}
