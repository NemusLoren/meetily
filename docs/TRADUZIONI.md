# Tradurre Trascrivi Seduta

L'interfaccia usa file **`.po`** (gettext), modificabili con [Poedit](https://poedit.net) o con qualsiasi editor di testo.
La lingua predefinita è l'**italiano**; l'utente sceglie un'altra lingua in *Impostazioni → Preferenze → Lingua dell'interfaccia*.

## Dove stanno i file

`frontend/src/i18n/locales/`

| File | Contenuto |
| --- | --- |
| `messages.pot` | Modello con tutti i testi (si rigenera da solo) |
| `it.po` | Traduzione italiana |
| `en.po` | Inglese: il testo originale, non serve tradurre |

Il testo originale (inglese) è la chiave `msgid`; la traduzione è `msgstr`. I segnaposto come `{nome}` vanno lasciati identici.

## Correggere una frase

1. Apri `it.po` e cerca il testo.
2. Modifica `msgstr`, salva.
3. Dalla cartella `frontend/` controlla: `pnpm run i18n:check`

## Aggiungere una lingua

1. Copia `messages.pot` in `locales/<codice>.po` (per esempio `fr.po`) e traduci.
2. In `frontend/src/i18n/index.ts` importa il file e aggiungilo a `LOCALES`.
3. `pnpm run i18n:check` segnala testi mancanti o segnaposto sbagliati.

## Per chi sviluppa

- Nei componenti: `import { t } from '@/i18n'` e poi `t("Testo in inglese")`, oppure `t("Ciao {nome}", { nome })`.
- Dopo aver aggiunto o cambiato testi: `pnpm run i18n:extract` aggiorna `messages.pot` e aggiunge le voci mancanti ai `.po`.
- Non usare frasi spezzate in più `t()`: l'ordine delle parole cambia da lingua a lingua.
- Restano da tradurre i messaggi che arrivano dal nucleo nativo (Rust), come alcune notifiche ed errori.
