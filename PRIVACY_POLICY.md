# Informativa sulla privacy di Trascrivi Seduta

*Ultimo aggiornamento: 16 novembre 2026. Documento tecnico sul funzionamento del software, non un parere legale.*

## In breve

Trascrivi Seduta elabora le sedute **sul tuo computer**. Il progetto non gestisce server che ricevono le tue registrazioni o trascrizioni, e chi lo sviluppa non può vederle.

## Dove stanno i dati

Registrazioni, trascrizioni e riassunti sono salvati in locale. Trascrizione e riassunti con modelli locali non inviano l'audio né il testo online.

## Connessioni di rete

Il software può collegarsi a internet nei casi seguenti:

- **Scaricare i modelli** di trascrizione e di riassunto, la prima volta o quando li scegli.
- **Ollama (facoltativo)**: se lo usi per i riassunti, il testo va al server Ollama indicato nelle impostazioni, di norma sul tuo stesso computer. Non indicare server di altre persone.
- **Nessuna statistica d'uso**, nessun provider di riassunto o trascrizione in cloud, nessuna icona scaricata da internet.

Non ci sono aggiornamenti automatici: nessuna verifica periodica di nuove versioni.

## Cifratura e protezione

**Attenzione: nella versione attuale i dati NON sono cifrati dall'app.** La password master (facoltativa) blocca l'apertura dell'app, ma non protegge i file sul disco: chi accede direttamente alla cartella dei dati può leggerli. La cifratura dell'archivio è in sviluppo. Nel frattempo attiva la cifratura del disco (FileVault su Mac, BitLocker su Windows) e proteggi l'accesso al computer.

## Backup

Il backup crea un file `.tsbackup` **cifrato** (XChaCha20-Poly1305, chiave derivata dalla password master con Argon2id). Puoi salvarlo dove vuoi, anche in una cartella sincronizzata con Dropbox o Google Drive: l'app non si collega a quei servizi e il file è illeggibile senza la password. L'intestazione in chiaro contiene solo versione, data e parametri tecnici, nessun dato clinico. Senza la password un backup **non si può recuperare**.

## Le tue responsabilità di professionista

Le sedute sono **dati relativi alla salute** (art. 9 GDPR). Resta a te, come titolare del trattamento, garantire base giuridica, consenso informato del paziente alla registrazione, misure di sicurezza adeguate, tempi di conservazione e cancellazione, e il rispetto del segreto professionale.

## Contatti

info@leandromancino.it

Software derivato da Meetily (Zackriya Solutions), licenza MIT.
