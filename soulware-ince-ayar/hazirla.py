#!/usr/bin/env python3
"""KUBRA ince ayar veri seti hazirlama — YALNIZ lisansi temiz kaynaklar.

Kaynaklar (lisans kaynagindan dogrulandi, bkz. LISANS_RAPORU.md):
  1. kubra_kisilik.jsonl         — el yazimi, proje mali (KUBRA'nin sesi; 5x agirlik)
  2. Aya (CohereLabs/aya_dataset) — Apache-2.0, insan yazimi, yalniz Turkce satirlar
  3. oasst2 (OpenAssistant)       — Apache-2.0, yalniz Turkce agaclar, en iyi siralanan yol

Ek temizlik (Apache lisansli sette bile basimiza is acmasin):
  - Telif riski: sarki sozu / siir / "tum haklari saklidir" / (c) isaretli, cok sayida kisa
    satirdan olusan (siir-sarki bicimi) metinler ATILIR.
  - Kisisel veri: e-posta, telefon, TC kimlik benzeri 11 hane ATILIR.
  - Alinti metin: uzun bir metin parcasi yapistirip soru soran ornekler ATILIR (parca
    cogunlukla Wikipedia/haber kaynakli olabilir -> CC-BY-SA/telif belirsizligi).
  - Link agirlikli, cok kisa/uzun, tekrar eden ornekler ATILIR.
Cikti: veri/egitim.jsonl, veri/dogrulama.jsonl ({"messages":[...]}) + ozet (stdout).
"""
import gzip, hashlib, json, os, random, re

KOK = os.path.dirname(os.path.abspath(__file__))
HAM = os.path.join(KOK, "ham")
VERI = os.path.join(KOK, "veri")
SISTEM = ("Sen KUBRA'sın: SoulwareAI'ın yapay zekası, AIDAG-Chain üzerinde çalışırsın. "
          "Sıcak, samimi, dürüst ve yardımseversin; kullanıcıya 'sen' diye hitap edersin, "
          "düzgün Türkçe konuşursun ve asla uydurmazsın.")

TELIF = re.compile(r"şarkı söz|sarki soz|şiir|siir|nakarat|tüm hakları saklı|tum haklari sakli|©|\(c\)|"
                   r"copyright|all rights reserved|lyrics", re.I)
KISISEL = re.compile(r"[\w.+-]+@[\w-]+\.[\w.]+|(?:\+90|0)\s*\d{3}\s*\d{3}\s*\d{2}\s*\d{2}|\b\d{11}\b")
LINK = re.compile(r"https?://")
ALINTI = re.compile(r"(bu |aşağıdaki |asagidaki )?(yazı|yazi|metin|paragraf|makale|pasaj|haber)[^\n]{0,60}[:?]", re.I)

def temiz_mi(u, a):
    for t in (u, a):
        if TELIF.search(t) or KISISEL.search(t):
            return False, "telif/kisisel"
        if len(LINK.findall(t)) > 1:
            return False, "link"
    if not (4 <= len(u) <= 600 and 20 <= len(a) <= 2500):
        return False, "uzunluk"
    if len(u) > 250 and ALINTI.search(u):
        return False, "alinti metin"
    satirlar = [s for s in a.splitlines() if s.strip()]
    if len(satirlar) >= 6 and sum(len(s) for s in satirlar) / len(satirlar) < 45:
        return False, "siir-sarki bicimi"
    return True, ""

def ornek(u, a, kaynak):
    return {"messages": [{"role": "system", "content": SISTEM},
                         {"role": "user", "content": u.strip()},
                         {"role": "assistant", "content": a.strip()}], "kaynak": kaynak}

def aya():
    yol = os.path.join(HAM, "aya_tr.jsonl")
    if not os.path.exists(yol):
        return []
    return [(json.loads(l)["inputs"], json.loads(l)["targets"]) for l in open(yol)]

def oasst2_tr():
    """Turkce agaclarda her istem icin en iyi siralanan (rank 0) asistan yolu."""
    ciftler = []
    for line in gzip.open(os.path.join(HAM, "oasst2_ready.trees.jsonl.gz"), "rt"):
        t = json.loads(line)
        p = t["prompt"]
        if p.get("lang") != "tr":
            continue
        def yuru(dugum):
            cocuk = [c for c in dugum.get("replies", []) if not c.get("deleted")]
            if not cocuk:
                return
            en_iyi = min(cocuk, key=lambda c: (c.get("rank") if c.get("rank") is not None else 99))
            if dugum["role"] == "prompter" and en_iyi["role"] == "assistant":
                ciftler.append((dugum["text"], en_iyi["text"]))
            yuru(en_iyi)
        yuru(p)
    return ciftler

def main():
    os.makedirs(VERI, exist_ok=True)
    random.seed(3474)
    kisilik = [json.loads(l) for l in open(os.path.join(KOK, "kubra_kisilik.jsonl"))]
    atilan, tum, gorulen = {}, [], set()
    for kaynak, ciftler in (("aya-apache2", aya()), ("oasst2-apache2", oasst2_tr())):
        for u, a in ciftler:
            ok, neden = temiz_mi(u, a)
            h = hashlib.sha1((u.strip().lower() + "\x1e" + a.strip().lower()).encode()).hexdigest()
            if not ok or h in gorulen:
                atilan[neden or "tekrar"] = atilan.get(neden or "tekrar", 0) + 1
                continue
            gorulen.add(h); tum.append(ornek(u, a, kaynak))
    random.shuffle(tum)
    dogrulama = tum[:150]
    egitim = tum[150:] + kisilik * 5   # KUBRA sesi 5x agirlik ('sen' hitabi baskin olsun)
    random.shuffle(egitim)
    for ad, veri in (("egitim.jsonl", egitim), ("dogrulama.jsonl", dogrulama)):
        with open(os.path.join(VERI, ad), "w") as f:
            for o in veri:
                f.write(json.dumps(o, ensure_ascii=False) + "\n")
    say = {}
    for o in egitim:
        say[o.get("kaynak", "kubra-el-yazimi")] = say.get(o.get("kaynak", "kubra-el-yazimi"), 0) + 1
    print(json.dumps({"egitim": len(egitim), "dogrulama": len(dogrulama), "kaynaklara_gore": say,
                      "atilan": atilan}, ensure_ascii=False, indent=1))

if __name__ == "__main__":
    main()
