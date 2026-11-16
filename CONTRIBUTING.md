# Contribuire a Trascrivi Seduta

Grazie! Il progetto è pensato per i colleghi psicologi e psicoterapeuti e migliora con il loro contributo. Non serve saper programmare: segnalazioni, idee, correzioni di testo e traduzioni sono preziose.

## Privacy prima di tutto

**Non allegare né incollare mai** registrazioni, trascrizioni, nomi, dati o log non ripuliti di pazienti, nemmeno in schermate. Usa esempi inventati.

## Modi per partecipare

- **Segnala un problema o proponi un'idea**: apri una [issue](https://github.com/NemusLoren/meetily/issues/new/choose) oppure scrivi a **info@leandromancino.it**.
- **Correggi o traduci un testo**: usa il modello "Traduzione o testo da correggere". Quando saranno pronti i file `.po`, potrai proporre le traduzioni direttamente lì.
- **Scrivi codice**: continua qui sotto.

## Per chi sviluppa

L'app vive in `frontend/` (interfaccia Next.js/React e nucleo Rust/Tauri). `backend/` è un archivio storico non supportato. Leggi [AGENTS.md](AGENTS.md) e [docs/BUILDING.md](docs/BUILDING.md).

1. Fai un fork e clona il repository.
2. Crea un ramo da `main`: `git checkout -b feature/nome-della-modifica`.
3. Tieni la PR concentrata su un solo problema.
4. Controlla ciò che hai toccato: `pnpm exec tsc --noEmit` in `frontend/` e, per il codice Rust, i test del sottosistema interessato.
5. Apri la pull request verso `main` e compila il modello: indica sistema, passi e risultati realmente verificati.

Se usi un assistente di programmazione, fagli leggere `AGENTS.md`: sei tu il responsabile di ciò che invii.

## Regole del progetto

- Gestione dei dati: tutto locale, nessuna chiamata a servizi esterni senza una scelta esplicita e informata dell'utente.
- I testi dell'interfaccia vanno in italiano (e, a regime, nei file di traduzione).
- Per modifiche grandi, apri prima una issue per discuterne.

Licenza: contribuendo accetti che il tuo lavoro sia rilasciato con la licenza MIT del progetto.
