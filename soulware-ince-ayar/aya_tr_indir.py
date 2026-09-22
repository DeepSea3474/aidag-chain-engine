#!/usr/bin/env python3
"""Aya (CohereLabs/aya_dataset, Apache-2.0) Turkce satirlarini HF datasets-server filter
ucundan indirir. Indeks soguk baslarken 'loading' hatasi verir -> bekleyip tekrar dener.
Her sayfa aninda dosyaya eklenir (yarida kalirsa kaldigi yerden devam)."""
import json, os, time, urllib.parse, urllib.request, urllib.error
CIKTI = os.path.join(os.path.dirname(os.path.abspath(__file__)), "ham", "aya_tr.jsonl")
BASE = "https://datasets-server.huggingface.co/filter?"

def cek(off):
    q = urllib.parse.urlencode({"dataset": "CohereLabs/aya_dataset", "config": "default", "split": "train",
                                "where": "\"language_code\"='tur'", "offset": off, "length": 100})
    for deneme in range(40):
        try:
            return json.load(urllib.request.urlopen(BASE + q, timeout=120))
        except urllib.error.HTTPError as e:
            govde = e.read().decode("utf-8", "replace")[:120]
            print(f"  off={off} HTTP {e.code}: {govde} — {15 + deneme * 5}s bekleniyor", flush=True)
        except Exception as e:
            print(f"  off={off} hata: {e}", flush=True)
        time.sleep(15 + deneme * 5)
    raise SystemExit(f"off={off} alinamadi")

mevcut = sum(1 for _ in open(CIKTI)) if os.path.exists(CIKTI) else 0
off = (mevcut // 100) * 100
if off != mevcut:  # yarim sayfa: dosyayi tam sayfaya kirp
    satirlar = open(CIKTI).readlines()[:off]; open(CIKTI, "w").writelines(satirlar)
toplam = None
while toplam is None or off < toplam:
    j = cek(off)
    toplam = j.get("num_rows_total", 0)
    with open(CIKTI, "a") as f:
        for r in j.get("rows", []):
            r = r["row"]
            f.write(json.dumps({"inputs": r["inputs"], "targets": r["targets"],
                                "annotation_type": r["annotation_type"]}, ensure_ascii=False) + "\n")
    off += 100
    print(f"{min(off, toplam)}/{toplam}", flush=True)
print("BITTI", sum(1 for _ in open(CIKTI)), "satir", flush=True)
