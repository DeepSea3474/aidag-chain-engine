"""KUBRA tuzlu kanit uctan uca testi (izole devnet, mainnet'e DOKUNMAZ).

Kullanim (root; ayri ag ad alani -> mDNS canli dugumu bulamaz):
  D=$(mktemp -d); cp e2e.py mock_beyin.py $D/
  cp <eski soulware-core> $D/soulware-core-ESKI; cp <yeni soulware-core> $D/soulware-core-YENI
  cp <lsc-node> $D/lsc-node
  cd $D && unshare -n bash -c 'ip link set lo up && python3 e2e.py'
Eski ikili tuzsuz kayit uretir; yeni ikili onlari /v1/verify ile dogrular ve
tuzlu kayit uretir. Tum KUBRA dosya yollari gecici dizindedir.
"""
import json,subprocess,time,urllib.request,os,sys,re
S=os.path.dirname(os.path.abspath(__file__)); os.chdir(S)
RPC="http://127.0.0.1:28645"; KUB="http://127.0.0.1:28646"; NET="99999"
def get(u): return json.loads(urllib.request.urlopen(u,timeout=30).read())
def post(u,b):
    r=urllib.request.Request(u,json.dumps(b).encode(),{"content-type":"application/json"})
    try: return json.loads(urllib.request.urlopen(r,timeout=60).read())
    except urllib.error.HTTPError as e: return {"_http":e.code, **json.loads(e.read())}
def sse(u,b):
    r=urllib.request.Request(u,json.dumps(b).encode(),{"content-type":"application/json"})
    done=None; toks=[]
    for blok in urllib.request.urlopen(r,timeout=60).read().decode().split("\n\n"):
        ev="message"; dl=[]
        for l in blok.split("\n"):
            if l.startswith("event:"): ev=l[6:].strip()
            elif l.startswith("data:"): d=l[5:]; dl.append(d[1:] if d.startswith(" ") else d)
        d="\n".join(dl)   # SSE: cok satirli data \n ile birlesir
        if ev=="token": toks.append(d)
        if ev=="done": done=json.loads(d)
    return "".join(toks),done
def sayfa_kanit(proof,soru,metin):
    """ask.html'deki GERCEK kanitVerisi() fonksiyonunu node ile calistir."""
    html=open(os.environ.get("ASK_HTML",S+"/ask.html"),encoding="utf-8").read()
    fn=html[html.index("// KANIT-BASLA"):html.index("// KANIT-BITIS")]
    js=fn+"\nconst a=JSON.parse(require('fs').readFileSync(0,'utf8'));process.stdout.write(JSON.stringify(kanitVerisi(a[0],a[1],a[2])));"
    return json.loads(subprocess.run(["node","-e",js],input=json.dumps([proof,soru,metin]),capture_output=True,text=True,check=True).stdout)
def bekle(u):
    for _ in range(120):
        try: return get(u)
        except Exception: time.sleep(0.5)
    raise SystemExit("baslamadi: "+u)
env=dict(os.environ, LSC_NETWORK_ID=NET, LSC_RPC_ADDR="127.0.0.1:28645", RUST_LOG="warn")
env.pop("LSC_MAINNET",None)
node=subprocess.Popen(["./lsc-node","/ip4/127.0.0.1/tcp/49001","e2e-data.log"],env=env,stdout=open("node.out","w"),stderr=subprocess.STDOUT)
mock=subprocess.Popen([sys.executable,"mock_beyin.py"])
bekle(RPC+"/status")
kenv=dict(os.environ, SOULWARE_NET_ID=NET, SOULWARE_CHAIN_RPC=RPC, SOULWARE_LISTEN="127.0.0.1:28646",
  SOULWARE_KEY_PATH=S+"/test.key", SOULWARE_BRAIN="remote", SOULWARE_REMOTE_URL="http://127.0.0.1:28650/v1/chat/completions",
  SOULWARE_REMOTE_MODEL="mock-7b", SOULWARE_LOCAL_MODEL=S+"/yok.gguf", SOULWARE_EMBED_DIR=S+"/yok",
  SOULWARE_KNOWLEDGE_PATH=S+"/kb.json", SOULWARE_SEED_PATH=S+"/kb.seed.json", SOULWARE_RESMI_PATH=S+"/kb.aidag.json",
  SOULWARE_MODEL_REGISTRY=S+"/registry.json", SOULWARE_GROUND="0")
for k in ("ANTHROPIC_API_KEY","CLAUDE_API_KEY"): kenv.pop(k,None)
def kubra(bin_):
    p=subprocess.Popen(["./"+bin_],env=kenv,stdout=open(bin_+".out","w"),stderr=subprocess.STDOUT); bekle(KUB+"/health"); return p
sonuc=[]
def kontrol(ad,ok): sonuc.append((ad,bool(ok))); print(("GECTI " if ok else "KALDI ")+ad)
try:
    # ── 1) ESKI ikili (canlidaki surumun kopyasi): tuzsuz kayitlar uret ──
    k=kubra("soulware-core-ESKI")
    e1=post(KUB+"/v1/ask",{"prompt":"7 çarpı 8"})            # arac: prompt|cevap|arac
    e2=post(KUB+"/v1/ask",{"prompt":"Merhaba, bir sey sorabilir miyim?"})  # beyin: prompt|cevap|model
    e3_metin,e3=sse(KUB+"/v1/ask-stream",{"prompt":"Merhaba, akis testi"})  # akis: prompt|metin
    k.terminate(); k.wait()
    for ad,r in (("eski arac",e1),("eski beyin",e2),("eski akis",e3)):
        kontrol(f"{ad}: zincire yazildi, yanitta salt YOK", r["chain"]["submitted"] and "salt" not in r)
    # ts eski yanitta yok -> zincirdeki kayit zamanindan al (vertex ts = hash ts)
    ts=lambda h: get(RPC+"/belge/"+h)["zaman"]
    # ── 2) YENI ikili ──
    k=kubra("soulware-core-YENI")
    v=post(KUB+"/v1/verify",{"ts":ts(e1["proof_hash"]),"prompt":"7 çarpı 8","answer":e1["answer"],"model":e1["model"],"proof_hash":e1["proof_hash"]})
    kontrol("ESKI arac kaydi tuzsuz dogrulanir", v["dogrulandi"] and v["sema"]=="eski-tuzsuz" and v["proof_eslesir"])
    v=post(KUB+"/v1/verify",{"ts":ts(e2["proof_hash"]),"prompt":"Merhaba, bir sey sorabilir miyim?","answer":e2["answer"],"model":e2["model"]})
    kontrol("ESKI beyin kaydi tuzsuz dogrulanir", v["dogrulandi"] and v["proof_hash"]==e2["proof_hash"])
    v=post(KUB+"/v1/verify",{"ts":ts(e3["proof_hash"]),"prompt":"Merhaba, akis testi","answer":e3_metin})
    kontrol("ESKI akis kaydi tuzsuz (modelsiz) dogrulanir", v["dogrulandi"] and v["proof_hash"]==e3["proof_hash"])
    v=post(KUB+"/v1/verify",{"ts":ts(e2["proof_hash"]),"prompt":"Merhaba, bir sey sorabilir miyim?","answer":e2["answer"]+"x","model":e2["model"]})
    kontrol("ESKI kayit: degistirilmis cevap dogrulanmaz", not v["dogrulandi"] and not v["zincirde"])

    y1=post(KUB+"/v1/ask",{"prompt":"7 çarpı 8"})
    y2=post(KUB+"/v1/ask",{"prompt":"Merhaba, bir sey sorabilir miyim?"})
    y3_metin,y3=sse(KUB+"/v1/ask-stream",{"prompt":"Merhaba, akis testi"})
    y4_metin,y4=sse(KUB+"/v1/ask-stream",{"prompt":"7 çarpı 8"})
    tuzlar=[]
    for ad,r,cevap,prompt in (("arac",y1,y1["answer"],"7 çarpı 8"),("beyin",y2,y2["answer"],"Merhaba, bir sey sorabilir miyim?"),
                             ("akis-beyin",y3,y3_metin,"Merhaba, akis testi"),("akis-arac",y4,y4_metin,"7 çarpı 8")):
        s=r.get("salt",""); tuzlar.append(s)
        kontrol(f"YENI {ad}: zincire yazildi + salt(64 hex) + ts donuyor", r["chain"]["submitted"] and re.fullmatch("[0-9a-f]{64}",s) and r.get("ts"))
        kontrol(f"YENI {ad}: ChainProof icinde salt YOK", s not in json.dumps(r["chain"]))
        b=get(RPC+"/belge/"+r["proof_hash"])
        kontrol(f"YENI {ad}: zincirde kayitli, zaman == ts", b["kayitli"] and b["zaman"]==r["ts"])
        q={"ts":r["ts"],"prompt":prompt,"answer":cevap,"model":r["model"],"proof_hash":r["proof_hash"]}
        v=post(KUB+"/v1/verify",dict(q,salt=s))
        kontrol(f"YENI {ad}: salt ile dogrulanir", v["dogrulandi"] and v["sema"]=="tuzlu-v1" and v["kubra_imzali"])
        v=post(KUB+"/v1/verify",q)
        kontrol(f"YENI {ad}: salt OLMADAN dogrulanmaz", not v["dogrulandi"] and v["proof_eslesir"] is False)
        v=post(KUB+"/v1/verify",dict(q,salt="00"*32))
        kontrol(f"YENI {ad}: yanlis salt ile dogrulanmaz", not v["dogrulandi"])
    kontrol("YENI akis: done.answer == hash'lenen metin (satir sonlari dahil)", y3["answer"]==y3_metin and "\n" in y3["answer"])
    for ad,r,soru,metin in (("akis-beyin",y3,"Merhaba, akis testi",y3_metin),("akis-arac",y4,"7 çarpı 8",y4_metin)):
        veri=sayfa_kanit(r,soru,metin)
        kontrol(f"SAYFA kanit dosyasi alanlari tam ({ad})", all(veri.get(k) for k in ("prompt","answer","model","ts","salt","proof_hash")))
        v=post(KUB+"/v1/verify",{k:veri[k] for k in ("ts","prompt","answer","model","salt","proof_hash")})
        kontrol(f"SAYFA kanit dosyasi /v1/verify'da dogrulanir ({ad})", v["dogrulandi"])
    # eski sunucu (prompt/answer alani yok) -> sayfa kendi metnine duser
    veri=sayfa_kanit({"proof_hash":"ab"*32,"salt":"cd"*32,"ts":5,"model":"m"},"soru","metin")
    kontrol("SAYFA: done'da answer yoksa sayfadaki metin kullanilir", veri["prompt"]=="soru" and veri["answer"]=="metin")
    kontrol("YENI: ayni soru (7 carpi 8) iki kez -> farkli hash (tahmin edilemez)", y1["proof_hash"]!=y4["proof_hash"])
    kontrol("YENI: tum tuzlar birbirinden farkli", len(set(tuzlar))==len(tuzlar))
    v=post(KUB+"/v1/verify",{"ts":1,"prompt":"a","answer":"b","model":"m","salt":"zz"})
    kontrol("gecersiz salt -> HTTP 400", v.get("_http")==400)
    k.terminate(); k.wait()
    # ── 3) Zincir verisinde (diskteki tum vertex'ler) hicbir tuz yok ──
    veri=open("e2e-data.log","rb").read()
    kontrol("zincir veri dosyasinda hicbir tuz (hex veya ham bayt) YOK",
        all(t.encode() not in veri and bytes.fromhex(t) not in veri for t in tuzlar))
    kontrol("zincir veri dosyasinda proof_hash'ler VAR (kontrol gecerli)", y1["proof_hash"].encode() in veri or bytes.fromhex(y1["proof_hash"]) in veri)
finally:
    node.terminate(); mock.terminate()
g=sum(1 for _,o in sonuc if o); print(f"\nSONUC: {g}/{len(sonuc)} gecti")
