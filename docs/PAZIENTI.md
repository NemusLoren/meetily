# Cartelle per paziente

Ogni **paziente** ha una cartella; dentro, le sedute sono raggruppate in **sottocartelle per giorno**. Più sedute dello stesso giorno stanno nella stessa sottocartella.

```
Paziente
└── lunedì 9 marzo 2026
    ├── Seduta del mattino
    └── Seduta del pomeriggio
```

Le sedute non assegnate (per esempio quelle registrate prima di questa funzione) stanno nella voce **«Senza paziente»**: restano valide e si possono spostare in qualsiasi momento.

> ⚠️ **Le cartelle sono logiche**, non cartelle sul disco: la struttura vive nel database dell'app. I file audio restano dove li salva già la registrazione.

## Dove si trova

- **Barra laterale**: albero pazienti → giorni → sedute, comprimibile, con ricerca (nome del paziente, titolo della seduta, giorno, testo delle trascrizioni).
- **Pazienti e sedute** (pulsante nella barra laterale): la stessa vista a tutta pagina.

## Registrare una seduta

Sopra i comandi di registrazione trovi il selettore **Paziente + giorno**:

- scegli un paziente esistente oppure **Nuovo paziente** per crearlo al volo;
- il giorno è oggi, ma puoi cambiarlo;
- la scelta vale per **una sola seduta**: dopo il salvataggio si azzera, così la seduta successiva non finisce per errore nella cartella sbagliata.

Se non scegli nessun paziente, la seduta va in «Senza paziente». Se la cartella scelta risulta chiusa o non esiste più al momento del salvataggio, **la seduta viene salvata comunque** in «Senza paziente» e l'app te lo segnala: non si perde mai una registrazione.

## Gestire i pazienti

Passa il mouse su un paziente per vedere le azioni: rinomina (nome o sigla, note), password, blocca adesso, archivia, elimina. Sulle sedute: sposta in un altro paziente e/o in un altro giorno.

- **Archiviare** nasconde il paziente dall'elenco (si rivede con «Mostra archiviati»); non cancella nulla.
- **Eliminare** la cartella chiede conferma e **non cancella le sedute**: passano a «Senza paziente». Una cartella protetta va prima sbloccata.

## Password della cartella (facoltativa)

Ogni cartella può avere una password propria (almeno 8 caratteri, doppia conferma). Una cartella protetta resta **chiusa**: l'app non mostra né restituisce giorni, sedute e note finché non la sblocchi nella sessione corrente. Si richiude alla chiusura dell'app o con «Blocca la cartella adesso».

- Dopo alcuni errori consecutivi i tentativi vengono rallentati (come per la password master).
- Per cambiare o togliere la password serve quella attuale.
- **Non c'è un recupero**: se dimentichi la password di una cartella, non puoi aprirla dall'app.

> ⚠️ **È un blocco di interfaccia, non una cifratura.** I dati sul disco (database e audio) non sono ancora cifrati: chi accede direttamente ai file può leggerli. La cifratura vera è prevista in una fase successiva. Nel frattempo attiva FileVault (Mac) o BitLocker (Windows).

## Backup

Il [backup](BACKUP.md) può includere tutto, solo alcuni pazienti o solo alcune sedute, e salva anche pazienti, giorni e hash delle password. Il ripristino non sovrascrive mai ciò che è già presente.

## Note tecniche

- Tabella `patients` (`id`, `name`, `notes`, `created_at`, `updated_at`, `password_hash` Argon2id, `archived`) e colonne `meetings.patient_id` (NULL = «Senza paziente») e `meetings.session_date` (testo ISO `AAAA-MM-GG`). Migrazione: `src-tauri/migrations/20260116000000_add_patients.sql`. Per le sedute esistenti il giorno è ricavato dalla data di creazione (UTC).
- Codice: `src-tauri/src/patients.rs` (comandi e stato di sblocco), `src-tauri/src/database/repositories/patient.rs` (repository), `src/components/Patients/`, `src/services/patientService.ts`.
