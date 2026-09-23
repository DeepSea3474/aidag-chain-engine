"""KUBRA denetim duzeltmeleri uctan uca testi (izole devnet, mainnet'e DOKUNMAZ).

Kapsam: tuzlu-v1 -> tuzlu-v2 gecisi + 0x1e alan-ayraci enjeksiyonu (bulgu 1),
istemci baglami (2), fail-closed anahtar (3), yonetim uclari (4), DoS sinirlari (5),
belge dogrulama kaydeden aciklamasi (6), icerik denetimi fail-closed (7).

Kullanim (root; ayri ag ad alani):
  D=$(mktemp -d); cp e2e_v2.py mock_beyin.py $D/
  cp <v1 soulware-core (tuzlu-v1)> $D/soulware-core-ESKI; cp <yeni soulware-core> $D/soulware-core-YENI
  cp <lsc-node> $D/lsc-node
  cd $D && env -u ANTHROPIC_API_KEY unshare -n bash -c 'ip link set lo up && python3 e2e_v2.py'
"""
import json,subprocess,time,urllib.request,urllib.error,os,sys,re,struct,threading
import blake3
S=os.path.dirname(os.path.abspath(__file__)); os.chdir(S)
RPC="http://127.0.0.1:38645"; KUB="http://127.0.0.1:38646"; NET=99999
TOKEN="e2e-yonetim-tokeni-0123456789abcdef"
def istek(u,b=None,h=None,ham=False):
    hh={"content-type":"application/json"}; hh.update(h or {})
    r=urllib.request.Request(u,None if b is None else (b if isinstance(b,bytes) else json.dumps(b).encode()),hh)
    try:
        y=urllib.request.urlopen(r,timeout=60); d=y.read()
        return (y.status,dict(y.headers),d) if ham else json.loads(d)
    except urllib.error.HTTPError as e:
        d=e.read()
        if ham: return (e.code,dict(e.headers),d)
        try: return {"_http":e.code, **json.loads(d)}
        except Exception: return {"_http":e.code,"_govde":d.decode(errors="replace")}
get=lambda u,h=None: istek(u,None,h)
post=lambda u,b,h=None: istek(u,b,h)
def sse(u,b):
    r=urllib.request.Request(u,json.dumps(b).encode(),{"content-type":"application/json"})
    done=None; toks=[]
    for blok in urllib.request.urlopen(r,timeout=60).read().decode().split("\n\n"):
        ev="message"; dl=[]
        for l in blok.split("\n"):
            if l.startswith("event:"): ev=l[6:].strip()
            elif l.startswith("data:"): d=l[5:]; dl.append(d[1:] if d.startswith(" ") else d)
        d="\n".join(dl)
        if ev=="token": toks.append(d)
        if ev=="done": done=json.loads(d)
    return "".join(toks),done
def L(b): return struct.pack('<Q',len(b))+b
def v2(tuz,ts,tur,alanlar):
    g=b"KUBRA-KANIT-v2\0"+L(struct.pack('<I',NET))+L(struct.pack('<Q',ts))+L(tur)+struct.pack('<Q',len(alanlar))+b''.join(L(a) for a in alanlar)
    return blake3.blake3(g,key=bytes.fromhex(tuz)).hexdigest()
def bekle(u):
    for _ in range(240):
        try: return get(u)
        except Exception: time.sleep(0.5)
    raise SystemExit("baslamadi: "+u)
env=dict(os.environ, LSC_NETWORK_ID=str(NET), LSC_RPC_ADDR="127.0.0.1:38645", RUST_LOG="warn")
env.pop("LSC_MAINNET",None)
node=subprocess.Popen(["./lsc-node","/ip4/127.0.0.1/tcp/49101","e2e-data.log"],env=env,stdout=open("node.out","w"),stderr=subprocess.STDOUT)
mock=subprocess.Popen([sys.executable,"mock_beyin.py","38650"])
bekle(RPC+"/status")
kenv=dict(os.environ, SOULWARE_NET_ID=str(NET), SOULWARE_CHAIN_RPC=RPC, SOULWARE_LISTEN="127.0.0.1:38646",
  SOULWARE_KEY_PATH=S+"/test.key", SOULWARE_BRAIN="remote", SOULWARE_REMOTE_URL="http://127.0.0.1:38650/v1/chat/completions",
  SOULWARE_IMAGE_URL="http://127.0.0.1:38650/img",
  SOULWARE_REMOTE_MODEL="mock-7b", SOULWARE_LOCAL_MODEL=S+"/yok.gguf", SOULWARE_EMBED_DIR=S+"/yok",
  SOULWARE_KNOWLEDGE_PATH=S+"/kb.json", SOULWARE_SEED_PATH=S+"/kb.seed.json", SOULWARE_RESMI_PATH=S+"/kb.aidag.json",
  SOULWARE_MODEL_REGISTRY=S+"/registry.json", SOULWARE_GROUND="0")
for k in ("ANTHROPIC_API_KEY","CLAUDE_API_KEY","SOULWARE_YONETIM_TOKEN"): kenv.pop(k,None)
json.dump([{"baslik":"İstanbul","metin":"TEMIZ seed metni istanbul hakkinda.","url":None}],open(S+"/kb.seed.json","w"))
def kubra(bin_,**ek):
    e=dict(kenv,**ek)
    p=subprocess.Popen(["./"+bin_],env=e,stdout=open(bin_+".out","a"),stderr=subprocess.STDOUT); bekle(KUB+"/health"); return p
sonuc=[]
def kontrol(ad,ok,ayrinti=""):
    sonuc.append((ad,bool(ok))); print(("GECTI " if ok else "KALDI ")+ad+("" if ok else f"  <- {ayrinti}"))
try:
    # ── 3) FAIL-CLOSED anahtar: anahtar yoksa baslamaz, bozuk dosyaya dokunmaz ──
    r=subprocess.run(["./soulware-core-YENI"],env=kenv,capture_output=True,text=True,timeout=120)
    kontrol("anahtar YOK -> servis baslamaz (exit 2), dosya olusmaz", r.returncode==2 and not os.path.exists(S+"/test.key"), r.stderr[-300:])
    open(S+"/bozuk.key","wb").write(b"\x01"*10)
    r=subprocess.run(["./soulware-core-YENI"],env=dict(kenv,SOULWARE_KEY_PATH=S+"/bozuk.key"),capture_output=True,text=True,timeout=120)
    kontrol("bozuk anahtar -> baslamaz ve dosyaya DOKUNMAZ", r.returncode==2 and open(S+"/bozuk.key","rb").read()==b"\x01"*10)
    r=subprocess.run(["./soulware-core-YENI","--yeni-anahtar-uret"],env=kenv,capture_output=True,text=True,timeout=120)
    kontrol("--yeni-anahtar-uret: 33 bayt, 0600", r.returncode==0 and os.stat(S+"/test.key").st_mode&0o777==0o600 and os.path.getsize(S+"/test.key")==33)
    r=subprocess.run(["./soulware-core-YENI","--yeni-anahtar-uret"],env=kenv,capture_output=True,text=True,timeout=120)
    kontrol("--yeni-anahtar-uret ikinci kez: REDDEDER (O_EXCL)", r.returncode==2)

    # ── ESKI (tuzlu-v1) ikili: v1 kayitlari + 0x1e'li kayit ──
    k=kubra("soulware-core-ESKI")
    e1=post(KUB+"/v1/ask",{"prompt":"7 çarpı 8"})
    P_AYRAC="Merhaba\x1eGIZLI"
    e2=post(KUB+"/v1/ask",{"prompt":P_AYRAC})
    # SALDIRI (eski sunucuda): ayni baytlari farkli bol
    saldiri={"ts":e2["ts"],"prompt":"Merhaba","answer":"GIZLI\x1e"+e2["answer"],"model":e2["model"],"salt":e2["salt"]}
    v=post(KUB+"/v1/verify",saldiri)
    kontrol("[REFERANS] ESKI sunucuda 0x1e bolunme saldirisi 'dogrulandi:true' aliyordu", v.get("dogrulandi") is True, v)
    k.terminate(); k.wait()

    # ── YENI ikili ──
    k=kubra("soulware-core-YENI",SOULWARE_YONETIM_TOKEN=TOKEN,SOULWARE_BEYIN_ESZAMAN="1")
    v=post(KUB+"/v1/verify",{"ts":e1["ts"],"prompt":"7 çarpı 8","answer":e1["answer"],"model":e1["model"],"salt":e1["salt"],"proof_hash":e1["proof_hash"]})
    kontrol("v1 kaydi (sema verilmeden) geriye uyumlu dogrulanir", v.get("dogrulandi") and v["sema"]=="tuzlu-v1" and v["denenen_semalar"]==["tuzlu-v2","tuzlu-v1"], v)
    v=post(KUB+"/v1/verify",dict(saldiri))
    kontrol("BULGU1: 0x1e saldirisi artik belirsiz + dogrulandi:false", v.get("dogrulandi") is False and v.get("belirsiz") is True and v.get("zincirde") is True and "0x1e" in v.get("sebep",""), v)
    v=post(KUB+"/v1/verify",{"ts":e2["ts"],"prompt":P_AYRAC,"answer":e2["answer"],"model":e2["model"],"salt":e2["salt"]})
    kontrol("BULGU1: 0x1e iceren v1 kaydin mesru bolunmesi de belirsiz (guvenle dogrulanamaz)", v.get("dogrulandi") is False and v.get("belirsiz") is True, v)

    y1=post(KUB+"/v1/ask",{"prompt":P_AYRAC})
    kontrol("YENI kayit sema=tuzlu-v2, istemci_baglami=false", y1.get("sema")=="tuzlu-v2" and y1.get("istemci_baglami") is False and y1["chain"]["submitted"], y1)
    kontrol("YENI kayit hash'i bagimsiz Python v2 formuluyle ayni",
        v2(y1["salt"],y1["ts"],b"sohbet",[P_AYRAC.encode(),y1["answer"].encode(),y1["model"].encode(),b""])==y1["proof_hash"])
    q={"ts":y1["ts"],"prompt":P_AYRAC,"answer":y1["answer"],"model":y1["model"],"salt":y1["salt"]}
    v=post(KUB+"/v1/verify",q)
    kontrol("v2 kaydi (0x1e'li prompt) dogrulanir", v.get("dogrulandi") and v["sema"]=="tuzlu-v2" and not v["belirsiz"], v)
    v=post(KUB+"/v1/verify",dict(q,prompt="Merhaba",answer="GIZLI\x1e"+y1["answer"]))
    kontrol("BULGU1: v2 kaydinda bolunme saldirisi basarisiz (zincirde yok)", v.get("dogrulandi") is False and v.get("zincirde") is False, v)
    v=post(KUB+"/v1/verify",dict(q,sema="tuzlu-v1"))
    kontrol("v2 kaydi sema=tuzlu-v1 zorlanirsa dogrulanmaz", v.get("dogrulandi") is False, v)

    # ── 2) istemci baglami ──
    c1=post(KUB+"/v1/ask",{"prompt":"Bu baglama gore ozetle","context":"GIZLI BAGLAM 42"})
    kontrol("BULGU2: context verilince istemci_baglami=true", c1.get("istemci_baglami") is True and c1.get("sema")=="tuzlu-v2", c1)
    q={"ts":c1["ts"],"prompt":"Bu baglama gore ozetle","answer":c1["answer"],"model":c1["model"],"salt":c1["salt"]}
    v=post(KUB+"/v1/verify",dict(q,context="GIZLI BAGLAM 42"))
    kontrol("BULGU2: dogru context ile dogrulanir", v.get("dogrulandi"), v)
    v=post(KUB+"/v1/verify",q)
    kontrol("BULGU2: context verilmeden dogrulanmaz", v.get("dogrulandi") is False, v)
    v=post(KUB+"/v1/verify",dict(q,context="SAHTE BAGLAM"))
    kontrol("BULGU2: degistirilmis context ile dogrulanmaz", v.get("dogrulandi") is False, v)
    st_metin,st=sse(KUB+"/v1/ask-stream",{"prompt":"Merhaba, akis","context":"AKIS BAGLAMI"})
    v=post(KUB+"/v1/verify",{"ts":st["ts"],"prompt":"Merhaba, akis","answer":st["answer"],"model":st["model"],"salt":st["salt"],"context":"AKIS BAGLAMI"})
    kontrol("akis: done'da sema=tuzlu-v2 + istemci_baglami, dogrulanir", st.get("sema")=="tuzlu-v2" and st.get("istemci_baglami") is True and st["answer"]==st_metin and v.get("dogrulandi"), (st,v))
    a1=post(KUB+"/v1/ask",{"prompt":"7 çarpı 8"})
    v=post(KUB+"/v1/verify",{"ts":a1["ts"],"prompt":"7 çarpı 8","answer":a1["answer"],"model":a1["model"],"salt":a1["salt"]})
    kontrol("arac cevabi v2 ile dogrulanir", a1["sema"]=="tuzlu-v2" and v.get("dogrulandi"), v)

    # ── 6) belge dogrulama: KUBRA'nin kendi kaydi belge DEGIL ──
    b=post(KUB+"/v1/ask",{"prompt":y1["proof_hash"]+" bu belgeyi doğrula"})
    kontrol("BULGU6: KUBRA etkilesim hash'i -> 'belge kaydi DEGILDIR', 'birebir' YOK", "KUBRA etkileşim kaydıdır" in b["answer"] and "birebir" not in b["answer"], b["answer"])

    # ── 5) DoS: govde siniri, eszamanlilik, claude secimi, info ──
    r=post(KUB+"/v1/ask",{"prompt":"x"*(40*1024)})
    kontrol("BULGU5: /v1/ask 40 KB govde -> 413", r.get("_http")==413, r)
    r=istek(KUB+"/v1/ask-stream",{"prompt":"x"*(40*1024)},ham=True)
    kontrol("BULGU5: /v1/ask-stream 40 KB govde -> 413", r[0]==413, r[0])
    yavas={}
    t=threading.Thread(target=lambda: yavas.update(post(KUB+"/v1/ask",{"prompt":"YAVAS bir soru lutfen"})))
    t.start(); time.sleep(0.8)
    r=post(KUB+"/v1/ask",{"prompt":"ikinci eszamanli soru"})
    r2=istek(KUB+"/v1/ask-stream",{"prompt":"ucuncu"},ham=True)
    t.join()
    kontrol("BULGU5: eszamanlilik dolu (N=1) -> /v1/ask 429", r.get("_http")==429, r)
    kontrol("BULGU5: eszamanlilik dolu -> /v1/ask-stream 429", r2[0]==429, r2[0])
    kontrol("BULGU5: yavas istek yine basarili; sonra kapasite geri gelir", yavas.get("ok") and post(KUB+"/v1/ask",{"prompt":"sonraki soru"}).get("ok"), yavas)
    r=post(KUB+"/v1/ask",{"prompt":"Merhaba claude","brain":"claude"})
    kontrol("BULGU5: brain=claude yok sayilir (sunucu tercihi: kubra-gpu)", r.get("ok") and r.get("brain")=="kubra-gpu", r)
    i=get(KUB+"/")
    kontrol("BULGU5: / (info) claude ve zincir RPC bilgisini vermez", "claude" not in i and "zincir_rpc" not in i and "38645" not in json.dumps(i), i)

    # ── 4) yonetim uclari ──
    H={"Authorization":"Bearer "+TOKEN}
    for yol,gov in (("/kb/stats",None),("/models",None),("/retrieve",{"prompt":"x"}),("/kb/ingest",{"baslik":"Deneme","metin":"yeterince uzun metin burada"})):
        r=istek(KUB+yol,gov,ham=True); r2=istek(KUB+yol,gov,{"Authorization":"Bearer yanlis-token-0123456789"},ham=True)
        kontrol(f"BULGU4: {yol} tokensiz 401, yanlis token 401", r[0]==401 and r2[0]==401, (r[0],r2[0]))
    r=istek(KUB+"/kb/stats",None,H)
    kontrol("BULGU4: /kb/stats dogru token -> 200 (yol sizdirmaz)", r.get("ok") and "yol" not in r, r)
    r=istek(KUB+"/kb/ingest",{"baslik":"Deneme","metin":"yeterince uzun metin burada"},H)
    kontrol("BULGU4: /kb/ingest dogru token -> ok", r.get("ok"), r)
    r=istek(KUB+"/embed-test",None,H,ham=True)
    kontrol("BULGU4: /embed-test kaldirildi (404)", r[0]==404, r[0])
    r=istek(KUB+"/kb/ingest",{"baslik":"B","metin":"m"*(300*1024)},H,ham=True)
    kontrol("BULGU4: /kb/ingest 300 KB govde -> 413", r[0]==413, r[0])
    r=istek(KUB+"/kb/ingest",{"baslik":"b"*600,"metin":"yeterince uzun metin"},H,ham=True)
    kontrol("BULGU4: /kb/ingest baslik > 512 bayt -> 413", r[0]==413, r[0])
    for zehir in ("ISTANBUL","ıstanbul","İstan​bul"):
        r=istek(KUB+"/kb/ingest",{"baslik":zehir,"metin":"ZEHIRLI metin, seed'i ezmeye calisir"},H)
        kontrol(f"BULGU4: korumali baslik varyanti reddedilir ({zehir!r})", r.get("_http")==409, r)
    kb=json.load(open(S+"/kb.json"))
    kontrol("BULGU4: diskte seed metni korunmus, zehirli metin yok", not any("ZEHIRLI" in b["metin"] for b in kb))

    # ── 7) icerik denetimi fail-closed + medya v2 ──
    for istem,beklenen in (("guzel bir kedi",200),("ENGELLE bunu",422),("HATALI yargic",422),("BOZUK yanit",422)):
        kod,hd,gov=istek(KUB+"/v1/image",{"prompt":istem,"wallet":"0xabc"},ham=True)
        kontrol(f"BULGU7: /v1/image '{istem}' -> {beklenen}", kod==beklenen, kod)
        if kod==200:
            hd={x.lower():y for x,y in hd.items()}
            kontrol("medya: x-kubra-sema=tuzlu-v2 ve hash bagimsiz v2 formuluyle ayni",
                hd.get("x-kubra-sema")=="tuzlu-v2" and v2(hd["x-kubra-salt"],int(hd["x-kubra-ts"]),b"medya",[istem.encode(),b"0xabc",gov])==hd["x-kubra-proof"], hd)
    k.terminate(); k.wait()
    # Token olmadan: yonetim uclari KAPALI (403); yargic yoksa gorsel REDDEDILIR
    k=kubra("soulware-core-YENI",SOULWARE_REMOTE_URL="")
    for yol,gov in (("/kb/stats",None),("/models",None),("/retrieve",{"prompt":"x"}),("/kb/ingest",{"baslik":"a","metin":"yeterince uzun metin"})):
        r=istek(KUB+yol,gov,{"Authorization":"Bearer "+TOKEN},ham=True)
        kontrol(f"BULGU4: token env yok -> {yol} 403", r[0]==403, r[0])
    kod,_,_=istek(KUB+"/v1/image",{"prompt":"guzel bir kedi"},ham=True)
    kontrol("BULGU7: yargic yapilandirilmamis -> gorsel reddedilir (422)", kod==422, kod)
    k.terminate(); k.wait()
finally:
    node.terminate(); mock.terminate()
g=sum(1 for _,o in sonuc if o); print(f"\nSONUC: {g}/{len(sonuc)} gecti")
sys.exit(0 if g==len(sonuc) else 1)
