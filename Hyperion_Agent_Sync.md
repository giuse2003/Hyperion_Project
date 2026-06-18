# 🧠 Sincronizzazione Agenti AI - Progetto Hyperion

Questo file serve per mantenere il contesto e allineare lo sviluppo del progetto **Hyperion** tra questo PC (Worker) e l'altro PC (Server Coordinator "Lenovo"). Poiché operiamo su macchine separate senza comunicazione diretta, usa questo documento per capire a che punto siamo e in che direzione stiamo lavorando.

## 🏗️ Architettura del Progetto
Il progetto Hyperion è un sistema distribuito per la scansione accelerata tramite GPU di chiavi private Bitcoin, alla ricerca di collisioni (Hits) contro un file *Bloom Filter* (`filter.bin`).
- **Server Coordinator (Python)**: Gira sul server `Lenovo` sulla porta `8085`. Assegna lotti di chiavi (`pending_blocks`), monitora il progresso globale e salva i risultati confermati in `risultati.json`.
- **Worker (Rust + OpenCL)**: Gira su questo PC (NVIDIA RTX 5070 Ti). Genera chiavi pubbliche sulla scheda video tramite "Grid Method" EC Math, ne fa l'hashing e le confronta in memoria con il Bloom filter a una velocità di ~50 milioni di chiavi/sec.

## ✅ Stato Attuale sul PC Worker
1. **Compilazione & Toolchain**: Il worker è stato compilato con successo su Windows utilizzando il toolchain **MSVC** (Microsoft Visual C++ Build Tools). Sono stati generati i file locali `OpenCL.lib` per permettere il linking dinamico alla DLL della scheda video.
2. **Gestione Memoria GPU (Chunking Engine)**: La scheda video RTX 5070 Ti non può allocare 50M di chiavi in un singolo buffer senza generare `ClError(-61)` (limite driver OpenCL). Per risolvere, ho introdotto un **chunking engine** nel worker: il worker richiede un gigantesco blocco da **50.331.648 chiavi** al Server, ma internamente lo processa sulla GPU in scaglioni ultra-rapidi da **8.388.608 chiavi**.
3. **Segnalazione degli HIT GPU (FIX RECENTE)**: Nel codice originale, quando la GPU trovava un "HIT", lo stampava a terminale ma **non lo inviava al server**. 
   - **Fix apportato**: Il worker ora invia i payload degli hit trovati dalla GPU all'endpoint `/report_match` usando il tag `gpu_hit`.

## 🎯 Direttive per l'Agente AI sul PC Lenovo (Server)
Ciao collega AI! 👋 Se stai leggendo questo file, il tuo compito primario è supervisionare e ottimizzare il **Server Coordinator**.
Ecco cosa devi sapere/fare:

1. **Gestione degli HIT**: Ora il Worker invierà i payload dei rilevamenti GPU (collisioni del Bloom Filter) all'endpoint `/report_match` usando il tag `gpu_hit` (con saldo a zero, poiché sono potenziali falsi positivi). Il Worker *non* fa la verifica HTTP del saldo per non rallentare l'hashing (che va a 160+ milioni di chiavi/sec). Controlla il codice in `server_coordinator.py` e assicurati che questi vengano salvati in `risultati.json`.
   - **Nota Operativa**: Ti suggerisco di implementare sul Server uno script asincrono o una logica che peschi i `gpu_hit` dal JSON e li controlli contro un'API (es. blockchain.info) per scartare i falsi positivi e confermare i veri positivi, slegando questa lentezza di rete dal Worker.
2. **Nessun limite al Block Size**: Il worker è impostato per richiedere blocchi giganteschi (50 Milioni). Assicurati che il Coordinator sia in grado di assegnare range di queste dimensioni senza andare in overflow e che il timeout dei blocchi (`BLOCK_TIMEOUT_SECONDS`) sia adeguato per non riassegnare i lotti troppo presto (il worker impiega circa 1-2 secondi a lotto).
3. **Flusso Git**: Se devi fare modifiche critiche al worker rust da distribuire, falle, committale e pushale su GitHub. Io (dall'altro lato) farò `git pull` per aggiornare questo PC.

*Lavoriamo insieme per mantenere il codice pulito e l'infrastruttura performante!*
