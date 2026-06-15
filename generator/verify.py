import sys
import struct
import math

def fnv1a_64(data: bytes) -> int:
    h = 14695981039346656037
    for byte in data:
        h = h ^ byte
        h = (h * 1099511628211) & 0xffffffffffffffff
    return h

def get_hashes(data: bytes):
    h = fnv1a_64(data)
    h1 = h & 0xffffffff
    h2 = (h >> 32) & 0xffffffff
    return h1, h2

class BloomFilterLoader:
    def __init__(self, filename: str):
        print(f"Caricamento Bloom Filter da '{filename}'...")
        with open(filename, "rb") as f:
            # Leggi m (uint64, 8 byte) e k (uint32, 4 byte)
            header = f.read(12)
            if len(header) < 12:
                raise ValueError("Errore: file del filtro troppo corto o non valido.")
            self.m, self.k = struct.unpack("<QI", header)
            
            # Leggi l'intero array di bit
            self.bit_array = f.read()
            self.byte_size = len(self.bit_array)
            
            # Verifica consistenza dimensione
            expected_byte_size = (self.m + 7) // 8
            if self.byte_size != expected_byte_size:
                raise ValueError(f"Dimensione dell'array non coerente. Atteso: {expected_byte_size} byte, Ricevuto: {self.byte_size} byte")
            
        print(f"Filtro caricato con successo: m = {self.m} bit ({self.byte_size / (1024*1024):.4f} MB), k = {self.k} hash")

    def check(self, item: bytes) -> bool:
        h1, h2 = get_hashes(item)
        for i in range(self.k):
            bit_pos = (h1 + i * h2) % self.m
            byte_idx = bit_pos // 8
            bit_offset = bit_pos % 8
            if not (self.bit_array[byte_idx] & (1 << bit_offset)):
                return False
        return True

def main():
    filter_file = "filter.bin"
    test_file = "test_addresses.txt"
    
    try:
        bf = BloomFilterLoader(filter_file)
    except Exception as e:
        print(f"Errore durante il caricamento del filtro: {e}")
        sys.exit(1)
        
    print("\n--- TEST DI VERIFICA ---")
    
    # 1. Verifica indirizzi inseriti
    success = True
    print("Verifica indirizzi che DEVONO essere presenti:")
    with open(test_file, "r", encoding="utf-8") as f:
        for line in f:
            address = line.strip()
            if address:
                is_present = bf.check(address.encode("utf-8"))
                status = "PRESENTE (OK)" if is_present else "NON PRESENTE (ERRORE!)"
                print(f" * {address}: {status}")
                if not is_present:
                    success = False
                    
    # 2. Verifica indirizzi che NON devono essere presenti
    print("\nVerifica indirizzi che NON devono essere presenti:")
    not_inserted = [
        "1BitcoinEaterAddressDontSendf59kuE",
        "bc1qinvalidaddress0000000000000000000000",
        "3EktnHQD7Ri8tN5fC8yQ6YWd8n6pPAG9fA"
    ]
    for address in not_inserted:
        is_present = bf.check(address.encode("utf-8"))
        status = "PRESENTE (FALSO POSITIVO)" if is_present else "ASSENTE (OK)"
        print(f" * {address}: {status}")
        
    if success:
        print("\n=== TEST SUPERATO CON SUCCESSO! ===")
    else:
        print("\n=== TEST FALLITO: Rilevato falso negativo! ===")

if __name__ == "__main__":
    main()
