# 🚀 Guida di Avvio Rapido del Worker (Hyperion Project)

Questa cartella è pensata per ospitare tutto il necessario per avviare il client di ricerca (worker) in modo indipendente, sia sul PC principale che su macchine esterne dedicate.

---

## 📦 File Necessari per il Funzionamento
Per far funzionare il worker sono richiesti esattamente **3 file** in questa cartella:

1. **`hyperion_worker.exe`**: L'eseguibile compilato in Rust del worker.
2. **`filter.bin`**: Il database Bloom Filter binario contenente tutti gli indirizzi Bitcoin attivi con saldo positivo.
3. **`Avvia_Worker.cmd`**: Lo script batch interattivo per avviare il processo di scansione.

---

## 🛠️ Come Preparare la Cartella sul PC Principale
Se hai appena clonato il repository sul tuo computer principale, segui questi passaggi per popolare la cartella `Workers`:

1. **Compila il Worker (Rust):**
   Apri un terminale nella cartella `worker` ed esegui la compilazione ottimizzata in modalità release:
   ```cmd
   cd worker
   cargo build --release
   ```
2. **Genera il Bloom Filter reale (`filter.bin`):**
   Scarica il dump degli indirizzi attivi da Loyce.club (es. `blockchair_bitcoin_addresses_and_balance_LATEST.tsv.gz`), salvalo nella cartella `generator` ed esegui:
   ```cmd
   cd generator
   python generator.py blockchair_bitcoin_addresses_and_balance_LATEST.tsv.gz 80000000 0.000001
   ```
3. **Popolamento Automatico:**
   Fai semplicemente doppio clic su **`Avvia_Worker.cmd`** in questa cartella (`Workers\`). Lo script rileverà l'eseguibile compilato in `worker/target/release/` e il file `filter.bin` generato in `generator/` e **li copierà automaticamente qui dentro**, rendendo la cartella pronta ed indipendente.

---

## 💻 Come Eseguire su PC Esterni (Stand-alone)
Per distribuire la computazione su altri PC Windows senza dover installare Rust, Python o scaricare l'intero codice sorgente:

1. Copia l'intera cartella **`Workers`** (che ora contiene `hyperion_worker.exe`, `filter.bin` e `Avvia_Worker.cmd`) su una chiavetta USB o inviala tramite rete al PC di destinazione.
2. Sul PC di destinazione, apri la cartella e fai doppio clic su **`Avvia_Worker.cmd`**.
3. Inserisci le informazioni richieste:
   * **IP/Hostname del Server Coordinator:** L'indirizzo IP locale del tuo PC principale (es. `192.168.1.50` o il nome host come `desktop-casa-giuse`). Il coordinator deve essere in esecuzione sulla porta `8085`.
   * **Numero di Thread:** Il numero di core CPU che desideri dedicare alla scansione (es. `4`, `8`, `12`).
   * **ID del Worker:** Un nome univoco per identificare la macchina (es. `PC-Portatile`, `PC-Ufficio`). Premendo semplicemente Invio verrà usato il nome del computer di Windows.

Il worker scaricherà automaticamente i range di chiavi dal server coordinator principale, verificherà le chiavi generate in locale a grandissima velocità e riporterà i risultati in tempo reale.
