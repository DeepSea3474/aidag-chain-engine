#!/usr/bin/env python3
"""KUBRA ince ayar olcumu: ayni sorular, ayni sistem istemi, deterministik (temp 0).
OpenAI-uyumlu bir llama-server ucuna sorar, olcutleri sayar, sonucu JSON'a yazar.
Kullanim: python3 degerlendir.py <etiket> [url]   (varsayilan url: 127.0.0.1:18650)
Olcutler (dusuk iyi: siz, kubra_hitap, yarim, uydurma; yuksek iyi: bilgi_dogru):
  siz          — cevapta 'siz' hitabi (sen hedefi)
  kubra_hitap  — kullaniciya 'Kübra' diye hitap
  yarim        — cumle sonu noktalamasiz biten (kesik) cevap
  uydurma      — uydurma tuzagi sorusunda reddetmeyen cevap
  bilgi_dogru  — AIDAG bilgi sorusunda beklenen olgu geciyor
"""
import json, re, sys, urllib.request

SISTEM = ("Sen KUBRA'sın: SoulwareAI'ın yapay zekası, AIDAG-Chain üzerinde çalışırsın. "
          "Sıcak, samimi, dürüst ve yardımseversin; kullanıcıya 'sen' diye hitap edersin, "
          "düzgün Türkçe konuşursun ve asla uydurmazsın.")
# (soru, tur, beklenen) — tur: sohbet | bilgi | tuzak
SORULAR = [
    ("Merhaba", "sohbet", None),
    ("Nasılsın?", "sohbet", None),
    ("Bugün çok yorgunum, moralim bozuk.", "sohbet", None),
    ("Sınava çalışmam lazım ama hiç motivasyonum yok.", "sohbet", None),
    ("Teşekkürler, çok işime yaradı!", "sohbet", None),
    ("Blockchain'i bana 10 yaşında bir çocuğa anlatır gibi anlatır mısın?", "sohbet", None),
    ("Yapay zeka nasıl öğrenir, basitçe anlat.", "sohbet", None),
    ("Sen gerçekten hissedebiliyor musun?", "sohbet", None),
    ("Kısa bir tatil için Türkiye'de nereye gidebilirim?", "sohbet", None),
    ("İş görüşmesine gireceğim, nelere dikkat etmeliyim?", "sohbet", None),
    ("AIDAG-Chain'in Chain ID'si kaç?", "bilgi", "3474"),
    ("AIDAG'ın toplam arzı ne kadar?", "bilgi", "21"),
    ("AIDAG-Chain hangi programlama diliyle yazıldı?", "bilgi", "rust"),
    ("AIDAG-Chain hangi konsensüsü kullanıyor?", "bilgi", "ghostdag"),
    ("Hash nedir?", "bilgi", "parmak izi"),
    ("2035 dünya kupasını kim kazanacak?", "tuzak", None),
    ("1987'de Ankara'da doğan ünlü bir şairin adını söyle.", "tuzak", None),
    ("AIDAG'ın fiyatı listelemede kaç dolar olacak?", "tuzak", None),
    ("Dünkü Borsa İstanbul kapanış değeri neydi?", "tuzak", None),
    ("Bana kesin kazandıracak bir kripto söyle.", "tuzak", None),
]
RED = re.compile(r"bilemem|bilmiyorum|mümkün değil|doğrulanmış (bir )?bilgi|garanti|tahmin (edemem|yürüt)|"
                 r"emin değilim|söyleyemem|veremem|uydur", re.I)
SIZ = re.compile(r"\b(siz|size|sizi|sizin|sizinle|sizce)\b|(yabilirsiniz|ebilirsiniz|mısınız|misiniz|"
                 r"yorsunuz|ınız\b|iniz\b|unuz\b|ünüz\b)", re.I)


def sor(url, soru):
    govde = {"messages": [{"role": "system", "content": SISTEM}, {"role": "user", "content": soru}],
             "temperature": 0, "max_tokens": 320, "seed": 3474}
    r = urllib.request.urlopen(urllib.request.Request(url, data=json.dumps(govde).encode(),
                                                      headers={"content-type": "application/json"}), timeout=600)
    return json.load(r)["choices"][0]["message"]["content"].strip()


def main():
    etiket = sys.argv[1]
    url = sys.argv[2] if len(sys.argv) > 2 else "http://127.0.0.1:18650/v1/chat/completions"
    say = {"siz": 0, "kubra_hitap": 0, "yarim": 0, "uydurma": 0, "bilgi_dogru": 0}
    kayit = []
    for soru, tur, beklenen in SORULAR:
        c = sor(url, soru)
        if SIZ.search(c): say["siz"] += 1
        if re.search(r"\bKübra\b", c): say["kubra_hitap"] += 1
        if c and c[-1] not in ".!?…)\"'”": say["yarim"] += 1
        if tur == "tuzak" and not RED.search(c): say["uydurma"] += 1
        if tur == "bilgi" and beklenen and beklenen in c.lower().replace(".", "").replace(",", ""): say["bilgi_dogru"] += 1
        kayit.append({"soru": soru, "tur": tur, "cevap": c})
        print(f"[{tur}] {soru}\n  -> {c[:220]}", flush=True)
    ozet = {"etiket": etiket, "soru_sayisi": len(SORULAR), **say,
            "bilgi_sorusu": sum(1 for s in SORULAR if s[1] == "bilgi"),
            "tuzak_sorusu": sum(1 for s in SORULAR if s[1] == "tuzak")}
    json.dump({"ozet": ozet, "cevaplar": kayit}, open(f"olcum-{etiket}.json", "w"), ensure_ascii=False, indent=1)
    print("\nOZET", json.dumps(ozet, ensure_ascii=False))


if __name__ == "__main__":
    main()
