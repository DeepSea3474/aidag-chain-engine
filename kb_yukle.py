import json, re, urllib.request, os
KB = "http://127.0.0.1:8646/kb/ingest"
DOSYALAR = ["README.md","ON_SATIS_PLANI.md","TOKENOMICS_TASARIM.md","GENESIS_DAGITIM.md",
            "YOL_HARITASI.md","EKOSISTEM_VIZYONU.md","MAINNET_TEKONOMIK.md","TESTLER.md",
            "DUGUM_CALISTIRMA.md","NOTLAR_BILINEN_SINIRLAR.md"]
MAX = 3000
def bol(ad, t):
    p = re.split(r'^(##+\s+.+)$', t, flags=re.M)
    if len(p) <= 1:
        for i in range(0, len(t), MAX): yield (ad, t[i:i+MAX])
        return
    if p[0].strip(): yield (ad, p[0].strip())
    for i in range(1, len(p), 2):
        bas = p[i].strip('# ').strip()
        icerik = (bas + "\n" + (p[i+1] if i+1 < len(p) else "")).strip()
        for j in range(0, len(icerik), MAX):
            yield (f"{ad} - {bas}", icerik[j:j+MAX])
def gonder(bas, met):
    veri = json.dumps({"baslik": bas, "metin": met}).encode()
    r = urllib.request.Request(KB, data=veri, headers={"Content-Type":"application/json"}, method="POST")
    with urllib.request.urlopen(r, timeout=60) as y: return json.loads(y.read())
toplam = 0
for d in DOSYALAR:
    if not os.path.exists(d):
        print(f"  ATLA (yok): {d}"); continue
    metin = open(d, encoding="utf-8").read()
    n = 0
    for bas, parca in bol(d, metin):
        if len(parca.strip()) < 40: continue
        try:
            if gonder(bas, parca).get("ok"): n += 1; toplam += 1
        except Exception as e: print(f"    HATA {bas}: {e}")
    print(f"  {d}: {n} parca")
print(f"\nTOPLAM: {toplam}")
