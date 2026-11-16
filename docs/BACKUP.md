# Backup cifrato

In **Impostazioni → Backup** puoi salvare una copia cifrata dei tuoi dati e ripristinarla quando serve.

## Cosa puoi salvare

- **Tipi di contenuto** (scegli quali): trascrizioni, riassunti, appunti, registrazioni audio.
- **Quali pazienti e sedute**: tutto; **solo alcuni pazienti** (con tutte le loro sedute, compresa la voce «Senza paziente»); oppure **solo alcune sedute** scelte dall'elenco.
- Il backup contiene anche l'elenco dei pazienti (nome o sigla, note, archiviato), il paziente e il giorno di ogni seduta e, per le cartelle protette, l'**hash** (Argon2id) della loro password, mai la password in chiaro. Tutto sta dentro il file cifrato.
- Scegliendo «solo alcune sedute» si salvano solo i pazienti a cui appartengono quelle sedute.

I modelli di trascrizione e riassunto **non** sono nel backup (si riscaricano).

## Password

Il backup è **sempre cifrato con la password master**. Se non ne hai una, l'app ti chiede di crearla.

> ⚠️ Senza la password un backup non si può aprire, per nessuno. Annotala in un posto sicuro.
> Se in futuro reimposti la password master con la domanda segreta, i backup già fatti restano apribili **solo con la vecchia password**.

## Dove salvarlo

Scegli tu la posizione: disco del computer, chiavetta, hard disk esterno, oppure una cartella sincronizzata con Dropbox, Google Drive, iCloud, OneDrive. L'app non si collega a nessuno di questi servizi: il file cifrato viene soltanto copiato nella cartella scelta.

## Ripristinare

1. *Impostazioni → Backup → Ripristina da un backup*.
2. Scegli il file `.tsbackup` e inserisci la password con cui è stato creato.
3. Scegli cosa recuperare: tutto, solo alcuni pazienti o solo alcune sedute. Sedute e pazienti già presenti nell'app **non vengono mai sovrascritti** (un paziente con lo stesso identificativo resta com'è).
4. Se una seduta puntava a un paziente che non è nel backup, torna in «Senza paziente». Le cartelle protette ripristinate restano chiuse finché non inserisci la loro password.

## Dettagli tecnici

- Cifratura: XChaCha20-Poly1305 a blocchi da 1 MiB; ogni blocco è autenticato, quindi un file alterato o troncato viene rifiutato.
- Chiave: Argon2id (64 MiB, 3 passate) con sale casuale per ogni backup.
- Il file contiene un archivio compresso: elenco sedute e pazienti (`manifest.json`), dati (`db.json`, con le tabelle `patients` e `meetings`) e audio. Il formato resta `1`: i backup fatti prima delle cartelle per paziente si ripristinano normalmente (le sedute vanno in «Senza paziente»).
- Le password delle cartelle sono un blocco di interfaccia, non una cifratura dei dati (vedi [guida pazienti](PAZIENTI.md)).
