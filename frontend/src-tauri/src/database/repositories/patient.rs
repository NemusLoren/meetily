//! Repository dei pazienti e delle loro cartelle (giorni di seduta).
//!
//! Una seduta appartiene a un paziente tramite `meetings.patient_id` (NULL = "Senza paziente")
//! e a un giorno tramite `meetings.session_date` (testo ISO `AAAA-MM-GG`).

use chrono::{NaiveDate, Utc};
use serde::Serialize;
use sqlx::{Error as SqlxError, FromRow, SqlitePool};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

pub const MAX_NAME_CHARS: usize = 120;
pub const MAX_NOTES_CHARS: usize = 2000;

#[derive(Debug, Clone, FromRow)]
pub struct PatientRow {
    pub id: String,
    pub name: String,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub password_hash: Option<String>,
    pub archived: bool,
}

#[derive(Debug, Clone, FromRow)]
struct SessionRow {
    id: String,
    title: String,
    created_at: String,
    patient_id: Option<String>,
    day: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionItem {
    pub id: String,
    pub title: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionDay {
    /// Giorno ISO `AAAA-MM-GG`
    pub date: String,
    pub sessions: Vec<SessionItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatientFolder {
    pub id: String,
    pub name: String,
    pub notes: Option<String>,
    pub archived: bool,
    /// Ha una password propria.
    pub protected: bool,
    /// Protetta e non ancora sbloccata in questa sessione: giorni e sedute non vengono restituiti.
    pub locked: bool,
    pub session_count: usize,
    pub days: Vec<SessionDay>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatientsTree {
    pub patients: Vec<PatientFolder>,
    /// Voce virtuale "Senza paziente": sedute con `patient_id` NULL.
    pub unassigned: Vec<SessionDay>,
    pub unassigned_count: usize,
}

pub fn validate_name(name: &str) -> Result<String, String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err("Il nome del paziente non può essere vuoto.".to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err("Il nome del paziente è troppo lungo.".to_string());
    }
    Ok(name)
}

pub fn validate_notes(notes: Option<String>) -> Result<Option<String>, String> {
    match notes.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
        Some(n) if n.chars().count() > MAX_NOTES_CHARS => {
            Err("Le note sono troppo lunghe.".to_string())
        }
        other => Ok(other),
    }
}

/// Accetta solo giorni ISO validi (`AAAA-MM-GG`).
pub fn validate_session_date(date: &str) -> Result<String, String> {
    let d = date.trim();
    NaiveDate::parse_from_str(d, "%Y-%m-%d")
        .ok()
        .filter(|parsed| parsed.format("%Y-%m-%d").to_string() == d)
        .map(|_| d.to_string())
        .ok_or_else(|| "La data della seduta non è valida (usa AAAA-MM-GG).".to_string())
}

/// Raggruppa le sedute per giorno: giorni dal più recente, sedute in ordine cronologico nel giorno.
fn group_by_day(rows: Vec<SessionRow>) -> Vec<SessionDay> {
    let mut map: BTreeMap<String, Vec<SessionItem>> = BTreeMap::new();
    for r in rows {
        map.entry(r.day).or_default().push(SessionItem {
            id: r.id,
            title: r.title,
            created_at: r.created_at,
        });
    }
    map.into_iter()
        .rev()
        .map(|(date, mut sessions)| {
            sessions.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
            SessionDay { date, sessions }
        })
        .collect()
}

pub struct PatientsRepository;

impl PatientsRepository {
    pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<PatientRow>, SqlxError> {
        sqlx::query_as::<_, PatientRow>("SELECT * FROM patients WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    async fn name_taken(pool: &SqlitePool, name: &str, except_id: Option<&str>) -> Result<bool, SqlxError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM patients WHERE lower(name) = lower(?) AND archived = 0 AND id != ? LIMIT 1",
        )
        .bind(name)
        .bind(except_id.unwrap_or(""))
        .fetch_optional(pool)
        .await?;
        Ok(row.is_some())
    }

    pub async fn create(pool: &SqlitePool, name: &str, notes: Option<String>) -> Result<PatientRow, String> {
        let name = validate_name(name)?;
        let notes = validate_notes(notes)?;
        if Self::name_taken(pool, &name, None).await.map_err(db_err)? {
            return Err("Esiste già un paziente con questo nome.".to_string());
        }
        let id = format!("patient-{}", Uuid::new_v4());
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO patients (id, name, notes, created_at, updated_at, password_hash, archived)
             VALUES (?, ?, ?, ?, ?, NULL, 0)",
        )
        .bind(&id)
        .bind(&name)
        .bind(&notes)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(db_err)?;
        Ok(PatientRow {
            id,
            name,
            notes,
            created_at: now.clone(),
            updated_at: now,
            password_hash: None,
            archived: false,
        })
    }

    pub async fn update(
        pool: &SqlitePool,
        id: &str,
        name: &str,
        notes: Option<String>,
    ) -> Result<(), String> {
        let name = validate_name(name)?;
        let notes = validate_notes(notes)?;
        if Self::name_taken(pool, &name, Some(id)).await.map_err(db_err)? {
            return Err("Esiste già un paziente con questo nome.".to_string());
        }
        let res = sqlx::query("UPDATE patients SET name = ?, notes = ?, updated_at = ? WHERE id = ?")
            .bind(&name)
            .bind(&notes)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(pool)
            .await
            .map_err(db_err)?;
        if res.rows_affected() == 0 {
            return Err("Paziente non trovato.".to_string());
        }
        Ok(())
    }

    pub async fn set_archived(pool: &SqlitePool, id: &str, archived: bool) -> Result<(), String> {
        let res = sqlx::query("UPDATE patients SET archived = ?, updated_at = ? WHERE id = ?")
            .bind(archived)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(pool)
            .await
            .map_err(db_err)?;
        if res.rows_affected() == 0 {
            return Err("Paziente non trovato.".to_string());
        }
        Ok(())
    }

    /// Elimina la cartella del paziente. Comportamento sicuro: le sedute NON vengono cancellate,
    /// tornano in "Senza paziente". Restituisce quante sedute sono state spostate.
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<usize, String> {
        let mut tx = pool.begin().await.map_err(db_err)?;
        let moved = sqlx::query("UPDATE meetings SET patient_id = NULL WHERE patient_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected() as usize;
        let res = sqlx::query("DELETE FROM patients WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        if res.rows_affected() == 0 {
            return Err("Paziente non trovato.".to_string());
        }
        tx.commit().await.map_err(db_err)?;
        Ok(moved)
    }

    pub async fn set_password_hash(pool: &SqlitePool, id: &str, hash: Option<&str>) -> Result<(), String> {
        let res = sqlx::query("UPDATE patients SET password_hash = ?, updated_at = ? WHERE id = ?")
            .bind(hash)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(pool)
            .await
            .map_err(db_err)?;
        if res.rows_affected() == 0 {
            return Err("Paziente non trovato.".to_string());
        }
        Ok(())
    }

    /// Assegna una seduta a un paziente (o a nessuno) e a un giorno.
    /// `session_date = None` lascia il giorno com'è.
    pub async fn assign_meeting(
        pool: &SqlitePool,
        meeting_id: &str,
        patient_id: Option<&str>,
        session_date: Option<&str>,
    ) -> Result<(), String> {
        let date = session_date.map(validate_session_date).transpose()?;
        if let Some(pid) = patient_id {
            if Self::get(pool, pid).await.map_err(db_err)?.is_none() {
                return Err("Paziente non trovato.".to_string());
            }
        }
        let res = sqlx::query(
            "UPDATE meetings SET patient_id = ?, session_date = COALESCE(?, session_date, substr(created_at, 1, 10))
             WHERE id = ?",
        )
        .bind(patient_id)
        .bind(date)
        .bind(meeting_id)
        .execute(pool)
        .await
        .map_err(db_err)?;
        if res.rows_affected() == 0 {
            return Err("Seduta non trovata.".to_string());
        }
        Ok(())
    }

    /// Paziente a cui appartiene una seduta, se ne ha uno.
    pub async fn patient_of_meeting(pool: &SqlitePool, meeting_id: &str) -> Result<Option<String>, SqlxError> {
        let row: Option<(Option<String>,)> = sqlx::query_as("SELECT patient_id FROM meetings WHERE id = ?")
            .bind(meeting_id)
            .fetch_optional(pool)
            .await?;
        Ok(row.and_then(|r| r.0))
    }

    /// Albero pazienti > giorni > sedute. Le cartelle protette non sbloccate non mostrano il contenuto.
    pub async fn tree(
        pool: &SqlitePool,
        unlocked: &HashSet<String>,
        include_archived: bool,
    ) -> Result<PatientsTree, String> {
        let patients = sqlx::query_as::<_, PatientRow>(
            "SELECT * FROM patients WHERE (? OR archived = 0) ORDER BY lower(name)",
        )
        .bind(include_archived)
        .fetch_all(pool)
        .await
        .map_err(db_err)?;

        let sessions = sqlx::query_as::<_, SessionRow>(
            "SELECT id, title, created_at, patient_id,
                    COALESCE(session_date, substr(created_at, 1, 10)) AS day
             FROM meetings",
        )
        .fetch_all(pool)
        .await
        .map_err(db_err)?;

        let mut per_patient: BTreeMap<String, Vec<SessionRow>> = BTreeMap::new();
        let mut unassigned = Vec::new();
        for s in sessions {
            match s.patient_id.clone() {
                Some(pid) => per_patient.entry(pid).or_default().push(s),
                None => unassigned.push(s),
            }
        }

        let folders = patients
            .into_iter()
            .map(|p| {
                let rows = per_patient.remove(&p.id).unwrap_or_default();
                let session_count = rows.len();
                let protected = p.password_hash.is_some();
                let locked = protected && !unlocked.contains(&p.id);
                PatientFolder {
                    notes: if locked { None } else { p.notes },
                    days: if locked { Vec::new() } else { group_by_day(rows) },
                    id: p.id,
                    name: p.name,
                    archived: p.archived,
                    protected,
                    locked,
                    session_count,
                }
            })
            .collect();

        Ok(PatientsTree {
            unassigned_count: unassigned.len(),
            unassigned: group_by_day(unassigned),
            patients: folders,
        })
    }
}

fn db_err(e: SqlxError) -> String {
    log::error!("Errore del database (pazienti): {e}");
    "Errore del database.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn add_meeting(pool: &SqlitePool, id: &str, created: &str) {
        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(format!("Seduta {id}"))
            .bind(created)
            .bind(created)
            .execute(pool)
            .await
            .unwrap();
    }

    #[test]
    fn date_and_name_validation() {
        assert!(validate_session_date("2026-03-09").is_ok());
        assert!(validate_session_date("2026-02-30").is_err());
        assert!(validate_session_date("9/3/2026").is_err());
        assert!(validate_session_date("2026-3-9").is_err());
        assert_eq!(validate_name("  Mario   R. ").unwrap(), "Mario R.");
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"a".repeat(121)).is_err());
        assert_eq!(validate_notes(Some("  ".into())).unwrap(), None);
    }

    #[test]
    fn grouping_orders_days_and_sessions() {
        let row = |id: &str, created: &str, day: &str| SessionRow {
            id: id.into(),
            title: id.into(),
            created_at: created.into(),
            patient_id: None,
            day: day.into(),
        };
        let days = group_by_day(vec![
            row("b", "2026-03-02T15:00:00Z", "2026-03-02"),
            row("a", "2026-03-02T09:00:00Z", "2026-03-02"),
            row("c", "2026-03-10T09:00:00Z", "2026-03-10"),
        ]);
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].date, "2026-03-10");
        let ids: Vec<_> = days[1].sessions.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
    }

    #[tokio::test]
    async fn existing_meetings_stay_valid_and_unassigned() {
        let pool = pool().await;
        add_meeting(&pool, "m1", "2026-01-05T10:00:00+00:00").await;
        let tree = PatientsRepository::tree(&pool, &HashSet::new(), false).await.unwrap();
        assert!(tree.patients.is_empty());
        assert_eq!(tree.unassigned_count, 1);
        assert_eq!(tree.unassigned[0].date, "2026-01-05");
    }

    #[tokio::test]
    async fn create_assign_group_rename_archive() {
        let pool = pool().await;
        let p = PatientsRepository::create(&pool, "Anna B.", Some("note".into())).await.unwrap();
        assert!(PatientsRepository::create(&pool, "anna b.", None).await.is_err());
        add_meeting(&pool, "m1", "2026-01-05T10:00:00Z").await;
        add_meeting(&pool, "m2", "2026-01-05T17:00:00Z").await;
        add_meeting(&pool, "m3", "2026-01-12T10:00:00Z").await;
        for id in ["m1", "m2", "m3"] {
            PatientsRepository::assign_meeting(&pool, id, Some(&p.id), None).await.unwrap();
        }
        // la seduta m3 viene spostata a un altro giorno
        PatientsRepository::assign_meeting(&pool, "m3", Some(&p.id), Some("2026-02-01")).await.unwrap();
        assert!(PatientsRepository::assign_meeting(&pool, "m3", Some(&p.id), Some("boh")).await.is_err());
        assert!(PatientsRepository::assign_meeting(&pool, "m3", Some("patient-x"), None).await.is_err());
        assert!(PatientsRepository::assign_meeting(&pool, "nope", Some(&p.id), None).await.is_err());

        let tree = PatientsRepository::tree(&pool, &HashSet::new(), false).await.unwrap();
        let f = &tree.patients[0];
        assert_eq!(f.session_count, 3);
        assert_eq!(f.days.len(), 2);
        assert_eq!(f.days[0].date, "2026-02-01");
        assert_eq!(f.days[1].sessions.len(), 2, "due sedute nello stesso giorno, stessa sottocartella");
        assert_eq!(tree.unassigned_count, 0);

        PatientsRepository::update(&pool, &p.id, "Anna Bianchi", None).await.unwrap();
        PatientsRepository::set_archived(&pool, &p.id, true).await.unwrap();
        assert!(PatientsRepository::tree(&pool, &HashSet::new(), false).await.unwrap().patients.is_empty());
        assert_eq!(PatientsRepository::tree(&pool, &HashSet::new(), true).await.unwrap().patients[0].name, "Anna Bianchi");

        // spostamento in "Senza paziente"
        PatientsRepository::assign_meeting(&pool, "m1", None, None).await.unwrap();
        assert_eq!(PatientsRepository::patient_of_meeting(&pool, "m1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn delete_keeps_sessions() {
        let pool = pool().await;
        let p = PatientsRepository::create(&pool, "Luca", None).await.unwrap();
        add_meeting(&pool, "m1", "2026-01-05T10:00:00Z").await;
        PatientsRepository::assign_meeting(&pool, "m1", Some(&p.id), None).await.unwrap();
        assert_eq!(PatientsRepository::delete(&pool, &p.id).await.unwrap(), 1);
        let tree = PatientsRepository::tree(&pool, &HashSet::new(), true).await.unwrap();
        assert!(tree.patients.is_empty());
        assert_eq!(tree.unassigned_count, 1);
        assert!(PatientsRepository::delete(&pool, &p.id).await.is_err());
    }

    #[tokio::test]
    async fn locked_folder_hides_contents_until_unlocked() {
        let pool = pool().await;
        let p = PatientsRepository::create(&pool, "Chiara", Some("riservato".into())).await.unwrap();
        add_meeting(&pool, "m1", "2026-01-05T10:00:00Z").await;
        PatientsRepository::assign_meeting(&pool, "m1", Some(&p.id), None).await.unwrap();
        PatientsRepository::set_password_hash(&pool, &p.id, Some("hash")).await.unwrap();

        let locked = PatientsRepository::tree(&pool, &HashSet::new(), false).await.unwrap();
        let f = &locked.patients[0];
        assert!(f.protected && f.locked);
        assert!(f.days.is_empty() && f.notes.is_none());
        assert_eq!(f.session_count, 1);

        let open: HashSet<String> = [p.id.clone()].into();
        let tree = PatientsRepository::tree(&pool, &open, false).await.unwrap();
        assert!(!tree.patients[0].locked);
        assert_eq!(tree.patients[0].days.len(), 1);

        PatientsRepository::set_password_hash(&pool, &p.id, None).await.unwrap();
        assert!(!PatientsRepository::tree(&pool, &HashSet::new(), false).await.unwrap().patients[0].protected);
    }
}
