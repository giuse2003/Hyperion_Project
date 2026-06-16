# 🚀 Guida di Scansione Distribuita (Hyperion Project)

Questa guida spiega come configurare ed avviare il sistema distribuito di ricerca chiavi, composto dal **Server Coordinator** (centrale) e dai **Worker** (client di elaborazione Rust).

---

## 📋 Flusso di Lavoro in 3 Passaggi

Per iniziare la ricerca, i passaggi devono essere eseguiti in questo ordine:

### 1️⃣ Avviare il Server Coordinator (sul PC Principale)
Il server coordinator deve essere sempre attivo prima di lanciare i worker, in modo da poter distribuire i blocchi di lavoro.
1. Apri la cartella del server coordinator: `D:\GitHub\Hyperion_Project\Server_Coordinator\`
2. Fai doppio clic su **`Avvia_Server.cmd`**.
3. Il server si avvierà in ascolto sulla porta **8085** e rimarrà in attesa dei client. Lascia la finestra aperta.

### 2️⃣ Generare il Bloom Filter `filter.bin` (sul PC Principale)
Il worker ha bisogno del database compresso per fare i controlli offline ad altissima velocità.
1. Scarica il dump aggiornato da Loyce.club (file `.tsv.gz` con i saldi attivi, es. `blockchair_bitcoin_addresses_and_balance_LATEST.tsv.gz`).
2. Posiziona il file scaricato nella cartella `generator` di questo progetto: `D:\GitHub\Hyperion_Project\generator\`
3. Apri il terminale in quella cartella e genera il filtro:
   ```cmd
   python generator.py blockchair_bitcoin_addresses_and_balance_LATEST.tsv.gz 80000000 0.000001
   ```
4. Lo script produrrà un file `filter.bin` da circa 274 MB.

### 3️⃣ Avviare i Worker (Client Rust)
Una volta che il coordinator è attivo e il file `filter.bin` è pronto, puoi lanciare i worker.

#### A. Sul PC Principale (stesso computer del server)
1. Vai nella cartella `D:\GitHub\Hyperion_Project\Worker_Client\`.
2. Fai doppio clic su **`Avvia_Worker.cmd`**.
3. Alla prima esecuzione, lo script copierà automaticamente `hyperion_worker.exe` (compilato da Rust in `worker/target/release/`) e il file `filter.bin` (dal generatore) in questa cartella.
4. Quando ti viene chiesto l'IP del coordinator, premi semplicemente **Invio** (utilizzerà `127.0.0.1`).

#### B. Su PC Esterni (Macchine remote)
1. Copia l'intera cartella **`Worker_Client`** (che ora contiene `hyperion_worker.exe`, `filter.bin` e `Avvia_Worker.cmd`) su una chiavetta USB o tramite rete e incollala sul PC esterno.
2. Fai doppio clic su **`Avvia_Worker.cmd`** sul PC esterno.
3. Inserisci i parametri richiesti:
   * **IP/Hostname del Server:** L'IP locale del PC principale (es. `192.168.1.50` o il nome host del computer).
   * **Thread CPU:** Il numero di core CPU da dedicare alla scansione (es. `4`, `8`, `12`).
   * **Worker ID:** Un nome per identificare la macchina (es. `PC-Portatile`). Invio per usare il nome di Windows.

---

## 🛠️ Compilazione manuale del Worker (Se modifichi il codice Rust)
Se apporti modifiche al codice sorgente in Rust (`worker/src/main.rs`) e hai bisogno di ricompilare l'eseguibile:
1. Apri il terminale nella cartella `worker` ed esegui la compilazione ottimizzata in modalità release:
   ```cmd
   cd worker
   cargo build --release
   ```
2. Lo script `Avvia_Worker.cmd` rileverà la nuova build e aggiornerà l'eseguibile nella cartella `Worker_Client` al successivo avvio.

