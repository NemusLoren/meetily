# Modelli di riassunto

Questa cartella contiene i modelli con cui Trascrivi Seduta compila le note cliniche.

| File | Uso |
| --- | --- |
| `nota_di_seduta.json` | Nota di seduta generale (modello predefinito) |
| `nota_soap.json` | Nota in formato SOAP |
| `andamento_trattamento.json` | Verifica dei progressi del percorso |
| `colloquio_iniziale.json` | Primo colloquio |

## Struttura

```json
{
  "name": "Nome mostrato",
  "description": "A cosa serve",
  "sections": [
    { "title": "Titolo", "instruction": "Cosa deve scrivere il modello", "format": "paragraph|list|string",
      "item_format": "Facoltativo: intestazione di tabella Markdown per le liste" }
  ]
}
```

Il riassunto è sempre un **supporto**: va riletto e firmato dal professionista. Il modello deve riportare solo ciò che emerge dalla trascrizione, senza formulare diagnosi.

## Modelli personalizzati

Si possono aggiungere file `.json` nella cartella dei dati dell'app, sotto `templates/`: sostituiscono quelli predefiniti con lo stesso nome.
