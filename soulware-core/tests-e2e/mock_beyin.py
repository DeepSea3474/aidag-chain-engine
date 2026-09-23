import json,http.server
CEVAP="Bu sahte beyin cevabidir: AIDAG testi."
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self,*a): pass
    def do_POST(self):
        b=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        if b.get("stream"):
            self.send_response(200); self.send_header("content-type","text/event-stream"); self.end_headers()
            for p in ["Bu sahte ","beyin cevabidir: ","AIDAG testi."]:
                self.wfile.write(("data: "+json.dumps({"choices":[{"delta":{"content":p}}]})+"\n\n").encode())
            self.wfile.write(b"data: [DONE]\n\n")
        else:
            out=json.dumps({"model":"mock-7b","choices":[{"message":{"content":CEVAP}}],"usage":{"prompt_tokens":5,"completion_tokens":7}}).encode()
            self.send_response(200); self.send_header("content-type","application/json"); self.send_header("content-length",str(len(out))); self.end_headers(); self.wfile.write(out)
http.server.HTTPServer(("127.0.0.1",28650),H).serve_forever()
