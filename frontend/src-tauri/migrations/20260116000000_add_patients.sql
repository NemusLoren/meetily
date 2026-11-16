-- Cartelle per paziente.
-- Ogni paziente ha una cartella; dentro, le sedute sono raggruppate per giorno (session_date).
-- NOTA: password_hash è un blocco di INTERFACCIA (Argon2id), non cifra i dati.
-- Le sedute esistenti restano valide: patient_id NULL = voce virtuale "Senza paziente".
CREATE TABLE IF NOT EXISTS patients (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    password_hash TEXT,
    archived INTEGER NOT NULL DEFAULT 0
);

ALTER TABLE meetings ADD COLUMN patient_id TEXT REFERENCES patients(id) ON DELETE SET NULL;
ALTER TABLE meetings ADD COLUMN session_date TEXT;

-- Le sedute già presenti prendono come giorno la data di creazione (formato ISO AAAA-MM-GG).
UPDATE meetings SET session_date = substr(created_at, 1, 10) WHERE session_date IS NULL;

CREATE INDEX IF NOT EXISTS idx_meetings_patient_id ON meetings(patient_id);
CREATE INDEX IF NOT EXISTS idx_meetings_session_date ON meetings(session_date);
