import sys, time
sys.path.insert(0, "sdk/python")
from aidag_sdk import AidagClient, public_key_to_adres

RPC = "http://localhost:8645"

# Node'un anahtarini oku: [algo_id:1][seed:32]
with open("aidag-key-40001.bin", "rb") as f:
    raw = f.read()
seed = raw[1:33]
assert len(seed) == 32, f"seed 32 bayt olmali, {len(seed)} geldi"

cli = AidagClient(RPC, network_id=1, signing_key=seed)
gonderen = cli.adres()
gonderen_hex = gonderen.hex()
alici = bytes([0xEE]) * 20
alici_hex = alici.hex()

def bakiye(h): return cli.bakiye(h).get("bakiye", 0)
def nonce(h):  return cli.nonce(h)

def transfer(miktar, nonce_deger):
    tips = [bytes.fromhex(t) for t in cli.tips().get("tips", [])]
    payload = AidagClient.transfer_payload(alici, miktar, nonce_deger)
    wire = cli.vertex_olustur(tips, payload, int(time.time()))
    return cli.submit(wire)

print("=== BASLANGIC ===")
print("gonderen:", gonderen_hex)
print("gonderen bakiye:", bakiye(gonderen_hex), "| nonce:", nonce(gonderen_hex))
print("alici   bakiye:", bakiye(alici_hex))

print()
print("=== 1) nonce=0 ile transfer (300) ===")
print("sonuc:", transfer(300, 0))
time.sleep(1)
print("gonderen bakiye:", bakiye(gonderen_hex), "| nonce:", nonce(gonderen_hex))
print("alici   bakiye:", bakiye(alici_hex))

print()
print("=== 2) REPLAY: ayni nonce=0 ile tekrar (300) -> REDDEDILMELI ===")
print("sonuc:", transfer(300, 0))
time.sleep(1)
print("gonderen bakiye:", bakiye(gonderen_hex), "| nonce:", nonce(gonderen_hex))
print("alici   bakiye:", bakiye(alici_hex), "(degismediyse replay engellendi)")

print()
print("=== 3) nonce=1 ile yeni transfer (200) -> BASARILI ===")
print("sonuc:", transfer(200, 1))
time.sleep(1)
print("gonderen bakiye:", bakiye(gonderen_hex), "| nonce:", nonce(gonderen_hex))
print("alici   bakiye:", bakiye(alici_hex))

print()
print("=== 4) yanlis nonce=5 ile transfer (100) -> REDDEDILMELI ===")
print("sonuc:", transfer(100, 5))
time.sleep(1)
print("gonderen bakiye:", bakiye(gonderen_hex), "| nonce:", nonce(gonderen_hex))
print("alici   bakiye:", bakiye(alici_hex))

print()
print("=== BEKLENEN SONUC ===")
print("gonderen 500, alici 500, nonce 2 olmali (sadece 2 gecerli transfer islendi)")
