import os
import sys
import math
import struct
import time

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
        print("Uso: python generator.py <file_indirizzi.txt> [n_elementi_attesi] [p_falsi_positivi]")
        print("Esempio: python generator.py utxo_addresses.txt 80000000 0.000001")
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
    
    # 2. Legge il file riga per riga (streaming)
    count = 0
    with open(input_file, "r", encoding="utf-8") as f:
        for line in f:
            address = line.strip()
            if address:
                bf.add(address.encode("utf-8"))
                count += 1
                if count % 1000000 == 0:
                    elapsed = time.time() - start_time
                    speed = count / elapsed if elapsed > 0 else 0
                    print(f"Elaborati {count} indirizzi... Velocità: {speed:.0f} ind/sec")

    # 3. Salva su disco
    output_filename = "filter.bin"
    bf.save_to_file(output_filename)
    
    total_time = time.time() - start_time
    print(f"Fatto! Inseriti {count} indirizzi in {total_time:.2f} secondi.")
    print(f"File generato: '{output_filename}' ({os.path.getsize(output_filename) / (1024*1024):.2f} MB)")

if __name__ == "__main__":
    main()
