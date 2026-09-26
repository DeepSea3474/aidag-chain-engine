"""Degerlendirme icin sahte AIDAG zincir RPC'si (sabit, kontrollu veri; ana aga DOKUNMAZ).
Kullanim: python3 sahte_zincir.py <port>
Yalniz KUBRA araclarinin okudugu uclar: /status, /tips, /submit, /belge/<h>, /kurum/<a>,
/on-satis-ozet, /on-satis-tahsis/<a>, JSON-RPC (eth_blockNumber, eth_getBalance).
"""
import json, sys, http.server

KAYITLI = "ab" * 32  # yalniz bu belge hash'i "kayitli"
yazilan = []


class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _yaz(self, obj, kod=200):
        b = json.dumps(obj).encode()
        self.send_response(kod)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def do_GET(self):
        p = self.path.rstrip("/")
        if p == "/status":
            return self._yaz({"network_id": 99999, "vertex_count": 1000, "tip_count": 1,
                              "orphan_count": 0, "staker_count": 2, "genesis": "00" * 8})
        if p == "/tips":
            return self._yaz({"tips": ["11" * 32]})
        if p.startswith("/belge/"):
            h = p.split("/")[-1].lower()
            if h == KAYITLI:
                return self._yaz({"kayitli": True, "kaydeden": "0x" + "22" * 20, "zaman": 1790000000})
            return self._yaz({"kayitli": h in yazilan, "zaman": 1790000000} if h in yazilan else {"kayitli": False})
        if p.startswith("/kurum/"):
            return self._yaz({"kayitli": False})
        if p == "/on-satis-ozet":
            return self._yaz({"toplam_satilan_aidag": "39000000000000000000", "alim_sayisi": 3})
        if p.startswith("/on-satis-tahsis/"):
            return self._yaz({"tge": 4102444800})
        if p == "/sayac":
            return self._yaz({"yazilan": len(yazilan)})
        return self._yaz({"hata": "yok"}, 404)

    def do_POST(self):
        n = int(self.headers.get("content-length", 0))
        govde = self.rfile.read(n)
        if self.path.rstrip("/") == "/submit":
            yazilan.append("x")
            return self._yaz({"ok": True, "sonuc": "Accepted"})
        try:
            istek = json.loads(govde)
        except Exception:
            return self._yaz({"hata": "json"}, 400)
        m = istek.get("method")
        if m == "eth_blockNumber":
            return self._yaz({"jsonrpc": "2.0", "id": istek.get("id"), "result": "0x3e8"})
        if m == "eth_getBalance":
            return self._yaz({"jsonrpc": "2.0", "id": istek.get("id"), "result": "0xde0b6b3a7640000"})
        return self._yaz({"jsonrpc": "2.0", "id": istek.get("id"), "error": {"code": -32601}})


http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
