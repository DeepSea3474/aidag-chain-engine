import sys, time
sys.path.insert(0, "sdk/python")
from aidag_sdk import AidagClient

RPC = "http://localhost:8645"
seed = open("aidag-key-40001.bin", "rb").read()[1:33]
cli = AidagClient(RPC, network_id=1, signing_key=seed)

gonderen = cli.adres()
gonderen_hex = gonderen.hex()
hedef = bytes([0xEE]) * 20
hedef_hex = hedef.hex()
yakim_hex = "00" * 20
havuz_hex = "6eab1d6c2e5f708192a3b4c5d6e7f8091a2b3c4d"  # GELISTIRME_HAVUZU (avm.rs)

import requests as _rq
def lsc(h):
    return _rq.get(RPC + "/lsc-bakiye/" + h).json().get("lsc_bakiye", 0)
def nonce(h):
    return cli.nonce(h)

# AvmCagri payload: [9][hedef:20][deger:8 BE][nonce:8 BE]
TX_TYPE_AVM_CAGRI = 9
def avm_payload(hedef20, deger, n):
    import struct
    return bytes([TX_TYPE_AVM_CAGRI]) + hedef20 + struct.pack(">Q", deger) + struct.pack(">Q", n)

def avm_cagri(deger, n):
    tips = [bytes.fromhex(t) for t in cli.tips().get("tips", [])]
    payload = avm_payload(hedef, deger, n)
    wire = cli.vertex_olustur(tips, payload, int(time.time()))
    return cli.submit(wire)

print("=== AVM CAGRISI CANLI TEST ===")
print("gonderen:", gonderen_hex)
print()
print("--- ONCE ---")
print("gonderen LSC:", lsc(gonderen_hex), "| nonce:", nonce(gonderen_hex))
print("hedef    LSC:", lsc(hedef_hex))
print("yakim    LSC:", lsc(yakim_hex))
print("havuz    LSC:", lsc(havuz_hex))

print()
print("AVM cagrisi: hedefe 1000 LSC, nonce=0, gas=21000 (10500 yak + 10500 havuz)")
print("sonuc:", avm_cagri(1000, 0))
time.sleep(1)

print()
print("--- SONRA ---")
print("gonderen LSC:", lsc(gonderen_hex), "(100000 - 1000 - 21000 = 78000 olmali)")
print("hedef    LSC:", lsc(hedef_hex), "(1000 olmali)")
print("yakim    LSC:", lsc(yakim_hex), "(10500 olmali)")
print("havuz    LSC:", lsc(havuz_hex), "(10500 olmali)")
print("nonce:", nonce(gonderen_hex), "(1 olmali)")
