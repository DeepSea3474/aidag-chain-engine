#!/usr/bin/env python3
"""
soulware-bench — KUBRA yetenek olcum harness'i (DURUST, tekrarlanabilir).

KUBRA'yi sabit bir soru setiyle calistirir, dogruluk + gecikme + abstention olcer,
sonucu etiketli kaydeder. Amac: KUBRA'nin BUGUNKU seviyesini olcup her iyilestirmede
(GPU, buyuk model, genis korpus) tirmanisi izlemek. Uydurma frontier skoru YOK —
'frontier' bayragi yalniz "guclu modeller bunu bilir mi" referansidir (temel set).

Kullanim:
  python3 bench/bench.py --label "1.5b-cpu-grounding-v1"
  python3 bench/bench.py --brain http://127.0.0.1:8646 --questions bench/questions.json --label "..."
"""
import argparse, json, time, urllib.request, urllib.error, os, datetime

def normalize(s: str) -> str:
    tr = {"ç": "c", "ğ": "g", "ı": "i", "ş": "s", "ö": "o", "ü": "u", "â": "a", "î": "i", "û": "u"}
    s = "".join(tr.get(ch, ch) for ch in (s or "").lower())
    s = "".join(ch if ch.isalnum() else " " for ch in s)
    return " ".join(s.split())

def ask(brain: str, soru: str, timeout: int):
    body = json.dumps({"prompt": soru, "deterministic": True, "brain": "local"}).encode()
    req = urllib.request.Request(brain.rstrip("/") + "/v1/ask", data=body,
                                 headers={"Content-Type": "application/json"})
    t0 = time.time()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            v = json.loads(r.read().decode())
        ms = int((time.time() - t0) * 1000)
        return v, ms, None
    except Exception as e:
        return None, int((time.time() - t0) * 1000), str(e)

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--brain", default="http://127.0.0.1:8646")
    ap.add_argument("--questions", default=os.path.join(os.path.dirname(__file__), "questions.json"))
    ap.add_argument("--label", required=True, help="bu calismanin etiketi (or. '1.5b-cpu-grounding-v1')")
    ap.add_argument("--timeout", type=int, default=300)
    ap.add_argument("--outdir", default=os.path.join(os.path.dirname(__file__), "results"))
    args = ap.parse_args()

    qs = json.load(open(args.questions))["sorular"]
    print(f"═══ soulware-bench · KUBRA · etiket='{args.label}' · {len(qs)} soru ═══")
    print(f"    beyin={args.brain}\n")

    sonuclar, dogru, frontier_dogru, frontier_top = [], 0, 0, 0
    kat = {}
    for q in qs:
        v, ms, err = ask(args.brain, q["soru"], args.timeout)
        answer = (v or {}).get("answer", "")
        grounded = bool((v or {}).get("grounded"))
        abst = bool((v or {}).get("abstained"))
        beklenen = q.get("cevap", "")
        ok = bool(beklenen) and (normalize(beklenen) in normalize(answer))
        if ok: dogru += 1
        k = q["kategori"]; kat.setdefault(k, [0, 0]); kat[k][1] += 1; kat[k][0] += 1 if ok else 0
        if q.get("frontier"):
            frontier_top += 1; frontier_dogru += 1 if ok else 0
        isaret = "✅" if ok else ("🤔" if abst else "❌")
        print(f"  {isaret} [{k:16}] {q['soru'][:48]:48} → {answer[:40]!r}"
              f"  (grounded={grounded} {ms}ms){'' if not err else ' HATA:'+err}")
        sonuclar.append({"id": q["id"], "kategori": k, "soru": q["soru"], "beklenen": beklenen,
                         "cevap": answer, "dogru": ok, "grounded": grounded, "abstained": abst,
                         "ms": ms, "hata": err})

    n = len(qs)
    ort_ms = round(sum(s["ms"] for s in sonuclar) / max(n, 1))
    abst_say = sum(1 for s in sonuclar if s["abstained"])
    print(f"\n─── SONUC ───")
    print(f"  Genel dogruluk : {dogru}/{n}  (%{round(100*dogru/max(n,1))})")
    for k, (d, t) in sorted(kat.items()):
        print(f"    {k:18}: {d}/{t}")
    print(f"  Abstention     : {abst_say}/{n} ('Bilmiyorum' — durust bosluk)")
    print(f"  Ort. gecikme   : {ort_ms} ms/soru  (CPU; GPU'da dusecek)")
    print(f"  Frontier ref.  : bu setteki 'kolay' sorularin {frontier_top} tanesini guclu modeller ~tam yapar; "
          f"KUBRA {frontier_dogru}/{frontier_top}. (Fark ileri sorularda acilir.)")

    os.makedirs(args.outdir, exist_ok=True)
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    rapor = {"etiket": args.label, "zaman": stamp, "beyin": args.brain,
             "genel_dogru": dogru, "toplam": n, "ort_ms": ort_ms, "abstention": abst_say,
             "kategori": {k: {"dogru": d, "toplam": t} for k, (d, t) in kat.items()},
             "sonuclar": sonuclar}
    yol = os.path.join(args.outdir, f"{stamp}_{args.label}.json")
    json.dump(rapor, open(yol, "w"), ensure_ascii=False, indent=2)
    print(f"\n  💾 kaydedildi: {yol}")

if __name__ == "__main__":
    main()
