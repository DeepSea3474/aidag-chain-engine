#!/usr/bin/env python3
"""AIDAG ornek: kurumsal belge dogrulama akisi (uctan uca).

Senaryo: bir kurum kimligini kaydeder, bir belge uretir + imzalar (zincire
hash'ini yazar), belgeyi karsi tarafa gonderir; karsi taraf belgenin GERCEK
ve DEGISMEMIS oldugunu + hangi KURUMDAN geldigini dogrular.

Once dugum calistir (RPC acik):
    LSC_RPC_ADDR=0.0.0.0:8645 ./target/debug/lsc-node /ip4/0.0.0.0/tcp/40001
Sonra:
    pip install blake3 pynacl requests
    python3 ornek_kurum.py
"""
import time, hashlib
from aidag_sdk import AidagClient, KURUM_DEVLET

# Kurumun sabit anahtari (gercekte kasada saklanir) — kimligi = adresi
kurum = AidagClient("http://localhost:8645", network_id=1,
                    signing_key=bytes([42] * 32))
kurum_adres = kurum.adres().hex()
print("Kurum adresi:", kurum_adres)


def gonder(client, payload):
    tips = [bytes.fromhex(t) for t in client.tips().get("tips", [])]
    wire = client.vertex_olustur(tips, payload, int(time.time()))
    return client.submit(wire).get("sonuc", "")[:25]


# 1) Kurum kimligini kaydet (bir kez yapilir)
print("\n1) Kurum kimligi kaydediliyor...")
print("  ", gonder(kurum, kurum.kurum_payload(KURUM_DEVLET, "Tapu Mudurlugu")))
time.sleep(1)
print("   Kurum kaydi:", kurum.kurum_sorgula(kurum_adres))

# 2) Kurum bir belge uretir ve hash'ini zincire yazar (imzalar)
belge = b"Tapu Senedi No:12345 - Ada:678 Parsel:90 - Malik: Ali Veli"
belge_hash = hashlib.blake2b(belge, digest_size=32).digest()
print("\n2) Belge uretildi, hash zincire yaziliyor...")
print("  ", gonder(kurum, kurum.record_payload(belge_hash)))
time.sleep(1)

# 3) Belge karsi tarafa gonderilir (zincir disi). Karsi taraf DOGRULAR:
print("\n3) Karsi taraf belgeyi dogruluyor...")
dogrula = kurum.belge_dogrula(belge_hash.hex())
if dogrula.get("kayitli"):
    kaydeden = dogrula["kaydeden"]
    kurum_bilgi = kurum.kurum_sorgula(kaydeden)
    print("   Belge GERCEK ve DEGISMEMIS.")
    print("   Kaydeden adres:", kaydeden)
    if kurum_bilgi.get("kayitli"):
        print(f"   Belgeyi ueten kurum: {kurum_bilgi['ad']} ({kurum_bilgi['kategori']})")
else:
    print("   Belge kayitli DEGIL (sahte ya da hic yazilmamis)!")

# 4) Sahtecilik testi: belge degistirilirse dogrulama BASARISIZ
sahte = belge + b" [DEGISTIRILDI]"
sahte_hash = hashlib.blake2b(sahte, digest_size=32).digest()
print("\n4) Degistirilmis belge dogrulaniyor (basarisiz olmali)...")
print("   Sonuc:", "REDDEDILDI (sahtecilik yakalandi)"
      if not kurum.belge_dogrula(sahte_hash.hex()).get("kayitli")
      else "HATA: kabul edildi!")
