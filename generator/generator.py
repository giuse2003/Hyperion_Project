import os
import sys
import math
import struct
import time
import gzip
import base58

CHARSET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"

def bech32_polymod(values):
    generator = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]
    chk = 1
    for value in values:
        top = chk >> 25
        chk = (chk & 0x1ffffff) << 5 ^ value
        for i in range(5):
            chk ^= generator[i] if ((top >> i) & 1) else 0
    return chk

def bech32_hrp_expand(hrp):
    return [ord(x) >> 5 for x in hrp] + [0] + [ord(x) & 31 for x in hrp]

def bech32_decode(bech):
    if (any(ord(x) < 33 or ord(x) > 126 for x in bech)) or (bech.lower() != bech and bech.upper() != bech):
        return (None, None)
    bech = bech.lower()
    pos = bech.rfind('1')
    if pos < 1 or pos + 7 > len(bech) or len(bech) > 90:
        return (None, None)
    if not all(x in CHARSET for x in bech[pos+1:]):
        return (None, None)
    hrp = bech[:pos]
    data = [CHARSET.find(x) for x in bech[pos+1:]]
    if bech32_polymod(bech32_hrp_expand(hrp) + data) != 1:
        return (None, None)
    return (hrp, data[:-6])

def convertbits(data, frombits, tobits, pad=True):
    acc = 0
    bits = 0
    ret = []
    maxv = (1 << tobits) - 1
    max_acc = (1 << (frombits + tobits - 1)) - 1
    for value in data:
        if value < 0 or (value >> frombits):
            return None
        acc = ((acc << frombits) | value) & max_acc
        bits += frombits
        while bits >= tobits:
            bits -= tobits
            ret.append((acc >> bits) & maxv)
    if pad:
        if bits:
            ret.append((acc << (tobits - bits)) & maxv)
    elif bits >= frombits or ((acc << (tobits - bits)) & maxv):
        return None
    return ret

def decode_address_to_payload(address: str) -> bytes:
    """Decodifica un indirizzo Bitcoin estraendo il payload binario a 20 byte."""
    try:
        if address.startswith('1') or address.startswith('3'):
            decoded = base58.b58decode_check(address)
            if len(decoded) == 21:
                return decoded[1:21]
        elif address.lower().startswith('bc1q'):
            hrp, data = bech32_decode(address)
            if data is not None and hrp == 'bc' and data[0] == 0:
                witness_program = convertbits(data[1:], 5, 8, False)
                if witness_program is not None and len(witness_program) == 20:
                    return bytes(witness_program)
    except Exception:
        pass
    return None


def fnv1a_64(data: bytes) -> int:
    """Hash rapido FNV-1a a 64 bit."""
    h = 14695981039346656037
    for byte in data:
        h = h ^ byte
        h = (h * 1099511628211) & 0xffffffffffffffff
    return h

def get_hashes(data: bytes):
    """Genera due hash a 32 bit a partire da un singolo hash FNV-1a a 64 bit."""
    h = fnv1a_64(data)
    h1 = h & 0xffffffff
    h2 = (h >> 32) & 0xffffffff
    return h1, h2

class BloomFilter:
    def __init__(self, n: int, p: float):
        """
        n: numero atteso di elementi nel filtro (es. 80.000.000)
        p: probabilità accettabile di falso positivo (es. 0.000001 per 1 su un milione)
        """
        # Calcolo dimensione ottimale dell'array in bit (m)
        self.m = int(math.ceil(-n * math.log(p) / (math.log(2) ** 2)))
        
        # Calcolo del numero ottimale di funzioni hash da usare (k)
        self.k = int(round((self.m / n) * math.log(2)))
        
        # Arrotondiamo m a un multiplo di 8 per memorizzarlo in un bytearray
        self.byte_size = (self.m + 7) // 8
        self.m = self.byte_size * 8
        
        # Alloca l'array di byte in memoria
        self.bit_array = bytearray(self.byte_size)
        
        print(f"[BloomFilter] Inizializzato: m = {self.m} bit ({self.byte_size / (1024*1024):.2f} MB), k = {self.k} hash")

    def add(self, item: bytes):
        """Aggiunge un elemento (indirizzo o scripthash) al filtro."""
        h1, h2 = get_hashes(item)
        for i in range(self.k):
            bit_pos = (h1 + i * h2) % self.m
            byte_idx = bit_pos // 8
            bit_offset = bit_pos % 8
            self.bit_array[byte_idx] |= (1 << bit_offset)

    def check(self, item: bytes) -> bool:
        """Verifica se un elemento è presente nel filtro (può dare falsi positivi, mai falsi negativi)."""
        h1, h2 = get_hashes(item)
        for i in range(self.k):
            bit_pos = (h1 + i * h2) % self.m
            byte_idx = bit_pos // 8
            bit_offset = bit_pos % 8
            if not (self.bit_array[byte_idx] & (1 << bit_offset)):
                return False
        return True

    def save_to_file(self, filename: str):
        """Salva il Bloom Filter su disco in formato binario cross-compatible (Go/Rust/C++)."""
        print(f"Salvataggio del filtro in '{filename}'...")
        with open(filename, "wb") as f:
            # Header: m (uint64, 8 byte) + k (uint32, 4 byte)
            header = struct.pack("<QI", self.m, self.k)
            f.write(header)
            # Dati dell'array di bit
            f.write(self.bit_array)
        print("Salvataggio completato.")

def main():
    if len(sys.argv) < 2:
        print("Uso: python generator.py <file_indirizzi.txt o .gz> [n_elementi_attesi] [p_falsi_positivi]")
        print("Esempio: python generator.py blockchair_bitcoin_addresses_and_balance_LATEST.tsv.gz 80000000 0.000001")
        sys.exit(1)

    input_file = sys.argv[1]
    
    # Parametri di default
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 80000000
    p = float(sys.argv[3]) if len(sys.argv) > 3 else 0.000001 # 1 su un milione

    if not os.path.exists(input_file):
        print(f"Errore: il file di input '{input_file}' non esiste.")
        sys.exit(1)

    print(f"Caricamento indirizzi da '{input_file}'...")
    start_time = time.time()
    
    # 1. Inizializza il Bloom Filter
    bf = BloomFilter(n, p)
    
    # 2. Rileva se il file è compresso (.gz)
    is_gzip = input_file.endswith(".gz")
    open_func = gzip.open if is_gzip else open
    mode = "rt" if is_gzip else "r"
    
    # 3. Legge il file riga per riga (streaming)
    count = 0
    skipped_headers = 0
    
    with open_func(input_file, mode, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            
            # Se la riga contiene una tabulazione, assumiamo sia in formato TSV (address \t balance)
            if "\t" in line:
                parts = line.split("\t")
                address = parts[0].strip()
                # Salta l'intestazione se presente (es. "address")
                if address.lower() == "address":
                    skipped_headers += 1
                    continue
            else:
                address = line
                
            if address:
                payload = decode_address_to_payload(address)
                if payload is not None:
                    bf.add(payload)
                    count += 1
                    if count % 1000000 == 0:
                        elapsed = time.time() - start_time
                        speed = count / elapsed if elapsed > 0 else 0
                        print(f"Elaborati {count} indirizzi validi... Velocità: {speed:.0f} ind/sec")

    # 4. Salva su disco
    output_filename = "filter.bin"
    bf.save_to_file(output_filename)
    
    total_time = time.time() - start_time
    print(f"Fatto! Inseriti {count} indirizzi (saltate {skipped_headers} intestazioni) in {total_time:.2f} secondi.")
    print(f"File generato: '{output_filename}' ({os.path.getsize(output_filename) / (1024*1024):.2f} MB)")

if __name__ == "__main__":
    main()
