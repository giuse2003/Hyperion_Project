# 🧠 Sincronizzazione Agenti AI - Progetto Hyperion

Questo file serve per mantenere il contesto e allineare lo sviluppo del progetto **Hyperion** tra la flotta di **Worker** (PC multipli con GPU diverse, es. NVIDIA o AMD, come desktop-casa-giuse) e il **Server Coordinator** ("Lenovo"). Poiché operiamo su macchine separate senza comunicazione diretta, usa questo documento per capire a che punto siamo e in che direzione stiamo lavorando.

## 🏗️ Architettura del Progetto
Il progetto Hyperion è un sistema distribuito per la scansione accelerata tramite GPU di chiavi private Bitcoin, alla ricerca di collisioni (Hits) contro un file *Bloom Filter* (`filter.bin`).
- **Server Coordinator (Python)**: Gira sul server `Lenovo` sulla porta `8085`. Assegna lotti di chiavi (`pending_blocks`), monitora il progresso globale e salva i risultati confermati in `risultati.json`.
- **Worker (Rust + OpenCL)**: Girano su molteplici PC (es. NVIDIA RTX 5070 Ti, AMD su desktop, ecc.). Generano chiavi pubbliche sulla scheda video tramite "Grid Method" EC Math, ne fanno l'hashing e le confrontano in memoria con il Bloom filter.

## ✅ Stato Attuale dei PC Worker
1. **Compilazione & Toolchain**: Il worker è compilato in Rust su Windows utilizzando MSVC. Viene sfruttato **OpenCL** per garantire l'astrazione hardware e la compatibilità cross-vendor su tutte le schede (AMD/NVIDIA).
2. **Gestione Memoria GPU (Chunking Engine)**: Per evitare limiti di allocazione massimi imposti da alcuni driver OpenCL (es. l'errore `ClError(-61)` rilevato su alcune schede), è stato implementato un **chunking engine**: il worker richiede blocchi da oltre **50 milioni di chiavi** al Server, ma poi li processa sulla GPU in "scaglioni" più sicuri e veloci (es. da 8 milioni).
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
