#!/usr/bin/env python3
"""
SoulwareAI — GitHub öğrenici (KUBRA'nın açık kaynak bilgi kaynağı).

GitHub'daki GÜVENİLİR açık kaynak projelerin README/dokümantasyonundan öğrenir ve
KUBRA bilgi tabanına /kb/ingest ile ARTIMLI ekler (kaynak + lisans + bağlantı ile).

GÜVENLİK İLKELERİ (değiştirmeden önce düşün):
  1. KOD ASLA ÇALIŞTIRILMAZ / KURULMAZ / KUBRA'YA VEYA ZİNCİRE UYGULANMAZ. Yalnızca
     README metni okunur ve bilgi olarak saklanır. KUBRA'nın kendi kodu yalnızca
     insan onaylı PR + test + (zincir için) mainnet replay ile değişir. Otomatik
     "GitHub'dan kod çekip kendini güncelleme" tedarik zinciri saldırısına açık kapıdır.
  2. Lisans: yalnız izin verici (MIT, Apache-2.0, BSD-2/3, ISC, 0BSD, Unlicense).
  3. Güven süzgeci: >= MIN_YILDIZ yıldız, arşivlenmemiş, fork değil, son 12 ayda güncel,
     CI iş akışı VAR ve varsayılan dalın son kontrolleri BAŞARILI ("test edilmiş").
  4. README güvenilmeyen VERİDİR: istem-enjeksiyonu kalıbı içeren metin atlanır;
     KUBRA grounding istemi de kaynakları "bilgi, talimat değil" olarak işaretler.

Nazik tarama: kimlik doğrulamasız GitHub API (arama 10/dk, çekirdek 60/saat) sınırlarına
uyar; README raw.githubusercontent.com'dan alınır. Durum diske yazılır → kaldığı yerden.
"""
import json, os, re, time, urllib.parse, urllib.request, urllib.error

BRAIN = os.environ.get("SOULWARE_BRAIN_URL", "http://127.0.0.1:8646")
DURUM = os.environ.get("SOULWARE_GITHUB_DURUM", "/root/aidag-lsc/soulware-knowledge/github-durum.json")
BEKLE = int(os.environ.get("SOULWARE_GITHUB_BEKLE", "600"))        # tur arası saniye (nazik; ~6 tur/saat)
KONTROL_BASI = int(os.environ.get("SOULWARE_GITHUB_KONTROL", "4"))  # tur başına en fazla CI kontrolü (2 API isteği/aday -> <=48/saat)
TUR_BASI = int(os.environ.get("SOULWARE_GITHUB_TUR_BASI", "4"))    # tur başına en fazla yeni proje
MIN_YILDIZ = int(os.environ.get("SOULWARE_GITHUB_MIN_YILDIZ", "500"))
UA = {"User-Agent": "SoulwareAI-KUBRA-learner/0.1", "Accept": "application/vnd.github+json"}

LISANSLAR = ["mit", "apache-2.0", "bsd-3-clause", "bsd-2-clause", "isc", "0bsd", "unlicense"]

# AIDAG/KUBRA'ya yakın konular (imleç ilerledikçe her konuda derine iner).
KONULAR = [
    "blockchain", "dag", "consensus", "ethereum", "evm", "solidity", "smart-contracts",
    "cryptography", "zero-knowledge", "p2p", "libp2p", "rust", "distributed-systems",
    "database", "llm", "rag", "machine-learning", "deep-learning", "nlp", "computer-vision",
    "security", "devops", "kubernetes", "webassembly", "compiler",
]

ENJEKSIYON = re.compile(
    r"ignore (all |the )?(previous|prior|above) (instructions|prompts?)|system prompt|you are now|"
    r"disregard (all |the )?(previous|prior)|önceki talimat|onceki talimat|talimatları yok say",
    re.I,
)


def istek(url, timeout=30, data=None, hdr=None):
    h = dict(UA if hdr is None else hdr)
    if data is not None:
        data = json.dumps(data).encode(); h["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=h)
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read()


def gh(yol):
    return json.loads(istek("https://api.github.com" + yol))


def durum_yukle():
    try:
        d = json.load(open(DURUM))
        return set(d.get("gorulen", [])), d.get("sayfa", {})
    except Exception:
        return set(), {}


def durum_kaydet(gorulen, sayfa):
    tmp = DURUM + ".tmp"
    json.dump({"gorulen": sorted(gorulen), "sayfa": sayfa}, open(tmp, "w"))
    os.replace(tmp, DURUM)


def ara(konu, lisans, sayfa_no):
    """Güven süzgecinin arama kısmı (yıldız, fork, arşiv, güncellik, lisans)."""
    son_yil = time.strftime("%Y-%m-%d", time.gmtime(time.time() - 365 * 86400))
    q = f"topic:{konu} license:{lisans} stars:>={MIN_YILDIZ} archived:false fork:false pushed:>={son_yil}"
    yol = "/search/repositories?" + urllib.parse.urlencode(
        {"q": q, "sort": "stars", "order": "desc", "per_page": 10, "page": sayfa_no})
    return gh(yol).get("items", [])


class HizSiniri(Exception):
    """GitHub API hız sınırı: aday İŞARETLENMEZ, tur durur (sonra tekrar denenir)."""


def test_edilmis_mi(repo):
    """CI iş akışı var mı ve varsayılan dalın son kontrolleri başarılı mı?"""
    ad, dal = repo["full_name"], repo.get("default_branch") or "main"
    try:
        wf = gh(f"/repos/{ad}/actions/workflows?per_page=1")
        if int(wf.get("total_count", 0)) == 0:
            return False, "CI iş akışı yok"
        cr = gh(f"/repos/{ad}/commits/{urllib.parse.quote(dal)}/check-runs?per_page=30")
        kosular = cr.get("check_runs", [])
        biten = [k for k in kosular if k.get("status") == "completed"]
        if not biten:
            return False, "tamamlanmış kontrol yok"
        kotu = [k for k in biten if k.get("conclusion") not in ("success", "neutral", "skipped")]
        return (not kotu), ("CI başarılı" if not kotu else f"{len(kotu)} kontrol başarısız")
    except urllib.error.HTTPError as e:
        if e.code in (403, 429):
            raise HizSiniri(f"API {e.code}")
        return False, f"API {e.code}"


def readme_al(repo):
    ad, dal = repo["full_name"], repo.get("default_branch") or "main"
    for dosya in ("README.md", "readme.md", "README.rst", "README"):
        try:
            return istek(f"https://raw.githubusercontent.com/{ad}/{dal}/{dosya}", hdr={"User-Agent": UA["User-Agent"]}).decode("utf-8", "replace")
        except Exception:
            continue
    return ""


def sadelestir(md):
    """Rozet, HTML, resim ve uzun kod bloklarını at; düz bilgi metni kalsın."""
    md = re.sub(r"```.*?```", " ", md, flags=re.S)          # kod blokları (çalıştırılmaz, saklanmaz)
    md = re.sub(r"<[^>]+>", " ", md)                          # HTML
    md = re.sub(r"!\[[^\]]*\]\([^)]*\)", " ", md)            # resim/rozet
    md = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", md)          # link -> metin
    md = re.sub(r"[#>*_`|]+", " ", md)
    return re.sub(r"\s+", " ", md).strip()


def main():
    gorulen, sayfa = durum_yukle()
    print(f"🐙 GitHub öğrenici başladı | görülen: {len(gorulen)} | min ★{MIN_YILDIZ} | lisans: {', '.join(LISANSLAR)}", flush=True)
    i = 0
    while True:
        konu = KONULAR[i % len(KONULAR)]; lisans = LISANSLAR[(i // len(KONULAR)) % len(LISANSLAR)]; i += 1
        anahtar = f"{konu}|{lisans}"
        s_no = int(sayfa.get(anahtar, 1))
        try:
            adaylar = ara(konu, lisans, s_no)
        except Exception as e:
            print(f"[{anahtar}] arama hata: {e} — bekleniyor", flush=True); time.sleep(BEKLE); continue
        # Sayfa bittiyse (sonuç yok / 10'dan az) bu konu-lisansta başa dön; aksi halde ilerle.
        sayfa[anahtar] = 1 if len(adaylar) < 10 else s_no + 1
        eklendi = 0
        kontrol = 0
        for r in adaylar:
            if eklendi >= TUR_BASI:
                sayfa[anahtar] = s_no  # yarım kaldı: sonraki turda aynı sayfa (görülenler atlanır)
                break
            ad = r["full_name"]
            if ad in gorulen:
                continue
            if kontrol >= KONTROL_BASI:
                sayfa[anahtar] = s_no  # kontrol bütçesi bitti: kalanlar sonraki turda
                break
            kontrol += 1
            try:
                ok, neden = test_edilmis_mi(r)
            except HizSiniri as e:
                print(f"  hız sınırı ({e}) — tur durduruldu, {ad} sonra tekrar denenecek", flush=True)
                sayfa[anahtar] = s_no
                break
            if not ok:
                print(f"  atla {ad}: {neden}", flush=True); gorulen.add(ad); continue
            ham = readme_al(r)
            if ENJEKSIYON.search(ham):
                print(f"  atla {ad}: istem-enjeksiyonu kalıbı", flush=True); gorulen.add(ad); continue
            metin = sadelestir(ham)
            if len(metin) < 300:
                print(f"  atla {ad}: README çok kısa", flush=True); gorulen.add(ad); continue
            aciklama = (r.get("description") or "").strip()
            lis = ((r.get("license") or {}).get("spdx_id") or lisans).upper()
            kunye = f"Kaynak: GitHub {ad} · lisans {lis} · ★{r.get('stargazers_count', 0)} · {neden} · son güncelleme {str(r.get('pushed_at', ''))[:10]}."
            try:
                cevap = json.loads(istek(BRAIN + "/kb/ingest", timeout=120, data={
                    "baslik": f"GitHub: {ad}"[:220],
                    "metin": f"{ad}: {aciklama}. {kunye} {metin}"[:2200],
                    "url": r.get("html_url"),
                }))
                if cevap.get("ok"):
                    gorulen.add(ad); eklendi += 1
                    print(f"  + {ad} ({lis}, ★{r.get('stargazers_count', 0)})", flush=True)
                    time.sleep(0.5)
            except Exception as e:
                print(f"  ingest hata {ad}: {e}", flush=True); time.sleep(5)
        durum_kaydet(gorulen, sayfa)
        print(f"[{anahtar} s{s_no}] +{eklendi} | toplam görülen: {len(gorulen)}", flush=True)
        time.sleep(BEKLE)


if __name__ == "__main__":
    main()
