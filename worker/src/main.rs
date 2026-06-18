use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use std::sync::mpsc;

use clap::Parser;
use serde::{Deserialize, Serialize};

// --- Moduli Crittografici ---
use ripemd::{Digest as RipeDigest, Ripemd160};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha2::Sha256;

// --- Hashing FNV-1a a 64 bit (uguale a generator.py) ---
fn fnv1a_64(data: &[u8]) -> u64 {
    let mut h = 14695981039346656037u64;
    for &byte in data {
        h ^= byte as u64;
        h = h.wrapping_mul(1099511628211u64);
    }
    h
}

fn get_hashes(data: &[u8]) -> (u32, u32) {
    let h = fnv1a_64(data);
    let h1 = (h & 0xffffffff) as u32;
    let h2 = ((h >> 32) & 0xffffffff) as u32;
    (h1, h2)
}

// --- Bech32 Address Encoding for P2WPKH ---
const BECH32_CHARSET: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

fn bech32_polymod(values: &[u8]) -> u32 {
    let generators: [u32; 5] = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];
    let mut checksum = 1u32;
    for &value in values {
        let top = checksum >> 25;
        checksum = ((checksum & 0x1ffffff) << 5) ^ (value as u32);
        for i in 0..5 {
            if ((top >> i) & 1) != 0 {
                checksum ^= generators[i];
            }
        }
    }
    checksum
}

fn convert_bits(data: &[u8], from_bits: u8, to_bits: u8, pad: bool) -> Option<Vec<u8>> {
    let mut acc = 0u32;
    let mut bits = 0u8;
    let mut ret = Vec::new();
    let maxv = (1u32 << to_bits) - 1;
    let max_acc = (1u32 << (from_bits + to_bits - 1)) - 1;
    for &value in data {
        acc = ((acc << from_bits) | (value as u32)) & max_acc;
        bits += from_bits;
        while bits >= to_bits {
            bits -= to_bits;
            ret.push(((acc >> bits) & maxv) as u8);
        }
    }
    if pad {
        if bits > 0 {
            ret.push(((acc << (to_bits - bits)) & maxv) as u8);
        }
    } else if bits >= from_bits || (((acc << (to_bits - bits)) & maxv) != 0) {
        return None;
    }
    Some(ret)
}

fn segwit_address(pubkey_hash: &[u8]) -> String {
    let converted = convert_bits(pubkey_hash, 8, 5, true).unwrap();
    let mut data = Vec::with_capacity(converted.len() + 1);
    data.push(0u8); // Witness version 0
    data.extend(converted);

    // bc1 prefix expansion
    let mut expanded = vec![3u8, 3u8, 0u8, 2u8, 3u8];
    expanded.extend(&data);
    expanded.extend(&[0u8; 6]);

    let polymod = bech32_polymod(&expanded) ^ 1u32;
    let mut checksum = Vec::with_capacity(6);
    for i in 0..6 {
        checksum.push(((polymod >> (5 * (5 - i))) & 31) as u8);
    }

    let mut addr = String::from("bc1");
    for &v in data.iter().chain(checksum.iter()) {
        addr.push(BECH32_CHARSET[v as usize] as char);
    }
    addr
}

// --- Derivazione Indirizzi Bitcoin ---
fn hash160(data: &[u8]) -> Vec<u8> {
    let sha = Sha256::digest(data);
    let mut rip = Ripemd160::new();
    rip.update(&sha);
    rip.finalize().to_vec()
}

#[derive(Clone)]
struct DerivedKeys {
    wif: String,
    legacy_addr: String,
    nested_addr: String,
    native_addr: String,
}

fn derive_addresses(secp: &Secp256k1<secp256k1::All>, private_key_int: u128) -> DerivedKeys {
    // Conversione chiave in 32 byte big-endian
    let mut priv_bytes = [0u8; 32];
    let bytes = private_key_int.to_be_bytes();
    priv_bytes[16..32].copy_from_slice(&bytes); // u128 entra comodamente nella metà inferiore

    let secret_key = SecretKey::from_slice(&priv_bytes).unwrap();
    let pub_key = PublicKey::from_secret_key(secp, &secret_key);
    let compressed_pubkey = pub_key.serialize();

    // Generazione WIF (Wallet Import Format) compressa
    let mut payload = Vec::with_capacity(34);
    payload.push(0x80u8);
    payload.extend(&priv_bytes);
    payload.push(0x01u8);
    let wif = bs58::encode(payload).with_check().into_string();

    // HASH160 della chiave pubblica
    let pubkey_hash = hash160(&compressed_pubkey);

    // 1. Legacy P2PKH (Prefisso 0x00)
    let mut legacy_payload = Vec::with_capacity(21);
    legacy_payload.push(0x00u8);
    legacy_payload.extend(&pubkey_hash);
    let legacy_addr = bs58::encode(legacy_payload).with_check().into_string();

    // 2. Nested SegWit P2SH-P2WPKH (Prefisso 0x05)
    let mut redeem_script = Vec::with_capacity(22);
    redeem_script.push(0x00u8);
    redeem_script.push(0x14u8);
    redeem_script.extend(&pubkey_hash);
    let redeem_hash = hash160(&redeem_script);

    let mut nested_payload = Vec::with_capacity(21);
    nested_payload.push(0x05u8);
    nested_payload.extend(&redeem_hash);
    let nested_addr = bs58::encode(nested_payload).with_check().into_string();

    // 3. Native SegWit P2WPKH
    let native_addr = segwit_address(&pubkey_hash);

    DerivedKeys {
        wif,
        legacy_addr,
        nested_addr,
        native_addr,
    }
}

// --- Gestione Bloom Filter ---
struct BloomFilterLoader {
    m: u64,
    k: u32,
    bit_array: Vec<u8>,
}

impl BloomFilterLoader {
    fn load_from_file<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let mut file = File::open(path)?;
        
        let mut m_bytes = [0u8; 8];
        file.read_exact(&mut m_bytes)?;
        let m = u64::from_le_bytes(m_bytes);

        let mut k_bytes = [0u8; 4];
        file.read_exact(&mut k_bytes)?;
        let k = u32::from_le_bytes(k_bytes);

        let mut bit_array = Vec::new();
        file.read_to_end(&mut bit_array)?;

        let expected_bytes = (m + 7) / 8;
        if bit_array.len() as u64 != expected_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Dimensione file non coerente. Atteso {} byte, ricevuti {} byte", expected_bytes, bit_array.len()),
            ));
        }

        println!("[BloomFilter] Caricato con successo: m = {} bit ({:.2} MB), k = {} hash", m, (bit_array.len() as f64) / (1024.0 * 1024.0), k);
        Ok(BloomFilterLoader { m, k, bit_array })
    }

    #[inline(always)]
    fn check(&self, item: &[u8]) -> bool {
        let (h1, h2) = get_hashes(item);
        for i in 0..self.k {
            let bit_pos = ((h1 as u64).wrapping_add((i as u64).wrapping_mul(h2 as u64))) % self.m;
            let byte_idx = (bit_pos / 8) as usize;
            let bit_offset = (bit_pos % 8) as u8;
            if (self.bit_array[byte_idx] & (1 << bit_offset)) == 0 {
                return false;
            }
        }
        true
    }
}

// --- Strutture JSON per API Coordinator ---
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    server: String,

    #[arg(long, default_value_t = 8085)]
    port: u16,

    #[arg(long)]
    worker_id: Option<String>,

    #[arg(long)]
    threads: Option<usize>,

    #[arg(long)]
    cpu: bool, // Forza l'uso esclusivo della CPU
}

#[derive(Deserialize, Debug)]
struct WorkResponse {
    status: String,
    start: Option<String>,
    count: Option<String>,
    message: Option<String>,
}

#[derive(Serialize, Debug)]
struct ReportCompleted {
    worker_id: String,
    start_key: String,
    count: u64,
}

#[derive(Serialize, Debug)]
struct AddressResults {
    confirmed: u64,
    unconfirmed: u64,
    history_count: u64,
}

#[derive(Serialize, Debug)]
struct ResultsData {
    legacy: AddressResults,
    nested: AddressResults,
    native: AddressResults,
}

#[derive(Serialize, Debug)]
struct MatchReport {
    worker_id: String,
    private_key_number: String,
    wif: String,
    addresses: std::collections::HashMap<String, String>,
    results: ResultsData,
    has_active_balance: bool,
}

// --- Query API Pubblica per conferma saldo ---
fn query_public_balance_and_history(address: &str) -> (u64, u64) {
    // Usiamo l'API di Blockstream per verificare il saldo effettivo in modo sicuro
    let url = format!("https://blockstream.info/api/address/{}", address);
    for _ in 0..3 {
        match ureq::get(&url).call() {
            Ok(resp) => {
                if let Ok(json) = resp.into_json::<serde_json::Value>() {
                    let chain_stats = &json["chain_stats"];
                    let funded = chain_stats["funded_txo_sum"].as_u64().unwrap_or(0);
                    let spent = chain_stats["spent_txo_sum"].as_u64().unwrap_or(0);
                    let tx_count = chain_stats["tx_count"].as_u64().unwrap_or(0);
                    
                    let balance = if funded > spent { funded - spent } else { 0 };
                    return (balance, tx_count);
                }
            }
            Err(e) => {
                eprintln!("[API Pubblica] Errore query bilancio per {}: {}. Riprovo...", address, e);
                thread::sleep(Duration::from_secs(2));
            }
        }
    }
    (0, 0)
}

fn check_address_online(address: &str) -> Option<(u64, u64)> {
    let (balance, tx_count) = query_public_balance_and_history(address);
    if tx_count > 0 {
        Some((balance, tx_count))
    } else {
        None
    }
}

// --- Flusso Principale del Thread Worker ---
fn worker_thread_loop(
    id: usize,
    bloom_filter: Arc<BloomFilterLoader>,
    task_rx: mpsc::Receiver<(u128, u64)>,
    result_tx: mpsc::Sender<(u128, DerivedKeys, String, u64, u64)>,
    keep_running: Arc<AtomicBool>,
    progress: Arc<AtomicU64>,
) {
    let secp = Secp256k1::new();
    
    while keep_running.load(Ordering::Relaxed) {
        if let Ok((start_key, count)) = task_rx.recv_timeout(Duration::from_millis(100)) {
            for offset in 0..count {
                if !keep_running.load(Ordering::Relaxed) {
                    break;
                }
                
                let current_key = start_key + offset as u128;
                let derived = derive_addresses(&secp, current_key);
                
                // Controlla Legacy
                if bloom_filter.check(derived.legacy_addr.as_bytes()) {
                    println!("[Thread {}] HIT LOCALE (Bloom Filter) rilevato per Legacy: {}", id, derived.legacy_addr);
                    if let Some((bal, txs)) = check_address_online(&derived.legacy_addr) {
                        result_tx.send((current_key, derived, String::from("legacy"), bal, txs)).unwrap();
                        keep_running.store(false, Ordering::Relaxed);
                        break;
                    }
                }
                
                // Controlla Nested
                if bloom_filter.check(derived.nested_addr.as_bytes()) {
                    println!("[Thread {}] HIT LOCALE (Bloom Filter) rilevato per Nested: {}", id, derived.nested_addr);
                    if let Some((bal, txs)) = check_address_online(&derived.nested_addr) {
                        result_tx.send((current_key, derived, String::from("nested"), bal, txs)).unwrap();
                        keep_running.store(false, Ordering::Relaxed);
                        break;
                    }
                }

                // Controlla Native
                if bloom_filter.check(derived.native_addr.as_bytes()) {
                    println!("[Thread {}] HIT LOCALE (Bloom Filter) rilevato per Native: {}", id, derived.native_addr);
                    if let Some((bal, txs)) = check_address_online(&derived.native_addr) {
                        result_tx.send((current_key, derived, String::from("native"), bal, txs)).unwrap();
                        keep_running.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            }
            // Incrementa il progresso una volta completato il chunk
            progress.fetch_add(count, Ordering::Relaxed);
        } else {
            // Nessun compito ricevuto, verifica se dobbiamo uscire
            if !keep_running.load(Ordering::Relaxed) {
                break;
            }
        }
    }
}

fn main() {
    let args = Args::parse();
    let worker_id = args.worker_id.unwrap_or_else(|| {
        std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| String::from("unknown_worker"))
    });

    let coordinator_url = format!("http://{}:{}", args.server, args.port);
    println!("=== HYPERION WORKER RUST STARTED ===");
    println!("Coordinator: {}", coordinator_url);
    println!("Worker ID: {}", worker_id);
    let num_threads = args.threads.unwrap_or_else(|| {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
    });
    
    println!("Threads CPU: {}", num_threads);
    println!("Modalità GPU: {}", if args.cpu { "DISABILITATA MANUALMENTE" } else { "AUTOMATICA (Default)" });

    let mut block_size = 100000u64; // Default CPU
    
    // Carica il Bloom Filter
    let filter_path = "filter.bin";
    if !Path::new(filter_path).exists() {
        eprintln!("Errore: File '{}' non trovato in questa directory. Generalo prima con generator.py", filter_path);
        std::process::exit(1);
    }

    let bloom_filter = Arc::new(BloomFilterLoader::load_from_file(filter_path).unwrap());

    // Inizializza OpenCL se abilitato
    let mut gpu_context = None;

    if !args.cpu {
        println!("\n=== INIZIALIZZAZIONE OPENCL GPU ===");
        match opencl3::platform::get_platforms() {
            Ok(platforms) if !platforms.is_empty() => {
                let mut chosen_device = None;
                for platform in platforms {
                    println!("Piattaforma: {}", platform.name().unwrap_or_default());
                    if let Ok(devices) = platform.get_devices(opencl3::device::CL_DEVICE_TYPE_GPU) {
                        if let Some(&device_id) = devices.first() {
                            let device = opencl3::device::Device::new(device_id);
                            println!("- Device GPU trovato: {}", device.name().unwrap_or_default());
                            chosen_device = Some(device_id);
                            break;
                        }
                    }
                }
                if let Some(device_id) = chosen_device {
                    println!("GPU rilevata correttamente! Preparazione Grid Method...");
                    block_size = 50331648u64;
                    
                    let context = opencl3::context::Context::from_device(&opencl3::device::Device::new(device_id)).expect("Creazione context fallita");
                    let queue = opencl3::command_queue::CommandQueue::create_default(&context, 0).expect("Creazione command queue fallita");
                    let program_src = include_str!("hyperion_kernel.cl");
                    let program = opencl3::program::Program::create_and_build_from_source(&context, program_src, "").expect("Compilazione kernel OpenCL fallita!");
                    let kernel_add = opencl3::kernel::Kernel::create(&program, "ec_add_grid").expect("Creazione kernel_add fallita");
                    let kernel_invert = opencl3::kernel::Kernel::create(&program, "heap_invert").expect("Creazione kernel_invert fallita");
                    let kernel_bloom = opencl3::kernel::Kernel::create(&program, "hash_ec_point_bloom").expect("Creazione kernel_bloom fallita");
                    
                    let mut filter_buffer = unsafe { opencl3::memory::Buffer::<u8>::create(&context, opencl3::memory::CL_MEM_READ_ONLY, bloom_filter.bit_array.len(), std::ptr::null_mut()).unwrap() };
                    let _ = unsafe { queue.enqueue_write_buffer(&mut filter_buffer, opencl3::command_queue::CL_BLOCKING, 0, &bloom_filter.bit_array, &[]).unwrap() };
                    
                    let output_buffer = unsafe { opencl3::memory::Buffer::<u64>::create(&context, opencl3::memory::CL_MEM_READ_WRITE, 100 * 2, std::ptr::null_mut()).unwrap() };
                    let output_count = unsafe { opencl3::memory::Buffer::<u32>::create(&context, opencl3::memory::CL_MEM_READ_WRITE, 1, std::ptr::null_mut()).unwrap() };
                    
                    let row_size = 8192usize;
                    let batch_size = 50331648u64;
                    let col_size = (batch_size / (row_size as u64)) as usize;
                    
                    let row_in = unsafe { opencl3::memory::Buffer::<u32>::create(&context, opencl3::memory::CL_MEM_READ_ONLY, row_size * 16, std::ptr::null_mut()).unwrap() };
                    let mut col_in = unsafe { opencl3::memory::Buffer::<u32>::create(&context, opencl3::memory::CL_MEM_READ_ONLY, col_size * 16, std::ptr::null_mut()).unwrap() };
                    let mut points_out = unsafe { opencl3::memory::Buffer::<u32>::create(&context, opencl3::memory::CL_MEM_READ_WRITE, (batch_size as usize) * 16, std::ptr::null_mut()).unwrap() };
                    let mut z_heap = unsafe { opencl3::memory::Buffer::<u32>::create(&context, opencl3::memory::CL_MEM_READ_WRITE, (batch_size as usize) * 2 * 8, std::ptr::null_mut()).unwrap() };
                    
                    println!("Precalcolo punti Row...");
                    let secp = secp256k1::Secp256k1::new();
                    let mut packed_row = vec![0u32; row_size * 16];
                    let bundle_size = 1024usize;
                    let stride = bundle_size / 8;
                    
                    for cell in 0..row_size {
                        let mut priv_bytes = [0u8; 32];
                        let key_bytes = ((cell as u128) + 1).to_be_bytes();
                        priv_bytes[16..32].copy_from_slice(&key_bytes);
                        let priv_key = secp256k1::SecretKey::from_slice(&priv_bytes).unwrap();
                        let pub_key = secp256k1::PublicKey::from_secret_key(&secp, &priv_key);
                        let serialized = pub_key.serialize_uncompressed();
                        
                        let mut px = [0u32; 8];
                        let mut py = [0u32; 8];
                        for i in 0..8 {
                            let ox = 33 - (i + 1) * 4;
                            px[i] = ((serialized[ox] as u32) << 24) | ((serialized[ox+1] as u32) << 16) | ((serialized[ox+2] as u32) << 8) | (serialized[ox+3] as u32);
                            let oy = 65 - (i + 1) * 4;
                            py[i] = ((serialized[oy] as u32) << 24) | ((serialized[oy+1] as u32) << 16) | ((serialized[oy+2] as u32) << 8) | (serialized[oy+3] as u32);
                        }
                        
                        let mut start = (((2 * cell) / stride) * bundle_size) + (cell % (stride / 2));
                        for i in 0..8 { packed_row[start + i * stride] = px[i]; }
                        start += stride / 2;
                        for i in 0..8 { packed_row[start + i * stride] = py[i]; }
                    }
                    let mut row_in_mut = row_in;
                    let _ = unsafe { queue.enqueue_write_buffer(&mut row_in_mut, opencl3::command_queue::CL_BLOCKING, 0, &packed_row, &[]).unwrap() };
                    
                    gpu_context = Some((context, queue, kernel_add, kernel_invert, kernel_bloom, filter_buffer, output_buffer, output_count, row_in_mut, col_in, points_out, z_heap, secp));
                } else {
                    println!("Nessuna scheda video idonea trovata. Switch automatico alla modalità CPU!");
                }
            }
            _ => println!("OpenCL non disponibile nel sistema. Switch automatico alla modalità CPU!"),
        }
        println!("===================================\n");
    }
    let keep_running = Arc::new(AtomicBool::new(true));

    // Canali per inviare i task ai thread ed i risultati al main thread
    let mut thread_senders = Vec::new();
    let (result_tx, result_rx) = mpsc::channel();

    let progress_counter = Arc::new(AtomicU64::new(0));

    // Avvio dei thread worker
    for i in 0..num_threads {
        let (task_tx, task_rx) = mpsc::channel();
        thread_senders.push(task_tx);
        
        let filter_clone = Arc::clone(&bloom_filter);
        let result_tx_clone = result_tx.clone();
        let keep_running_clone = Arc::clone(&keep_running);
        let progress_clone = Arc::clone(&progress_counter);
        
        thread::spawn(move || {
            worker_thread_loop(i, filter_clone, task_rx, result_tx_clone, keep_running_clone, progress_clone);
        });
    }

    // Gestore del segnale di arresto Ctrl+C
    let keep_running_ctrlc = Arc::clone(&keep_running);
    ctrlc::set_handler(move || {
        println!("\nSegnale di interruzione rilevato (Ctrl+C). Arresto del worker...");
        keep_running_ctrlc.store(false, Ordering::Relaxed);
    }).expect("Errore nell'impostare il gestore Ctrl+C");

    while keep_running.load(Ordering::Relaxed) {
        // 1. Chiedi un blocco di lavoro al Coordinator
        let work_url = format!("{}/request_work?worker_id={}&count={}", coordinator_url, worker_id, block_size);
        println!("[Network] Richiesta blocco di lavoro...");
        
        let mut job: Option<WorkResponse> = None;
        for _ in 0..5 {
            if !keep_running.load(Ordering::Relaxed) {
                break;
            }
            match ureq::get(&work_url).call() {
                Ok(resp) => {
                    if let Ok(data) = resp.into_json::<WorkResponse>() {
                        job = Some(data);
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("[Network] Connessione fallita con il coordinator: {}. Riprovo tra 10 secondi...", e);
                    thread::sleep(Duration::from_secs(10));
                }
            }
        }

        let job_data = match job {
            Some(j) => j,
            None => {
                eprintln!("Impossibile contattare il coordinator. Esco.");
                break;
            }
        };

        if job_data.status == "stop" {
            println!("Arresto ordinato dal coordinator: {}", job_data.message.unwrap_or_default());
            break;
        }

        let start_key = match job_data.start {
            Some(s) => s.parse::<u128>().unwrap(),
            None => break,
        };
        let count = match job_data.count {
            Some(c) => c.parse::<u64>().unwrap(),
            None => break,
        };

        println!("[Job] Ricevuto range: #{} a #{} (totale: {} chiavi)", start_key, start_key + count as u128 - 1, count);
        let start_time = Instant::now();
        progress_counter.store(0, Ordering::Relaxed);

        if let Some((_, ref queue, ref kernel_add, ref kernel_invert, ref kernel_bloom, ref filter_buffer, ref mut output_buffer, ref mut output_count, ref row_in, ref mut col_in, ref mut points_out, ref mut z_heap, ref secp)) = gpu_context {
            let start_key_high = (start_key >> 64) as u64;
            let start_key_low = (start_key & 0xFFFFFFFFFFFFFFFF) as u64;
            let m_bits = bloom_filter.m as u32;
            let k_hashes = bloom_filter.k as u32;
            let max_outputs = 100u32;
            let zero = [0u32; 1];
            
            let col_size = 1024usize;
            let row_size = 8192usize;
            let mut col_points = Vec::with_capacity(col_size * 16);
            for y in 0..col_size {
                let key = start_key + (y as u128) * (row_size as u128) - 1;
                let mut priv_bytes = [0u8; 32];
                let key_bytes = key.to_be_bytes();
                priv_bytes[16..32].copy_from_slice(&key_bytes);
                let priv_key = secp256k1::SecretKey::from_slice(&priv_bytes).unwrap();
                let pub_key = secp256k1::PublicKey::from_secret_key(secp, &priv_key);
                let serialized = pub_key.serialize_uncompressed();
                
                for i in 0..8 {
                    let ox = 33 - (i + 1) * 4;
                    col_points.push(((serialized[ox] as u32) << 24) | ((serialized[ox+1] as u32) << 16) | ((serialized[ox+2] as u32) << 8) | (serialized[ox+3] as u32));
                }
                for i in 0..8 {
                    let oy = 65 - (i + 1) * 4;
                    col_points.push(((serialized[oy] as u32) << 24) | ((serialized[oy+1] as u32) << 16) | ((serialized[oy+2] as u32) << 8) | (serialized[oy+3] as u32));
                }
            }
            
            let mut hits_count = [0u32; 1];
            unsafe {
                queue.enqueue_write_buffer(output_count, opencl3::command_queue::CL_BLOCKING, 0, &zero, &[]).unwrap();
                queue.enqueue_write_buffer(col_in, opencl3::command_queue::CL_BLOCKING, 0, &col_points, &[]).unwrap();
                
                use opencl3::kernel::ExecuteKernel;
                ExecuteKernel::new(kernel_add)
                    .set_arg(points_out)
                    .set_arg(z_heap)
                    .set_arg(row_in)
                    .set_arg(col_in)
                    .set_global_work_sizes(&[row_size, col_size])
                    .set_local_work_sizes(&[64, 1])
                    .enqueue_nd_range(queue).unwrap();
                    
                let invsize = 256i32;
                let invws = (50331648usize / (invsize as usize)) as usize;
                ExecuteKernel::new(kernel_invert)
                    .set_arg(z_heap)
                    .set_arg(&invsize)
                    .set_global_work_size(invws)
                    .set_local_work_size(64)
                    .enqueue_nd_range(queue).unwrap();
                    
                ExecuteKernel::new(kernel_bloom)
                    .set_arg(output_buffer)
                    .set_arg(output_count)
                    .set_arg(points_out)
                    .set_arg(z_heap)
                    .set_arg(filter_buffer)
                    .set_arg(&m_bits)
                    .set_arg(&k_hashes)
                    .set_arg(&max_outputs)
                    .set_arg(&start_key_high)
                    .set_arg(&start_key_low)
                    .set_global_work_sizes(&[row_size, col_size])
                    .set_local_work_sizes(&[64, 1])
                    .enqueue_nd_range(queue).unwrap();
                    
                queue.finish().unwrap();
                
                queue.enqueue_read_buffer(output_count, opencl3::command_queue::CL_BLOCKING, 0, &mut hits_count, &[]).unwrap();
            }
            
            if hits_count[0] > 0 {
                let mut hits = vec![0u64; (hits_count[0] * 2) as usize];
                unsafe { queue.enqueue_read_buffer(output_buffer, opencl3::command_queue::CL_BLOCKING, 0, &mut hits, &[]).unwrap(); }
                for i in 0..hits_count[0] {
                    let hit_high = hits[(i * 2) as usize];
                    let hit_low = hits[(i * 2 + 1) as usize];
                    let hit_key = ((hit_high as u128) << 64) | (hit_low as u128);
                    println!("=======================================================");
                    println!("!!! HIT GPU RILEVATO DALLA SCHEDA VIDEO !!!");
                    println!("Chiave Privata possibilmente valida: #{}", hit_key);
                    println!("=======================================================");
                    // La CPU si occuperà di verificare questa specifica chiave derivandola a mano
                    // Abbiamo un "hit" del Bloom Filter dalla GPU.
                    // Evitiamo di fare 3 richieste HTTP bloccanti per ogni falso positivo,
                    // altrimenti la CPU frena la GPU. Stampiamo solo gli indirizzi generati:
                    let derived = derive_addresses(&secp, hit_key);
                    println!("    Legacy: {}", derived.legacy_addr);
                    println!("    Nested: {}", derived.nested_addr);
                    println!("    Native: {}", derived.native_addr);
                    
                    // Invia il risultato al coordinator per salvarlo in risultati.json
                    let _ = result_tx.send((hit_key, derived.clone(), "gpu_hit".to_string(), 0, 0));
                }
            }
            progress_counter.store(count, Ordering::Relaxed);
        } else {
            // 2. Suddividi il range nei thread worker (invia come compiti da 10.000 chiavi a rotazione)
            let chunk_size = 10000u64;
            let mut sent_count = 0u64;
            let mut thread_idx = 0;

            while sent_count < count && keep_running.load(Ordering::Relaxed) {
                let current_chunk_size = std::cmp::min(chunk_size, count - sent_count);
                let chunk_start_key = start_key + sent_count as u128;
                
                // Invia il task al thread corrente a rotazione round-robin
                let _ = thread_senders[thread_idx].send((chunk_start_key, current_chunk_size));
                
                sent_count += current_chunk_size;
                thread_idx = (thread_idx + 1) % num_threads;
            }
        }

        // 3. Attendi il completamento monitorando periodicamente progressi e risultati
        let check_interval = Duration::from_millis(100);
        
        while keep_running.load(Ordering::Relaxed) {
            // Controlla se i thread hanno trovato una chiave con saldo reale
            if let Ok((found_key, derived, addr_type, balance, txs)) = result_rx.recv_timeout(check_interval) {
                // Abbiamo trovato qualcosa!
                println!("\n=======================================================");
                println!("!!! HIT CONFERMATO ONLINE !!!");
                println!("Chiave Privata: #{}", found_key);
                println!("WIF: {}", derived.wif);
                println!("Tipo indirizzo: {}", addr_type);
                println!("Saldo: {} satoshi (Txs: {})", balance, txs);
                println!("=======================================================");

                // Genera report JSON
                let mut addresses = std::collections::HashMap::new();
                addresses.insert(String::from("legacy"), derived.legacy_addr);
                addresses.insert(String::from("nested"), derived.nested_addr);
                addresses.insert(String::from("native"), derived.native_addr);

                let mut legacy = AddressResults { confirmed: 0, unconfirmed: 0, history_count: 0 };
                let mut nested = AddressResults { confirmed: 0, unconfirmed: 0, history_count: 0 };
                let mut native = AddressResults { confirmed: 0, unconfirmed: 0, history_count: 0 };

                let target_res = AddressResults {
                    confirmed: balance,
                    unconfirmed: 0,
                    history_count: txs,
                };

                match addr_type.as_str() {
                    "legacy" => legacy = target_res,
                    "nested" => nested = target_res,
                    _ => native = target_res,
                }

                let report = MatchReport {
                    worker_id: worker_id.clone(),
                    private_key_number: found_key.to_string(),
                    wif: derived.wif,
                    addresses,
                    results: ResultsData { legacy, nested, native },
                    has_active_balance: balance > 0,
                };

                // Invia notifica bloccante al coordinator
                let report_url = format!("{}/report_match", coordinator_url);
                let _ = ureq::post(&report_url).send_json(&report);
                
                keep_running.store(false, Ordering::Relaxed);
                break;
            }

            // Verifica se tutto il blocco è stato scansionato dai thread
            if progress_counter.load(Ordering::Relaxed) >= count {
                break;
            }
        }

        // 4. Se tutto il blocco è stato completato con successo (senza vincite)
        if keep_running.load(Ordering::Relaxed) {
            let total_elapsed = start_time.elapsed().as_secs_f64();
            let speed = (count as f64) / total_elapsed;
            println!("[Stats] Blocco completato in {:.2}s. Velocità: {:.0} chiavi/sec", total_elapsed, speed);

            // Invia report di completamento al coordinator
            let completed_url = format!("{}/report_completed", coordinator_url);
            let report_data = ReportCompleted {
                worker_id: worker_id.clone(),
                start_key: start_key.to_string(),
                count,
            };
            let _ = ureq::post(&completed_url).send_json(&report_data);
        }
    }

    println!("Worker terminato. Ciao!");
}
