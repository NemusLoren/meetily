# Trascrivi Seduta

**Trascrizione privata delle sedute cliniche, per psicologi e psicoterapeuti.**
Registra la seduta, ottieni la trascrizione e, se vuoi, un riassunto: tutto sul tuo computer, senza inviare nulla online.

> ⚠️ **Stato del progetto: in sviluppo.** Alcune funzioni descritte sotto (password master, cartelle per paziente, interfaccia tutta in italiano) sono in lavorazione e **non ancora disponibili**. Vedi la sezione [Cosa funziona e cosa manca](#cosa-funziona-e-cosa-manca).

## A chi serve

A chi fa colloqui clinici e vuole smettere di prendere appunti a mano senza affidare le sedute a un servizio cloud. Funziona su **macOS** e **Windows**.

## Cosa fa

- **Registra** la voce tua (microfono) e quella del paziente (audio del computer, per le sedute online).
- **Trascrive in locale** con modelli Whisper: l'audio non lascia il computer.
- **Riassume** con modelli locali.
- **Tutto resta sul tuo computer**: registrazioni, trascrizioni e riassunti.

## Cosa funziona e cosa manca

| Funzione | Stato |
| --- | --- |
| Registrazione e trascrizione in locale | ✅ disponibile (ereditata da Meetily) |
| Riassunti con modelli locali, con modelli clinici in italiano (nota di seduta, SOAP, andamento, primo colloquio) | ✅ disponibile |
| Interfaccia in italiano (lingua predefinita) | ✅ quasi tutta: restano alcuni messaggi dal nucleo nativo, da verificare nell'app |
| Altre lingue tramite file `.po`, scelte in Impostazioni | ✅ disponibili italiano e inglese; altre si aggiungono con un file `.po` ([guida](docs/TRADUZIONI.md)) |
| Password master facoltativa all'avvio, con domanda segreta per reimpostarla | ✅ disponibile: blocca l'apertura dell'app (password protetta con Argon2id) |
| Backup cifrato con la password master (tutto, solo alcuni pazienti, solo alcune sedute, solo alcuni tipi di file), da salvare dove vuoi | ✅ disponibile in Impostazioni → Backup ([guida](docs/BACKUP.md)) |
| Cifratura dei dati (database e audio) | 🚧 pianificato: **oggi i dati NON sono cifrati**, la password non protegge i file sul disco |
| Cartelle per paziente, con password facoltativa, e sottocartelle per giorno di seduta | ✅ disponibile ([guida](docs/PAZIENTI.md)): la password della cartella è oggi un blocco di interfaccia, **non cifra** i dati; le cartelle sono logiche (nell'archivio), non cartelle su disco |
| Riassunti solo con modelli locali (provider cloud rimossi) | ✅ fatto: restano Ollama e il modello integrato |
| Nessuna statistica d'uso né icona scaricata da internet | ✅ fatto: rimosse del tutto |
| Aggiornamenti automatici | ❌ non previsti per ora: si scarica la nuova versione a mano |

Finché la cifratura dei dati non è pronta, **non usare l'app con dati reali di pazienti** su un computer non protetto (almeno cifratura del disco attiva: FileVault su Mac, BitLocker su Windows).

## Requisiti

- **macOS** 12 o successivo, preferibilmente Mac con chip Apple (M1 o più recente), oppure **Windows 10/11 a 64 bit**
- 8 GB di memoria (16 GB consigliati per i modelli più precisi)
- Circa 5 GB di spazio libero per i modelli
- Microfono

Dettagli e primi passi: [guida all'installazione](docs/INSTALLAZIONE.md).

## Privacy

Leggi l'[informativa privacy](PRIVACY_POLICY.md). Usare il software non ti esonera dai tuoi obblighi di professionista (GDPR, consenso informato, segreto professionale): sono dati sanitari di categoria particolare.

## Contribuire

Sei un collega o uno sviluppatore? Ogni idea, segnalazione o traduzione è benvenuta: leggi [CONTRIBUTING.md](CONTRIBUTING.md) oppure scrivi a **info@leandromancino.it**. Non allegare mai dati di pazienti.

Per chi sviluppa: [istruzioni di compilazione](docs/BUILDING.md) e [architettura](docs/architecture.md) (in inglese).

## Origine e licenza

Trascrivi Seduta è un fork di [Meetily](https://github.com/Zackriya-Solutions/meetily) di Zackriya Solutions, rilasciato con licenza MIT: vedi [LICENSE.md](LICENSE.md). La cartella `backend/` è un archivio storico non supportato.
