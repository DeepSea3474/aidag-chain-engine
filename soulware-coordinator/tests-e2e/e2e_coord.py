"""soulware-coordinator + soulware-worker denetim duzeltmeleri uctan uca testi
(izole devnet; mainnet'e ve canli servislere DOKUNMAZ).

Kapsam (bulgu 3 + 8): fail-closed havuz/worker anahtari, cuzdan imzasi (401),
ayni IP sybil'in yedeklilige sayilmamasi, istemci basina gunluk ucretsiz kota,
ucretli is odemesinin belirli tip=7 transferine baglanmasi (cifte sayim yok),
oto-settlement varsayilan KAPALI, gercek soulware-worker ikilisinin imzali akisi.

Kullanim (root; ayri ag ad alani):
  D=$(mktemp -d); cp e2e_coord.py $D/
  cp <lsc-node> <soulware-pay> <soulware-coordinator> <soulware-worker> $D/
  cd $D && env -u ANTHROPIC_API_KEY unshare -n bash -c 'ip link set lo up && python3 e2e_coord.py'
Gerekli: python3 blake3, nacl (PyNaCl).
"""
import json,subprocess,time,urllib.request,urllib.error,urllib.parse,os,sys,threading,http.server
import blake3, nacl.signing
S=os.path.dirname(os.path.abspath(__file__)); os.chdir(S)
NET=99999; RPC="http://127.0.0.1:38745"; CO="http://127.0.0.1:38747"; BEYIN_PORT=38748
def istek(u,b=None,h=None,yontem=None):
    hh={"content-type":"application/json"}; hh.update(h or {})
    r=urllib.request.Request(u,None if b is None else json.dumps(b).encode(),hh,method=yontem)
    try: return json.loads(urllib.request.urlopen(r,timeout=60).read())
    except urllib.error.HTTPError as e:
        d=e.read()
        try: return {"_http":e.code, **json.loads(d)}
        except Exception: return {"_http":e.code,"_govde":d.decode(errors="replace")}
def bekle(u):
    for _ in range(240):
        try: return istek(u)
        except Exception: time.sleep(0.5)
    raise SystemExit("baslamadi: "+u)
sonuc=[]
def kontrol(ad,ok,ayrinti=""):
    sonuc.append((ad,bool(ok))); print(("GECTI " if ok else "KALDI ")+ad+("" if ok else f"  <- {ayrinti}"))

class Cuzdan:
    def __init__(s,seed):
        s.sk=nacl.signing.SigningKey(bytes([seed])*32); s.pk=bytes(s.sk.verify_key)
        s.w="0x"+blake3.blake3(s.pk).digest()[:20].hex()
    def imza(s,nonce,ts=None):
        ts=int(time.time()) if ts is None else ts
        m=f"AIDAG-WORKER|{s.w}|{nonce}|{ts}".encode()
        return {"ts":ts,"imza":s.sk.sign(m).signature.hex(),"pubkey":s.pk.hex()}
def cevap_ozeti(a): return blake3.blake3(a.strip().encode()).hexdigest()
def ip(x): return {"X-Real-IP":x}
def poll(c,ipx=None,imzali=True):
    q=("?"+urllib.parse.urlencode(c.imza("poll"))) if imzali else ""
    return istek(CO+f"/worker/poll/{c.w}{q}",None,ip(ipx) if ipx else None)
def submit(c,job,cevap,ipx=None,imza_cevap=None):
    return istek(CO+"/worker/submit",{"wallet":c.w,"job_id":job,"answer":cevap,**c.imza(f"is:{job}:{cevap_ozeti(imza_cevap or cevap)}")},ip(ipx) if ipx else None)

# Sahte KUBRA beyni (worker ikilisi icin): her soruya "4"
class Beyin(http.server.BaseHTTPRequestHandler):
    def log_message(s,*a): pass
    def do_POST(s):
        s.rfile.read(int(s.headers['Content-Length'])); o=json.dumps({"ok":True,"answer":"4"}).encode()
        s.send_response(200); s.send_header("content-type","application/json"); s.send_header("content-length",str(len(o))); s.end_headers(); s.wfile.write(o)
threading.Thread(target=http.server.ThreadingHTTPServer(("127.0.0.1",BEYIN_PORT),Beyin).serve_forever,daemon=True).start()

env=dict(os.environ, LSC_NETWORK_ID=str(NET), LSC_RPC_ADDR="127.0.0.1:38745", RUST_LOG="warn")
for k in ("LSC_MAINNET","LSC_PRODUCTION"): env.pop(k,None)
node=subprocess.Popen(["./lsc-node","/ip4/127.0.0.1/tcp/49201","e2e-data.log"],env=env,stdout=open("node.out","w"),stderr=subprocess.STDOUT)
bekle(RPC+"/status")
cenv={k:v for k,v in os.environ.items() if not k.startswith("SOULWARE_") and k!="ANTHROPIC_API_KEY"}
cenv.update(SOULWARE_COORD_KEY=S+"/havuz.key", SOULWARE_COORD_LISTEN="127.0.0.1:38747", SOULWARE_CHAIN_RPC=RPC,
    SOULWARE_NET_ID=str(NET), SOULWARE_COORD_DATA=S+"/coord.json", SOULWARE_FREE_DAILY_PER_CLIENT="2",
    SOULWARE_REDUNDANCY="2", SOULWARE_MAX_ASSIGN="3", SOULWARE_REWARD_LSC="1")
procs=[node]
try:
    # ── Bulgu 3: fail-closed havuz anahtari ──
    r=subprocess.run(["./soulware-coordinator"],env=cenv,capture_output=True,text=True,timeout=60)
    kontrol("koordinator: anahtar YOK -> baslamaz (exit 2), dosya olusmaz", r.returncode==2 and not os.path.exists(S+"/havuz.key"), r.stderr[-200:])
    open(S+"/bozuk.key","wb").write(b"\x02"*33)
    r=subprocess.run(["./soulware-coordinator"],env=dict(cenv,SOULWARE_COORD_KEY=S+"/bozuk.key"),capture_output=True,text=True,timeout=60)
    kontrol("koordinator: bozuk anahtar -> baslamaz, dosyaya DOKUNMAZ", r.returncode==2 and open(S+"/bozuk.key","rb").read()==b"\x02"*33)
    r=subprocess.run(["./soulware-coordinator","--yeni-anahtar-uret"],env=cenv,capture_output=True,text=True,timeout=60)
    kontrol("koordinator --yeni-anahtar-uret: 0600, 33 bayt", r.returncode==0 and os.stat(S+"/havuz.key").st_mode&0o777==0o600 and os.path.getsize(S+"/havuz.key")==33)
    r=subprocess.run(["./soulware-coordinator","--yeni-anahtar-uret"],env=cenv,capture_output=True,text=True,timeout=60)
    kontrol("koordinator --yeni-anahtar-uret tekrar: REDDEDER (O_EXCL)", r.returncode==2)
    wenv=dict(cenv, SOULWARE_COORD_URL=CO, SOULWARE_BRAIN_URL=f"http://127.0.0.1:{BEYIN_PORT}", SOULWARE_CONSENT="yes", SOULWARE_POLL_SEC="1")
    r=subprocess.run(["./soulware-worker"],env=dict(wenv,SOULWARE_WORKER_KEY=S+"/w-yok.key"),capture_output=True,text=True,timeout=60)
    kontrol("worker: anahtar YOK -> baslamaz (exit 2), dosya olusmaz", r.returncode==2 and not os.path.exists(S+"/w-yok.key"), r.stderr[-200:])

    co=subprocess.Popen(["./soulware-coordinator"],env=cenv,stdout=open("coord.out","w"),stderr=subprocess.STDOUT); procs.append(co)
    bekle(CO+"/health")
    havuz=istek(CO+"/status")["koordinator"]
    kontrol("oto-settlement varsayilan KAPALI", istek(CO+"/settlement/status")["auto"] is False)

    # ── Bulgu 8a: imza ──
    w1,w2,w3=Cuzdan(1),Cuzdan(2),Cuzdan(3)
    r=istek(CO+"/worker/register",{"wallet":w1.w})
    kontrol("register imzasiz -> 401", r.get("_http")==401, r)
    r=istek(CO+"/worker/register",{"wallet":w2.w,**w1.imza("kayit")})
    kontrol("register baskasinin cuzdani adina (w1 anahtariyla w2) -> 401", r.get("_http")==401, r)
    r=istek(CO+"/worker/register",{"wallet":w1.w,**w1.imza("kayit",int(time.time())-3600)})
    kontrol("register eski ts (replay penceresi disi) -> 401", r.get("_http")==401, r)
    r=istek(CO+"/worker/register",{"wallet":w1.w,**w1.imza("poll")})
    kontrol("register baska amacli imza (poll) -> 401", r.get("_http")==401, r)
    for c in (w1,w2,w3):
        r=istek(CO+"/worker/register",{"wallet":c.w,**c.imza("kayit")})
    kontrol("register gecerli imza -> ok", r.get("ok"), r)
    r=istek(CO+"/worker/benchmark",{"wallet":w1.w,"cevaplar":[{"id":1,"cevap":"4","ms":5}],**w2.imza("benchmark:00")})
    kontrol("benchmark baskasi adina -> 401", r.get("_http")==401, r)

    # ── Bulgu 8b: ayni IP sybil yedeklilige sayilmaz ──
    j=istek(CO+"/job/create",{"prompt":"2+2?"},ip("192.0.2.50"))["job_id"]
    r=poll(w1,imzali=False)
    kontrol("poll imzasiz -> 401", r.get("_http")==401, r)
    a=poll(w1,"10.0.0.1"); b=poll(w2,"10.0.0.1"); c=poll(w3,"10.0.0.2")
    kontrol("ayni IP'den ikinci cuzdana ayni is VERILMEZ", a.get("job_id")==j and b.get("none") and c.get("job_id")==j, (a,b,c))
    r=submit(w3,j,"5","10.0.0.2",imza_cevap="4")
    kontrol("submit: imza baska cevap icin -> 401 (cevap degistirilemez)", r.get("_http")==401, r)
    r1=submit(w1,j,"4","10.0.0.1"); r3=submit(w3,j,"4","10.0.0.2")
    kontrol("farkli IP iki cuzdan ayni cevap -> verified, 2 odul", r3.get("durum")=="verified" and r3.get("kazananlar")==2, (r1,r3))
    # sybil denemesi: w1 + w2 ayni IP; w2'ye is atanmadigindan submit edemez
    j2=istek(CO+"/job/create",{"prompt":"3+3?"},ip("192.0.2.51"))["job_id"]
    a=poll(w1,"10.0.0.1"); b=poll(w2,"10.0.0.1")
    r=submit(w2,j2,"6","10.0.0.1")
    kontrol("sybil: ayni IP'deki ikinci cuzdan isi alamaz / gonderemez (403)", a.get("job_id")==j2 and b.get("none") and r.get("_http")==403, (a,b,r))
    submit(w1,j2,"6","10.0.0.1")
    kontrol("sybil: tek IP ile is dogrulanamaz (pending)", istek(CO+f"/job/{j2}")["job"]["status"]=="pending")

    # ── Bulgu 8d: ucretli is odemesi belirli tip=7 transferine bagli ──
    payer_key=S+"/payer.key"; open(payer_key,"wb").write(b"\x01"+bytes([9])*32)
    payer=Cuzdan(9).w
    r=istek(CO+"/job/create",{"prompt":"ucretli","fee_lsc":2})
    kontrol("ucretli is payer'siz -> red", r.get("ok") is False, r)
    jp=istek(CO+"/job/create",{"prompt":"ucretli 1","fee_lsc":2,"payer":payer})["job_id"]
    jp2=istek(CO+"/job/create",{"prompt":"ucretli 2","fee_lsc":2,"payer":payer})["job_id"]
    r=istek(CO+"/job/confirm",{"job_id":jp})
    kontrol("confirm odeme_hex'siz -> 400 (havuz bakiyesine bakip paid YAPMAZ)", r.get("_http")==400, r)
    istek(RPC+"/lsc_test_bakiye",{"adres":payer[2:],"miktar":str(10*10**18)})
    for _ in range(40):
        if int(istek(RPC+f"/lsc-bakiye/{payer[2:]}").get("lsc_bakiye","0"))>0: break
        time.sleep(0.5)
    def ode(alici,lsc,nonce):
        tips=",".join(istek(RPC+"/tips").get("tips",[])) or "-"
        return subprocess.run(["./soulware-pay",payer_key,str(NET),alici,str(lsc),str(nonce),str(int(time.time())),tips],capture_output=True,text=True,check=True).stdout.strip()
    n0=istek(RPC+f"/nonce/{payer[2:]}")["nonce"]
    r=istek(CO+"/job/confirm",{"job_id":jp,"odeme_hex":ode("00"*20,2,n0)})
    kontrol("confirm: alicisi havuz olmayan transfer -> 400", r.get("_http")==400 and "havuz" in r.get("hata",""), r)
    r=istek(CO+"/job/confirm",{"job_id":jp,"odeme_hex":ode(havuz,1,n0)})
    kontrol("confirm: eksik miktar -> 400", r.get("_http")==400, r)
    odeme=ode(havuz,2,n0)
    r=istek(CO+"/job/confirm",{"job_id":jp,"odeme_hex":odeme})
    kontrol("confirm: gecerli tip=7 odeme -> paid, vertex id'ye bagli", r.get("paid") is True and len(r.get("odeme_vertex",""))==64, r)
    r=istek(CO+"/job/confirm",{"job_id":jp2,"odeme_hex":odeme})
    kontrol("AYNI odeme ikinci ise sayilamaz -> 409", r.get("_http")==409 and f"#{jp}" in r.get("hata",""), r)
    r=istek(CO+"/job/confirm",{"job_id":jp2,"odeme_hex":ode(havuz,3,n0)})
    kontrol("ayni nonce'lu farkli transfer (payer:nonce) ikinci ise sayilamaz -> 409", r.get("_http")==409, r)
    n1=istek(RPC+f"/nonce/{payer[2:]}")["nonce"]
    r=istek(CO+"/job/confirm",{"job_id":jp2,"odeme_hex":ode(havuz,2,n1+5)})
    kontrol("islenmeyecek (gelecek nonce) odeme -> paid DEGIL (409)", r.get("_http")==409 and not istek(CO+f"/job/{jp2}")["job"]["paid"], r)
    onceden=ode(havuz,2,n1)
    istek(RPC+"/submit",{"hex":onceden})
    r=istek(CO+"/job/confirm",{"job_id":jp2,"odeme_hex":onceden})
    kontrol("zincire onceden (baska yoldan) gonderilmis odeme bu ise kanitlanamaz -> red", r.get("_http") in (400,409) and not istek(CO+f"/job/{jp2}")["job"]["paid"], r)
    n1=istek(RPC+f"/nonce/{payer[2:]}")["nonce"]
    r=istek(CO+"/job/confirm",{"job_id":jp2,"odeme_hex":ode(havuz,2,n1)})
    kontrol("yeni (ayri) odeme ile ikinci is -> paid", r.get("paid") is True, r)

    # bakiyesi yetmeyen payer: vertex Integrated olur ama transfer uygulanmaz -> paid DEGIL
    fakir_key=S+"/fakir.key"; open(fakir_key,"wb").write(b"\x01"+bytes([10])*32); fakir=Cuzdan(10).w
    jf=istek(CO+"/job/create",{"prompt":"ucretli fakir","fee_lsc":2,"payer":fakir})["job_id"]
    tips=",".join(istek(RPC+"/tips").get("tips",[])) or "-"
    hx=subprocess.run(["./soulware-pay",fakir_key,str(NET),havuz,"2","0",str(int(time.time())),tips],capture_output=True,text=True,check=True).stdout.strip()
    r=istek(CO+"/job/confirm",{"job_id":jf,"odeme_hex":hx})
    # Dugum /submit kapi kontrolu bakiyesiz transferi bastan reddeder (ok:false) -> koordinator 400;
    # eski dugumde vertex girip etkisiz kalirdi (409). Her iki durumda is ODENMIS SAYILMAZ.
    kontrol("bakiyesiz payer: transfer uygulanmadi -> paid DEGIL", r.get("_http") in (400, 409) and not istek(CO+f"/job/{jf}")["job"]["paid"], r)

    # ── Gercek soulware-worker ikilisi: imzali register/benchmark/poll/submit ──
    wk=[]
    for i in (1,2):
        kp=S+f"/w{i}.key"
        subprocess.run(["./soulware-worker","--yeni-anahtar-uret"],env=dict(wenv,SOULWARE_WORKER_KEY=kp),check=True,capture_output=True)
        p=subprocess.Popen(["./soulware-worker"],env=dict(wenv,SOULWARE_WORKER_KEY=kp),stdout=open(f"w{i}.out","w"),stderr=subprocess.STDOUT); procs.append(p); wk.append(p)
    jw=istek(CO+"/job/create",{"prompt":"worker ikilisi testi"},ip("192.0.2.60"))["job_id"]
    durum=None
    for _ in range(60):
        durum=istek(CO+f"/job/{jw}")["job"]
        if durum["status"]!="pending": break
        time.sleep(1)
    kontrol("soulware-worker ikilisi (imzali) ile is dogrulandi", durum["status"]=="verified" and len(durum["rewards"])==2, (durum, open("w1.out").read()[-400:]))

    # ── Bulgu 8c: istemci basina gunluk ucretsiz kota (SOULWARE_FREE_DAILY_PER_CLIENT=2) ──
    r=[istek(CO+"/job/create",{"prompt":f"kota {i}"},ip("198.51.100.9")) for i in range(3)]
    kontrol("ayni IP: 2 ucretsiz is ok, 3. RED", r[0].get("ok") and r[1].get("ok") and r[2].get("ok") is False and "günlük" in r[2].get("hata",""), r)
    r=istek(CO+"/job/create",{"prompt":"kota x","payer":payer},ip("198.51.100.10"))
    r2=istek(CO+"/job/create",{"prompt":"kota y","payer":payer},ip("198.51.100.11"))
    r3=istek(CO+"/job/create",{"prompt":"kota z","payer":payer},ip("198.51.100.12"))
    kontrol("ayni cuzdan farkli IP'lerden: 3. RED (cuzdan kotasi)", r.get("ok") and r2.get("ok") and r3.get("ok") is False, (r,r2,r3))
    r=istek(CO+"/job/create",{"prompt":"kota baska"},ip("198.51.100.13"))
    kontrol("baska istemci etkilenmez", r.get("ok"), r)
finally:
    for p in procs: p.terminate()
g=sum(1 for _,o in sonuc if o); print(f"\nSONUC: {g}/{len(sonuc)} gecti")
sys.exit(0 if g==len(sonuc) else 1)
