import sys, time, struct, requests as rq
sys.path.insert(0, "sdk/python")
from aidag_sdk import AidagClient

RPC = "http://localhost:8645"
seed = open("aidag-key-40001.bin", "rb").read()[1:33]
cli = AidagClient(RPC, network_id=1, signing_key=seed)
gonderen = cli.adres(); gonderen_hex = gonderen.hex()

def lsc(h): return rq.get(RPC + "/lsc-bakiye/" + h).json().get("lsc_bakiye", 0)
def nonce(h): return cli.nonce(h)

TX_AVM = 9
def avm_payload(hedef20, deger, n, data):
    return (bytes([TX_AVM]) + hedef20 + struct.pack(">Q", deger)
            + struct.pack(">Q", n) + struct.pack(">I", len(data)) + data)

def gonder(hedef20, deger, n, data):
    tips = [bytes.fromhex(t) for t in cli.tips().get("tips", [])]
    payload = avm_payload(hedef20, deger, n, data)
    wire = cli.vertex_olustur(tips, payload, int(time.time()))
    return cli.submit(wire)

bin_hex = open("avm-sozlesmeler/BelgeDamgasi_sol_BelgeDamgasi.bin").read().strip()
deploy_kod = bytes.fromhex(bin_hex)

print("=== AVM KONTRAT CANLI TEST (BelgeDamgasi) ===")
print("gonderen:", gonderen_hex)
print("gonderen LSC:", lsc(gonderen_hex), "| nonce:", nonce(gonderen_hex))

print("\n-- LSC test bakiye ekleniyor (gas icin) --")
try:
    print(rq.post(RPC + "/lsc_test_bakiye", json={"adres": gonderen_hex, "miktar": 1000000}).json())
except Exception as e:
    print("test_bakiye:", e)
time.sleep(1)
print("gonderen LSC:", lsc(gonderen_hex))

n = nonce(gonderen_hex)
print(f"\n-- DEPLOY (hedef=sifir, data=bytecode, nonce={n}) --")
sifir = bytes(20)
print("sonuc:", gonder(sifir, 0, n, deploy_kod))
time.sleep(1)
print("nonce sonrasi:", nonce(gonderen_hex), "(", n+1, "olmali)")
print("gonderen LSC:", lsc(gonderen_hex), "(gas 21000 dusmus olmali)")
