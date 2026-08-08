#!/usr/bin/env python3
"""
soulware-ingest — KUBRA otomatik BİLGİ ingesti (grounding korpusunu büyütür).

"En güçlü AI'ların kaynakları" = otoriter, AÇIK-LİSANSLI bilgi tabanları. Bu araç
Wikipedia'yı (HuggingFace wikimedia/wikipedia, CC BY-SA) çeker, her makalenin GİRİŞ
özetini bir pasaj olarak KUBRA'nın bilgi deposuna (POST /kb/ingest) ekler. KUBRA
böylece bu bilgiyi GETİRİP cevaplar (model yeniden eğitilmez — anında "bilir").

DÜRÜSTLÜK/GÜVENLİK:
 - Yalnız BEYAZ-LİSTELİ otoriter kaynak (Wikipedia). Körlemesine scraping YOK
   (zehirlenme/yanlış bilgi riski). Kaynak + url her pasajla saklanır (izlenebilir).
 - Bu, BİLGİ öğrenmedir (grounding). Muhakeme/akıl artışı DEĞİL (o büyük model+GPU ister).
 - Ölçek notu: mevcut depo bellek-içi lineer tarama; bu araç BİR PARTİ (yüzler) için
   uygundur. Milyonlar için indeks (sqlite FTS / vektör) yükseltmesi ayrı adım.

Kullanım:
  python3 soulware-ingest/ingest.py --config 20231101.tr --count 200
  python3 soulware-ingest/ingest.py --kb http://127.0.0.1:8646 --config 20231101.en --count 100 --offset 0
"""
import argparse, json, time, urllib.request, urllib.parse, urllib.error

UA = "SoulwareAI-KUBRA/0.1 (grounding ingest; CC BY-SA)"
HF = "https://datasets-server.huggingface.co/rows"

def hf_rows(config, offset, length):
    q = urllib.parse.urlencode({"dataset": "wikimedia/wikipedia", "config": config,
                                "split": "train", "offset": offset, "length": length})
    req = urllib.request.Request(f"{HF}?{q}", headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read().decode()).get("rows", [])

def ozet(text, max_len):
    """Makalenin giriş özetini al: ilk paragraf(lar), max_len karaktere kadar."""
    t = (text or "").strip()
    # İlk anlamlı bölümü al (çift satır sonuna kadar), sonra kırp.
    ilk = t.split("\n\n")[0].strip() if "\n\n" in t else t
    if len(ilk) < 80 and "\n\n" in t:  # çok kısa giriş → biraz daha ekle
        ilk = " ".join(t.split("\n\n")[:2]).strip()
    return ilk[:max_len].strip()

def kb_ingest(kb, baslik, metin, url):
    body = json.dumps({"baslik": baslik, "metin": metin, "url": url}).encode()
    req = urllib.request.Request(kb.rstrip("/") + "/kb/ingest", data=body,
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            v = json.loads(r.read().decode())
        return v.get("ok", False), v.get("belge_sayisi")
    except Exception as e:
        return False, str(e)[:80]

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--kb", default="http://127.0.0.1:8646")
    ap.add_argument("--config", default="20231101.tr", help="HF wikipedia config (dil-tarih)")
    ap.add_argument("--count", type=int, default=200)
    ap.add_argument("--offset", type=int, default=0)
    ap.add_argument("--max-len", type=int, default=1000, help="pasaj başına maks karakter")
    ap.add_argument("--min-len", type=int, default=120, help="bu kadar kısa özetleri atla")
    ap.add_argument("--sleep", type=float, default=0.2)
    args = ap.parse_args()

    print(f"═══ soulware-ingest · Wikipedia({args.config}) → KUBRA({args.kb}) · hedef {args.count} makale ═══")
    eklenen, atlanan, son_sayi, off = 0, 0, None, args.offset
    while eklenen < args.count:
        kalan = args.count - eklenen
        batch = min(100, kalan + 20)  # atlananları telafi için biraz fazla çek
        try:
            rows = hf_rows(args.config, off, batch)
        except Exception as e:
            print(f"  ⚠ HF çekilemedi (off={off}): {str(e)[:100]}"); break
        if not rows:
            print("  (kaynak bitti)"); break
        for r in rows:
            if eklenen >= args.count:
                break
            row = r.get("row", {})
            baslik = (row.get("title") or "").strip()
            metin = ozet(row.get("text", ""), args.max_len)
            url = row.get("url")
            if not baslik or len(metin) < args.min_len or "(anlam ayrımı)" in baslik:
                atlanan += 1; continue
            ok, sayi = kb_ingest(args.kb, baslik, metin, url)
            if ok:
                eklenen += 1; son_sayi = sayi
                if eklenen % 25 == 0:
                    print(f"  … {eklenen} eklendi (depo={sayi}) — son: {baslik}")
            else:
                atlanan += 1
                if atlanan % 25 == 0:
                    print(f"  ⚠ ingest hata: {sayi}")
            time.sleep(args.sleep)
        off += len(rows)
    print(f"\n  ✅ bitti: {eklenen} makale eklendi · {atlanan} atlandı · depo≈{son_sayi}")
    print(f"  → KUBRA artık bu bilgiyi grounding ile kullanır (GET /kb/stats ile gör).")

if __name__ == "__main__":
    main()
