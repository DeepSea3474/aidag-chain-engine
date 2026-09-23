import json,http.server,sys,time
# Sahte uzak beyin (OpenAI-uyumlu) + gorsel servisi. Port: argv[1] (varsayilan 28650).
# - "YAVAS" iceren istem: 3 sn bekler (eszamanlilik testi).
# - Icerik denetleyicisi (system'de "denetleyicisi"): "ENGELLE" -> ENGEL, "HATALI" -> HTTP 500,
#   "BOZUK" -> JSON olmayan govde; aksi halde IZIN.
# - POST /img: sabit bayt (gorsel servisi).
CEVAP="Bu sahte beyin cevabidir: AIDAG testi."
PORT=int(sys.argv[1]) if len(sys.argv)>1 else 28650
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self,*a): pass
    def yaz(self,kod,govde,tip="application/json"):
        self.send_response(kod); self.send_header("content-type",tip); self.send_header("content-length",str(len(govde))); self.end_headers(); self.wfile.write(govde)
    def do_POST(self):
        b=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        if self.path=="/img": return self.yaz(200,b"PNGDATA\x00\x1e\xff","image/png")
        msgs=b.get("messages",[])
        tum=json.dumps(msgs,ensure_ascii=False)
        if "YAVAS" in tum: time.sleep(3)
        if msgs and "denetleyicisi" in msgs[0].get("content",""):
            u=msgs[-1].get("content","")
            if "HATALI" in u: return self.yaz(500,b'{"error":"x"}')
            if "BOZUK" in u: return self.yaz(200,b"<html>")
            t="ENGEL" if "ENGELLE" in u else "IZIN"
            return self.yaz(200,json.dumps({"choices":[{"message":{"content":t}}]}).encode())
        if b.get("stream"):
            self.send_response(200); self.send_header("content-type","text/event-stream"); self.end_headers()
            for p in ["Bu sahte"," beyin\ncevabidir:\n\n"," AIDAG testi."]:
                self.wfile.write(("data: "+json.dumps({"choices":[{"delta":{"content":p}}]})+"\n\n").encode())
            self.wfile.write(b"data: [DONE]\n\n")
        else:
            self.yaz(200,json.dumps({"model":"mock-7b","choices":[{"message":{"content":CEVAP}}],"usage":{"prompt_tokens":5,"completion_tokens":7}}).encode())
http.server.ThreadingHTTPServer(("127.0.0.1",PORT),H).serve_forever()
