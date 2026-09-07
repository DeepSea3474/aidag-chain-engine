#!/usr/bin/env python3
"""
SoulwareAI — OpenAlex açık-erişim makale toplayıcı (KUBRA onaylı-kaynak katmanı).

Sadece SERBEST lisanslı (CC-BY / CC-BY-SA / CC0 / kamu malı) hakemli makaleleri alır.
Her belgeye kaynak izi (DOI, lisans, yıl) iliştirir — KUBRA cevabında kaynak gösterebilsin,
kullanıcı doğrulayabilsin. Ticari-uyumsuz (NC) ve türev-yasak (ND) lisanslar ELENİR.

Kullanım:
  python3 soulware-oa-toplayici.py "arama sorgusu" [adet] [cikti.json]
Örn:
  python3 soulware-oa-toplayici.py "artificial intelligence" 40
"""
import json, sys, urllib.parse, urllib.request

MAILTO = "akyuzaydin7434@gmail.com"          # OpenAlex "kibar havuz" (daha hızlı, ücretsiz)
BASE = "https://api.openalex.org/works"

# Ticari kullanıma + türev/işlemeye izin veren lisanslar (bizde token ekonomisi var).
SERBEST_ONEK = ("cc-by", "cc0", "public-domain", "pd")
YASAK_PARCA = ("nc", "nd")                    # non-commercial / no-derivatives → ELE

def abstract_coz(inv):
    """OpenAlex abstract_inverted_index (kelime->konumlar) -> düz metin."""
    if not inv:
        return ""
    konum = {}
    for kelime, yerler in inv.items():
        for y in yerler:
            konum[y] = kelime
    return " ".join(konum[i] for i in sorted(konum))

def serbest_mi(lisans):
    """CC-BY / CC-BY-SA / CC0 / kamu malı = evet. NC/ND içerenler = hayır."""
    l = (lisans or "").lower().strip()
    if not l:
        return False
    if any(p in l for p in YASAK_PARCA):      # cc-by-nc, cc-by-nc-nd, cc-by-nd ...
        return False
    return any(l.startswith(o) for o in SERBEST_ONEK)

def getir(sorgu, adet, yil_min=2015):
    p = {
        "search": sorgu,
        "filter": f"open_access.is_oa:true,has_abstract:true,from_publication_date:{yil_min}-01-01",
        "per-page": min(max(adet * 3, 50), 200),   # lisans elemesi için fazla çek
        "sort": "cited_by_count:desc",              # en çok atıf alan (güvenilir) önce
        "mailto": MAILTO,
    }
    url = BASE + "?" + urllib.parse.urlencode(p)
    req = urllib.request.Request(url, headers={"User-Agent": f"SoulwareAI-KUBRA/0.1 (mailto:{MAILTO})"})
    with urllib.request.urlopen(req, timeout=40) as r:
        return json.load(r)

def lisans_al(w):
    loc = w.get("best_oa_location") or w.get("primary_location") or {}
    return (loc.get("license") or "").lower()

def main():
    sorgu = sys.argv[1] if len(sys.argv) > 1 else "artificial intelligence"
    adet  = int(sys.argv[2]) if len(sys.argv) > 2 else 40
    cikti = sys.argv[3] if len(sys.argv) > 3 else "soulware-knowledge/kb.scholar.json"

    veri = getir(sorgu, adet)
    sonuc = veri.get("results", [])
    belgeler, elenen_lisans, gorulen = [], 0, set()
    for w in sonuc:
        lis = lisans_al(w)
        if not serbest_mi(lis):
            elenen_lisans += 1
            continue
        ab = abstract_coz(w.get("abstract_inverted_index"))
        baslik = (w.get("title") or "").strip()
        if not baslik or len(ab) < 200:        # metni zayıf olanı alma
            continue
        anahtar = baslik.lower().strip()       # tekrar (aynı makale) elemesi
        if anahtar in gorulen:
            continue
        gorulen.add(anahtar)
        doi = w.get("doi") or w.get("id") or ""
        belgeler.append({
            "baslik": baslik[:220],
            "metin": f"{baslik}. {ab}"[:2200],
            "url": doi,
            "lisans": lis,
            "yil": w.get("publication_year"),
            "atif": w.get("cited_by_count"),
            "kaynak_tipi": "hakemli-oa",       # onaylı katman etiketi
            "guvenilir": True,
        })
        if len(belgeler) >= adet:
            break

    with open(cikti, "w") as f:
        json.dump(belgeler, f, ensure_ascii=False, indent=1)

    print(f"sorgu   : {sorgu!r}")
    print(f"eslesme : {veri.get('meta',{}).get('count'):,} (toplam OA)")
    print(f"cekilen : {len(sonuc)} | lisans-elenen: {elenen_lisans} | ALINAN (serbest): {len(belgeler)}")
    print(f"cikti   : {cikti}")
    for b in belgeler[:6]:
        print(f"  [{b['lisans']}] ({b['yil']}, atıf {b['atif']}) {b['baslik'][:66]}")

if __name__ == "__main__":
    main()
