#!/usr/bin/env python3
"""AIDAG SDK ornek: stake et + guvenli token kaydet (KALKAN akisi).

Calistirmadan once bir AIDAG dugumu RPC ile ayakta olmali:
    LSC_RPC_ADDR=0.0.0.0:8645 ./target/debug/lsc-node /ip4/0.0.0.0/tcp/40001

Sonra:
    pip install blake3 pynacl requests
    python3 ornek_kalkan.py
"""
import time
from aidag_sdk import AidagClient

# 1. Zincire baglan (kendi anahtarini uretir)
c = AidagClient("http://localhost:8645", network_id=1)
print("Adresin:", c.adres().hex())
print("Zincir durumu:", c.status())


def gonder(payload, etiket):
    """Mevcut uclari parent yapip vertex olustur, imzala, gonder."""
    tips = [bytes.fromhex(t) for t in c.tips().get("tips", [])]
    wire = c.vertex_olustur(parents=tips, payload=payload, timestamp=int(time.time()))
    sonuc = c.submit(wire)
    print(f"  {etiket}: {sonuc.get('sonuc', '')[:30]}")
    time.sleep(1)


# 2. Stake et (teminat yatir — Kalkan icin gerekli)
print("\\n1) Stake ediliyor (1000 birim)...")
gonder(c.stake_payload(c.adres(), 1000), "STAKE")

# 3. Kendi tokenini guvenli kaydet (Kalkan korumali)
print("2) Token kaydediliyor (MYTOKEN)...")
gonder(c.token_payload(c.adres(), "MYTOKEN"), "TOKEN")

# 4. Sonucu gor
print("\\nKayitli tokenlar:", c.tokens())
