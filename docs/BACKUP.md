# Backup cifrato

In **Impostazioni → Backup** puoi salvare una copia cifrata dei tuoi dati e ripristinarla quando serve.

## Cosa puoi salvare

- **Tipi di contenuto** (scegli quali): trascrizioni, riassunti, appunti, registrazioni audio.
- **Quali sedute**: tutte, oppure solo quelle che selezioni dall'elenco.
- Le cartelle per paziente non esistono ancora: appena arriveranno, si potrà scegliere anche per paziente.

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
3. Scegli le sedute da recuperare. Quelle già presenti nell'app **non vengono mai sovrascritte**.

## Dettagli tecnici

- Cifratura: XChaCha20-Poly1305 a blocchi da 1 MiB; ogni blocco è autenticato, quindi un file alterato o troncato viene rifiutato.
- Chiave: Argon2id (64 MiB, 3 passate) con sale casuale per ogni backup.
- Il file contiene un archivio compresso: elenco sedute, dati e audio.
