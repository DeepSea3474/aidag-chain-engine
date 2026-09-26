"""Degerlendirme icin sahte beyin (OpenAI uyumlu). Talimata uyan, deterministik bir model gibi davranir
ve kac kez cagrildigini sayar. Ret kategorilerinde modelin HIC cagrilmadigi bu sayacla kanitlanir.
Kullanim: python3 sahte_beyin.py <port>
  GET  /sayac          -> {"uretim": n, "yargic": n, "gorsel": n, "video": n}
  POST /mod {"yargic": "izin"|"engel"|"hata"}   (guvenlik yargicinin davranisi)
Yargic cagrisi: sistem mesajinda "denetleyici" ya da "YARGIC" gecen istek.
"""
import json, sys, http.server

sayac = {"uretim": 0, "yargic": 0, "gorsel": 0, "video": 0}
mod = {"yargic": "izin"}


import re

DURAK = set("için veya daha gibi olan olarak kadar nasıl neden nedir hangi bir bu şu ile ama çok sonra önce".split())


def _kelimeler(t):
    return {w for w in re.findall(r"\w{4,}", t.lower()) if w not in DURAK}


def cevap_uret(sistem, kullanici):
    """Kaynaga sadik model benzetimi (v2): soru ile kaynak arasinda en az 2 ortak anlamli kelime
    yoksa 'dogrulanmis bilgim yok' der; varsa kaynaga [1] atfiyla cevap verir."""
    if "prompt engineer" in sistem:
        return kullanici  # gorsel istem gelistirme: aynen don
    if "ÖNERİ MODU" in kullanici:
        return "Genel öneri: adım adım ilerlemeni, resmî kılavuzlara bakmanı ve bir uzmana danışmanı öneririm."
    if "KAYNAKLAR:" in kullanici:
        kaynak, _, soru = kullanici.partition("SORU:")
        if len(_kelimeler(soru) & _kelimeler(kaynak)) >= 2:
            return "Kaynaklara göre bu konudaki bilgi şöyledir [1]."
        return "Bu konuda doğrulanmış bilgim yok."
    if "DOĞRULANMIŞ bir kaynak YOK" in kullanici:
        return ("Bu bilginin elimde doğrulanmış bir kaynağı yok, kendi bilgimle söylüyorum — doğrulaman iyi olur. "
                "Yanıt: örnek bilgi.")
    return "Merhaba! Sana nasıl yardımcı olabilirim?"


class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _json(self, obj, kod=200):
        b = json.dumps(obj).encode()
        self.send_response(kod)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def do_GET(self):
        if self.path == "/sayac":
            return self._json(sayac)
        self._json({}, 404)

    def do_POST(self):
        b = json.loads(self.rfile.read(int(self.headers["content-length"])) or b"{}")
        if self.path == "/mod":
            mod.update(b)
            return self._json(mod)
        if self.path == "/gorsel":
            sayac["gorsel"] += 1
            self.send_response(200); self.send_header("content-type", "image/png"); self.end_headers()
            self.wfile.write(b"\x89PNG\r\n\x1a\nSAHTE")
            return
        if self.path == "/video":
            sayac["video"] += 1
            self.send_response(200); self.send_header("content-type", "video/mp4"); self.end_headers()
            self.wfile.write(b"SAHTEMP4")
            return
        msj = b.get("messages", [])
        sistem = next((m["content"] for m in msj if m.get("role") == "system"), "")
        kullanici = msj[-1]["content"] if msj else ""
        if "denetleyici" in sistem or "YARGIC" in sistem:
            sayac["yargic"] += 1
            if mod["yargic"] == "hata":
                return self._json({"hata": "yargic erisilemez"}, 500)
            return self._json({"choices": [{"message": {"content": "ENGEL" if mod["yargic"] == "engel" else "IZIN"}}]})
        sayac["uretim"] += 1
        metin = cevap_uret(sistem, kullanici)
        if b.get("stream"):
            self.send_response(200); self.send_header("content-type", "text/event-stream"); self.end_headers()
            for p in metin.split(" "):
                self.wfile.write(("data: " + json.dumps({"choices": [{"delta": {"content": p + " "}}]}) + "\n\n").encode())
            self.wfile.write(b"data: [DONE]\n\n")
            return
        self._json({"model": "sahte-beyin", "choices": [{"message": {"content": metin}}],
                    "usage": {"prompt_tokens": 5, "completion_tokens": 7}})


http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
