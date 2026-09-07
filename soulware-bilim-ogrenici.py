#!/usr/bin/env python3
"""
SoulwareAI — sürekli BİLİMSEL öğrenici (KUBRA'nın ANA kaynağı).

Ana kaynak: hakemli açık-erişim literatür — OpenAlex (WOS'un açık/ücretsiz muadili).
Kalite süzgeci: DOAJ-onaylı dergi (denetlenmiş, yağmacı değil) + CC-BY/CC0 lisans.
Her belge canlı KUBRA'ya /kb/ingest ile ARTIMLI eklenir (kesintisiz, kalıcı, embed dahil).

ÖLÇEK İLKESİ: öğrenme hızı ağın kapasitesiyle orantılı.
  - CPU-only (az işçi)  → nazik (küçük batch, uzun bekleme)  → 100'ler/gün
  - GPU işçileri katıldıkça → batch büyür → yüz binlere doğru
  (Not: bugün embedding sunucu CPU'sunda; gerçek yüz-binler için dağıtık embedding + ANN indeksi gerekir.)

Durum (görülen DOI + her alan için imleç) diske yazılır → yeniden başlarsa KALDIĞI YERDEN devam.
Yani "diğer kaynakları da yavaş yavaş öğrenerek hafızasına alır."
"""
import json, os, time, urllib.parse, urllib.request

MAILTO   = "akyuzaydin7434@gmail.com"
BRAIN    = os.environ.get("SOULWARE_BRAIN_URL", "http://127.0.0.1:8646")
COORD    = os.environ.get("SOULWARE_COORD_URL", "http://127.0.0.1:8647")
DURUM    = os.environ.get("SOULWARE_BILIM_DURUM", "/root/aidag-lsc/soulware-knowledge/bilim-durum.json")
BEKLE    = int(os.environ.get("SOULWARE_BILIM_BEKLE", "240"))       # batch arası saniye (nazik)
BATCH_TABAN = int(os.environ.get("SOULWARE_BILIM_BATCH", "15"))     # taban batch (CPU-only)
BATCH_TAVAN = int(os.environ.get("SOULWARE_BILIM_BATCH_TAVAN", "200"))  # GPU gelince yükselt
ISCI_KATSAYI = int(os.environ.get("SOULWARE_BILIM_ISCI_KATSAYI", "20"))  # her işçi +N belge

# Bize yakın + genel bilim (imleç ilerledikçe her alanda derine iner = yavaş yavaş öğrenir)
ALANLAR = [
    "artificial intelligence", "machine learning", "large language models", "deep learning",
    "neural networks", "natural language processing", "reinforcement learning", "computer vision",
    "blockchain", "distributed systems", "cryptography", "consensus algorithm", "peer to peer networks",
    "GPU computing", "parallel computing", "databases", "operating systems", "computer networks",
    "mathematics", "statistics", "optimization", "information theory",
    "physics", "chemistry", "biology", "medicine", "genetics", "neuroscience",
    "economics", "energy", "climate", "materials science", "robotics",
]

SERBEST_ONEK = ("cc-by", "cc0", "public-domain")
YASAK = ("nc", "nd")   # non-commercial / no-derivatives → ELE

def serbest(l):
    l = (l or "").lower()
    if not l or any(y in l for y in YASAK):
        return False
    return any(l.startswith(o) for o in SERBEST_ONEK)

def abs_coz(inv):
    if not inv:
        return ""
    k = {}
    for w, ys in inv.items():
        for y in ys:
            k[y] = w
    return " ".join(k[i] for i in sorted(k))

def get_json(url, timeout=40, data=None):
    hdr = {"User-Agent": f"SoulwareAI-KUBRA/0.1 (mailto:{MAILTO})"}
    if data is not None:
        data = json.dumps(data).encode(); hdr["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=hdr)
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.load(r)

def durum_yukle():
    try:
        d = json.load(open(DURUM)); return set(d.get("gorulen", [])), d.get("imlec", {})
    except Exception:
        return set(), {}

def durum_kaydet(gorulen, imlec):
    tmp = DURUM + ".tmp"
    json.dump({"gorulen": list(gorulen), "imlec": imlec}, open(tmp, "w"), ensure_ascii=False)
    os.replace(tmp, DURUM)

def kapasite_batch():
    """Batch boyu. DÜRÜST NOT: embedding şu an SUNUCU-CPU'da yapılıyor — cevap veren
    işçiler embedding kapasitesi EKLEMEZ. O yüzden batch KÜÇÜK sabit tutulur ki
    kullanıcı sorgularını yavaşlatmasın. Gerçek ölçekleme (yüz binler), 'embed-yetenekli'
    GPU işçileri (dağıtık embedding) gelince açılacak — o zaman burası ona bağlanır."""
    try:
        s = get_json(COORD + "/status", timeout=8)
        aktif = len([w for w in s.get("workers", []) if w.get("jobs_done", 0) > 0])  # gerçek katkıcı (bilgi amaçlı)
    except Exception:
        aktif = 0
    return BATCH_TABAN, aktif

def sayfa(query, cursor):
    p = {
        "search": query,
        "filter": ("open_access.is_oa:true,has_abstract:true,"
                   "primary_location.source.is_in_doaj:true,from_publication_date:2015-01-01"),
        "sort": "cited_by_count:desc", "per-page": 50, "cursor": cursor or "*", "mailto": MAILTO,
    }
    return get_json("https://api.openalex.org/works?" + urllib.parse.urlencode(p))

def main():
    gorulen, imlec = durum_yukle()
    print(f"🔬 bilimsel öğrenici başladı | görülen: {len(gorulen)} | ana kaynak: OpenAlex (DOAJ+CC-BY)", flush=True)
    ai = 0
    while True:
        batch, isci = kapasite_batch()
        alan = ALANLAR[ai % len(ALANLAR)]; ai += 1
        try:
            veri = sayfa(alan, imlec.get(alan))
        except Exception as e:
            print(f"[{alan}] API hata: {e} — bekleniyor", flush=True); time.sleep(BEKLE); continue
        imlec[alan] = veri.get("meta", {}).get("next_cursor") or "*"
        eklendi = 0
        for w in veri.get("results", []):
            if eklendi >= batch:
                break
            loc = w.get("best_oa_location") or w.get("primary_location") or {}
            if not serbest(loc.get("license")):
                continue
            doi = w.get("doi") or w.get("id") or ""
            if not doi or doi in gorulen:
                continue
            ab = abs_coz(w.get("abstract_inverted_index")); bas = (w.get("title") or "").strip()
            if not bas or len(ab) < 200:
                continue
            try:
                r = get_json(BRAIN + "/kb/ingest", timeout=90,
                             data={"baslik": bas[:220], "metin": f"{bas}. {ab}"[:2200], "url": doi})
                if r.get("ok"):
                    gorulen.add(doi); eklendi += 1
                    time.sleep(0.4)   # depo kilidini bırak → kullanıcı sorgusu araya girsin (yavaşlatma)
            except Exception as e:
                print(f"ingest hata: {e}", flush=True); time.sleep(5)
        durum_kaydet(gorulen, imlec)
        print(f"[{alan}] +{eklendi} (batch={batch}, işçi={isci}) | toplam görülen: {len(gorulen)}", flush=True)
        time.sleep(BEKLE)

if __name__ == "__main__":
    main()
