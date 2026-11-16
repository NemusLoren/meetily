# Workflow di Trascrivi Seduta

| File | Quando parte | Cosa fa |
| --- | --- | --- |
| `ci.yml` | su ogni pull request verso `main` | controlla i tipi TypeScript dell'interfaccia |
| `build-test.yml` | a mano | compila macOS e Windows, senza pubblicare; gli installer sono scaricabili dalla run |
| `release.yml` | a mano | crea una **bozza** di release con gli installer di macOS e Windows |
| `build.yml` | richiamato dagli altri | passi di compilazione condivisi |

Gli installer **non sono firmati**: all'apertura macOS e Windows mostrano un avviso, spiegato in `docs/INSTALLAZIONE.md`.
Gli aggiornamenti automatici non ci sono: ogni nuova versione va scaricata e installata a mano.

Per pubblicare: aumenta `version` in `frontend/src-tauri/tauri.conf.json`, lancia **Rilascio** da *Actions*, controlla la bozza in *Releases* e pubblicala.
