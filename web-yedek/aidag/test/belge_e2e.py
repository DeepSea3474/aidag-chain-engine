"""Belge araclari uctan uca testi — IZOLE ag ad alaninda (mainnet'e DOKUNMAZ).

Uretimdeki yolun aynisi: nginx (aday site ayari) -> /belge/<hash>, /lib, /rpc (devnet dugum :8645),
/kubra-brain (yeni soulware-core :8646). Sayfanin tarayici modulleri (lib/belge-kayit.js, blake3.js,
nacl.min.js) node ile calistirilir.

Kullanim (root):
  BIN=<lsc-node, soulware-core, belge-imzala klasoru> REPO=<repo koku> SITE=<aday nginx site dosyasi> \
  unshare -n bash -c 'ip link set lo up && python3 web-yedek/aidag/test/belge_e2e.py'
Gerekenler: nginx, node, python3 + blake3 paketi.
"""
import json, os, shutil, subprocess, sys, tempfile, time, urllib.request, urllib.error

BIN, REPO, SITE = os.environ["BIN"], os.environ["REPO"], os.environ["SITE"]
T = tempfile.mkdtemp(prefix="belge-e2e-")
NET = 3474  # izole devnet (ag ad alani disina cikamaz); sayfa 3474 bekler
H = "http://127.0.0.1"
YARDIMCI = os.path.join(REPO, "web-yedek/aidag/test/js_yardimci.js")

sonuc = []
def kontrol(ad, ok, ek=""):
    sonuc.append(bool(ok)); print(("GECTI " if ok else "KALDI ") + ad + (f"  [{ek}]" if ek else "")); sys.stdout.flush()

def istek(yol, govde=None, ham=None, basliklar=None):
    b = {"Host": "aidag-chain.com"}
    b.update(basliklar or {})
    veri = ham if ham is not None else (json.dumps(govde).encode() if govde is not None else None)
    if govde is not None: b["Content-Type"] = "application/json"
    r = urllib.request.Request(H + yol, veri, b)
    try:
        with urllib.request.urlopen(r, timeout=60) as y: return y.status, y.read()
    except urllib.error.HTTPError as e: return e.code, e.read()
def jget(yol): return json.loads(istek(yol)[1])
def jpost(yol, g): s, b = istek(yol, g); return s, json.loads(b)
def js(islem, arg):
    return json.loads(subprocess.run(["node", YARDIMCI, islem, json.dumps(arg)], capture_output=True, text=True, check=True,
                                     env=dict(os.environ, WEB_KOK=WEB)).stdout)
def bekle(url, n=120):
    for _ in range(n):
        try: return urllib.request.urlopen(url, timeout=2).read()
        except Exception: time.sleep(0.5)
    raise SystemExit("baslamadi: " + url)
def anahtar_yaz(ad, seed_bayt):
    p = os.path.join(T, ad); open(p, "wb").write(bytes([1]) + bytes([seed_bayt]) * 32); return p

# ── Web kokunu kur: sayfa + yeni lib + mevcut blake3/nacl ──
WEB = os.path.join(T, "www"); os.makedirs(WEB)
shutil.copy(os.path.join(REPO, "web-yedek/aidag/belge-dogrulama.html"), WEB)
shutil.copytree(os.path.join(REPO, "web-yedek/aidag/lib"), os.path.join(WEB, "lib"))
for f in ("blake3.js", "nacl.min.js"): shutil.copy(os.path.join(REPO, "web/lib", f), os.path.join(WEB, "lib"))
site = open(SITE).read().replace("/var/www/aidag", WEB)
open(os.path.join(T, "site.conf"), "w").write(site)
open(os.path.join(T, "nginx.conf"), "w").write(f"""pid {T}/nginx.pid; error_log {T}/nginx-err.log;
events {{}} http {{ include /etc/nginx/mime.types; access_log off; client_body_temp_path {T}; proxy_temp_path {T};
  include {T}/site.conf; }}""")

sureclar = []
try:
    env = dict(os.environ, LSC_NETWORK_ID=str(NET), LSC_RPC_ADDR="127.0.0.1:8645", RUST_LOG="warn"); env.pop("LSC_MAINNET", None)
    sureclar.append(subprocess.Popen([f"{BIN}/lsc-node", "/ip4/127.0.0.1/tcp/49011", f"{T}/veri.log"], env=env, cwd=T,
                                     stdout=open(f"{T}/node.out", "w"), stderr=subprocess.STDOUT))
    bekle("http://127.0.0.1:8645/status")
    kenv = dict(os.environ, SOULWARE_NET_ID=str(NET), SOULWARE_CHAIN_RPC="http://127.0.0.1:8645", SOULWARE_LISTEN="127.0.0.1:8646",
                SOULWARE_KEY_PATH=f"{T}/kubra.key", SOULWARE_BRAIN="remote", SOULWARE_REMOTE_URL="http://127.0.0.1:9/yok",
                SOULWARE_LOCAL_MODEL=f"{T}/yok", SOULWARE_EMBED_DIR=f"{T}/yok", SOULWARE_KNOWLEDGE_PATH=f"{T}/kb.json",
                SOULWARE_SEED_PATH=f"{T}/seed.json", SOULWARE_RESMI_PATH=f"{T}/resmi.json", SOULWARE_MODEL_REGISTRY=f"{T}/reg.json", SOULWARE_GROUND="0")
    for k in ("ANTHROPIC_API_KEY", "CLAUDE_API_KEY"): kenv.pop(k, None)
    # KUBRA anahtari fail-closed: yoksa servis baslamaz -> once acikca uret.
    subprocess.run([f"{BIN}/soulware-core", "--yeni-anahtar-uret"], env=kenv, cwd=T, check=True, capture_output=True)
    sureclar.append(subprocess.Popen([f"{BIN}/soulware-core"], env=kenv, cwd=T, stdout=open(f"{T}/kubra.out", "w"), stderr=subprocess.STDOUT))
    bekle("http://127.0.0.1:8646/health")
    subprocess.run(["nginx", "-c", f"{T}/nginx.conf"], check=True)
    time.sleep(0.5)
    KUBRA_ADRES = json.loads(urllib.request.urlopen("http://127.0.0.1:8646/").read())["imzalayan"]

    # ── nginx yonlendirmesi (aday ayar) ──
    ornek = "ab" * 32
    s, b = istek(f"/belge/{ornek}")
    kontrol("nginx: /belge/<64 hex> belge sayfasini dondurur", s == 200 and "Kayıt Talebi Hazırla".encode() in b)
    s, b = istek(f"/belge/{ornek.upper()}")
    kontrol("nginx: buyuk harfli hash de eslesir", s == 200 and b"belge-kayit.js" in b)
    s, b = istek(f"/belge/{ornek[:63]}")
    kontrol("nginx: 63 haneli yol belge sayfasina GITMEZ", "Kayıt Talebi Hazırla".encode() not in b, f"HTTP {s}")
    s, b = istek(f"/belge/{ornek}/../../etc/passwd")
    kontrol("nginx: yol gezinme denemesi sayfa/dosya dondurmez", b"root:" not in b and "Kayıt Talebi".encode() not in b, f"HTTP {s}")
    s, b = istek("/belge")
    kontrol("nginx: mevcut /belge adresi calismaya devam eder", s == 200 and "Kayıt Talebi Hazırla".encode() in b)
    for f in ("belge-kayit.js", "belge-cikti.js", "bwip-js-4.11.4.min.js", "jspdf-4.2.1.umd.min.js", "fonts/DejaVuSans-tr.ttf", "blake3.js", "nacl.min.js"):
        s, b = istek(f"/lib/{f}")
        kontrol(f"nginx: /lib/{f} yerelden sunulur", s == 200 and len(b) > 1000, f"{len(b)} bayt")
    sayfa = istek(f"/belge/{ornek}")[1].decode()
    dis = [u for u in ("cdn", "unpkg", "jsdelivr", "googleapis", "cloudflare.com/ajax") if u in sayfa]
    kontrol("sayfa: dis kaynaktan betik yuklemez", not dis, ",".join(dis))
    kontrol("sayfa: demo/tarayici anahtari ile kayit YOK", "aidag_demo_seed" not in sayfa and "demoWallet" not in sayfa)

    # ── Tarayicidaki blake3 == referans blake3 (dosya ozeti dogru) ──
    import blake3
    hatali = []
    for n in (0, 1, 63, 64, 65, 1023, 1024, 1025, 2048, 4097, 100_003, 1_048_577):
        p = os.path.join(T, f"b{n}"); veri = os.urandom(n); open(p, "wb").write(veri)
        if js("blake3", {"dosya": p})["hash"] != blake3.blake3(veri).hexdigest(): hatali.append(n)
    kontrol("sayfa blake3 (tarayici) = referans blake3, 0 B..1 MB", not hatali, str(hatali))

    # ── Geriye uyum: eski (standart disi) ozet ──
    eski_js = os.path.join(T, "blake3-eski.js")
    open(eski_js, "wb").write(subprocess.run(["git", "-C", REPO, "show", "986320a^:web/lib/blake3.js"], capture_output=True, check=True).stdout)
    fark = []
    for n in (1025, 5000, 100_003):
        p = os.path.join(T, f"e{n}"); open(p, "wb").write(os.urandom(n))
        ad = js("adaylar", {"dosya": p})
        if not (len(ad) == 2 and ad[1]["yontem"] == "eski" and ad[1]["hash"] == js("eski_canli", {"dosya": p, "eski_js": eski_js})["hash"]): fark.append(n)
    kontrol("eski ozet = duzeltme oncesi (986320a^) blake3.js ciktisi (>1 KB)", not fark, str(fark))
    p = os.path.join(T, "kucuk"); open(p, "wb").write(os.urandom(900))
    kontrol("<=1 KB girdide tek aday (eski = standart)", len(js("adaylar", {"dosya": p})) == 1)

    # ── Belge ve anahtarlar ──
    belge = os.path.join(T, "diploma.pdf"); open(belge, "wb").write(b"%PDF-1.4 ornek diploma " + os.urandom(3000))
    HASH = js("blake3", {"dosya": belge})["hash"]
    personel = anahtar_yaz("personel.key", 0x42); P = js("anahtar", {"anahtar": personel})
    yabanci = anahtar_yaz("yabanci.key", 0x77); Y = js("anahtar", {"anahtar": yabanci})
    say = lambda: jget("/rpc/status")["vertex_count"]

    # ── 1) KUBRA talep hazirlar, IMZALAMAZ ──
    once = say()
    s, r = jpost("/kubra-brain/v1/belge/hazirla", {"hash": HASH}); t = r.get("talep", {})
    kontrol("hazirla: talep doner (imzalayan-bekleniyor)", s == 200 and t.get("durum") == "imzalayan-bekleniyor" and t.get("kubra_imzalamaz") is True)
    kontrol("hazirla: kanit izi alanlari (arac, hash, zincir durumu)", t.get("arac") == "belge-kayit-hazirla" and t.get("belge_hash") == HASH and t["zincir"]["kayitli"] is False)
    kontrol("hazirla: talepte imza/gizli anahtar alani yok", not any(k in json.dumps(t) for k in ("signature", "secret", "seed")))
    time.sleep(1)
    kontrol("hazirla: KUBRA zincire HICBIR SEY gondermedi", say() == once, f"{once} -> {say()}")
    kontrol("hazirla: belge zincirde hala kayitsiz", jget(f"/rpc/belge/{HASH}")["kayitli"] is False)
    s, _ = istek("/kubra-brain/v1/belge/hazirla", ham=json.dumps({"hash": HASH, "x": "A" * 4000}).encode(), basliklar={"Content-Type": "application/json"})
    kontrol("hazirla: buyuk govde (dosya) reddedilir", s == 413, f"HTTP {s}")
    for kotu in (HASH[:63], HASH + "00", "zz" * 32, ""):
        s, _ = jpost("/kubra-brain/v1/belge/hazirla", {"hash": kotu})
        if s != 400: break
    kontrol("hazirla: bozuk hash -> 400", s == 400)
    s, _ = jpost("/kubra-brain/v1/belge/hazirla", {"hash": HASH, "imzalayan_pubkey": "00" * 31})
    kontrol("hazirla: bozuk acik anahtar -> 400", s == 400)

    # ── 2) Kurum olmayan anahtar: talep imzaya kapali ──
    s, r = jpost("/kubra-brain/v1/belge/hazirla", {"hash": HASH, "imzalayan_pubkey": Y["pubkey"]})
    kontrol("kurum olmayan imzalayan -> 'imzalayan-kurum-degil', imzalanacak_id yok", r["talep"]["durum"] == "imzalayan-kurum-degil" and r["talep"]["imzalanacak_id"] is None)

    # ── Personelin kurum kaydi (test hazirligi; tip=5, beyan) ──
    tips = jget("/rpc/tips")["tips"]
    kh = js("kurum", {"anahtar": personel, "net": NET, "ts": int(time.time()), "parents": tips, "kategori": 0, "ad": "Örnek Tapu Müdürlüğü"})["hex"]
    jpost("/rpc/submit", {"hex": kh})
    k = jget("/rpc/kurum/" + P["adres"][2:])
    kontrol("hazirlik: personel adresi kurum olarak kayitli (beyan)", k.get("kayitli") is True and k.get("ad") == "Örnek Tapu Müdürlüğü")

    # ── 3) Personel talebi: Rust id == JS id ──
    s, r = jpost("/kubra-brain/v1/belge/hazirla", {"hash": HASH, "imzalayan_pubkey": P["pubkey"]}); t = r["talep"]
    kontrol("kurum personeli -> 'imza-bekliyor', kurum beyan olarak gorunur", t["durum"] == "imza-bekliyor" and t["imzalayan"]["adres"] == P["adres"] and t["imzalayan"]["kurum"]["ad"] == "Örnek Tapu Müdürlüğü")
    kontrol("imzalanacak_id: KUBRA (Rust) == sayfa (JS)", t["imzalanacak_id"] == js("id", {"talep": t, "pubkey": P["pubkey"]})["id"])

    # ── Sayfa, degistirilmis/yanlis talebi imzalamaz ──
    kotu = dict(t, payload_hex="04" + HASH)
    kontrol("sayfa: payload degistirilmis talebi IMZALAMAZ", not js("imzala", {"anahtar": personel, "talep": kotu, "ts": 1, "beklenen": HASH, "net": NET})["ok"])
    kontrol("sayfa: baska belgenin talebini IMZALAMAZ", not js("imzala", {"anahtar": personel, "talep": t, "ts": 1, "beklenen": "cd" * 32, "net": NET})["ok"])
    kontrol("sayfa: baska ag icin talebi IMZALAMAZ", not js("imzala", {"anahtar": personel, "talep": dict(t, network_id=1), "ts": 1, "beklenen": HASH, "net": NET})["ok"])
    kontrol("sayfa: baska imzalayici icin hazirlanmis talebi IMZALAMAZ", not js("imzala", {"anahtar": yabanci, "talep": t, "ts": 1, "beklenen": HASH, "net": NET})["ok"])

    # ── 4) Sayfa yolu: personel imzalar -> /rpc/submit (sayfa gibi duz hex govde) ──
    im = js("imzala", {"anahtar": personel, "talep": t, "ts": int(time.time()), "beklenen": HASH, "net": NET})
    s, b = istek("/rpc/submit", ham=im["hex"].encode())
    b1 = jget(f"/rpc/belge/{HASH}")
    kontrol("SAYFA: personel imzasiyla belge zincire kaydedildi", im["ok"] and b1["kayitli"] is True, json.loads(b).get("sonuc", "")[:40])
    kontrol("SAYFA: kaydeden = personel adresi (KUBRA DEGIL)", "0x" + b1["kaydeden"] == P["adres"] and "0x" + b1["kaydeden"] != KUBRA_ADRES)
    s, r = jpost("/kubra-brain/v1/belge/hazirla", {"hash": HASH, "imzalayan_pubkey": P["pubkey"]})
    kontrol("tekrar hazirla -> 'zaten-kayitli' (kanit izinde kaydeden)", r["talep"]["durum"] == "zaten-kayitli" and r["talep"]["zincir"]["kaydeden"] == P["adres"])
    kontrol("sayfa: zaten kayitli talebi IMZALAMAZ", not js("imzala", {"anahtar": personel, "talep": r["talep"], "ts": 1, "beklenen": HASH, "net": NET})["ok"])

    # ── 5) Cevrimdisi CLI yolu (belge-imzala) ──
    H2 = blake3.blake3(b"ikinci belge " + os.urandom(64)).hexdigest()
    s, r = jpost("/kubra-brain/v1/belge/hazirla", {"hash": H2, "imzalayan_pubkey": P["pubkey"]})
    tp = os.path.join(T, "talep.json"); json.dump(r["talep"], open(tp, "w"))
    cli = subprocess.run([f"{BIN}/belge-imzala", personel, tp], capture_output=True, text=True)
    jpost("/rpc/submit", {"hex": cli.stdout.strip()})
    b2 = jget(f"/rpc/belge/{H2}")
    kontrol("CLI: belge-imzala ile kayit, kaydeden = personel", cli.returncode == 0 and b2["kayitli"] and "0x" + b2["kaydeden"] == P["adres"], cli.stderr.splitlines()[0][:60] if cli.stderr else "")
    json.dump(dict(r["talep"], payload_hex="04" + H2), open(tp, "w"))
    cli = subprocess.run([f"{BIN}/belge-imzala", personel, tp], capture_output=True, text=True)
    kontrol("CLI: degistirilmis talep reddedilir", cli.returncode != 0 and not cli.stdout.strip())
    json.dump(r["talep"], open(tp, "w"))
    cli = subprocess.run([f"{BIN}/belge-imzala", yabanci, tp], capture_output=True, text=True)
    kontrol("CLI: baska personele hazirlanmis talep reddedilir", cli.returncode != 0)

    # ── 6) KUBRA sohbet araci + kanit izi ──
    H3 = blake3.blake3(b"ucuncu belge " + os.urandom(64)).hexdigest()
    s, r = jpost("/kubra-brain/v1/ask", {"prompt": f"Bu belgeyi zincire kaydetmek istiyorum: {H3}"})
    cevap = r.get("answer", "")
    kontrol("sohbet: arac = belge-kayit-hazirla", r.get("model") == "belge-kayit-hazirla", r.get("model"))
    kontrol("sohbet: kanit izi (arac, hash, zincir durumu)", "araç: belge-kayit-hazirla" in cevap and f"hash: {H3}" in cevap and "zincir durumu: kayıtlı değil" in cevap)
    kontrol("sohbet: KUBRA belgeyi KAYDETMEDI", jget(f"/rpc/belge/{H3}")["kayitli"] is False and "İMZALAMADIM" in cevap)
    kontrol("sohbet: KUBRA'nin kendi etkilesim kaniti belge hash'inden farkli (tuzlu)", r["proof_hash"] != H3 and r.get("salt"))
    s, r = jpost("/kubra-brain/v1/ask", {"prompt": f"şu belgeyi kaydet {HASH}"})
    kontrol("sohbet: kayitli belge icin 'zaten kayitli' + kaydeden", "zaten AIDAG-Chain'de kayıtlı" in r.get("answer", "") and P["adres"] in r.get("answer", ""))
    s, r = jpost("/kubra-brain/v1/ask", {"prompt": f"bu belge zincirde kayıtlı mı {HASH}"})
    kontrol("sohbet: dogrulama niyeti hala dogrulama araci", r.get("model") == "belge-dogrula" and "DOĞRULANDI" in r.get("answer", ""))

    # ── 6b) Eski sayfadan (standart disi ozetle) kaydedilmis >1 KB belge hala dogrulanir ──
    eb = os.path.join(T, "eski-belge.pdf"); open(eb, "wb").write(b"%PDF eski " + os.urandom(5000))
    ad = js("adaylar", {"dosya": eb}); STD, ESKI = ad[0]["hash"], ad[1]["hash"]
    s, r = jpost("/kubra-brain/v1/belge/hazirla", {"hash": ESKI, "imzalayan_pubkey": P["pubkey"]})
    im = js("imzala", {"anahtar": personel, "talep": r["talep"], "ts": int(time.time()), "beklenen": ESKI, "net": NET})
    istek("/rpc/submit", ham=im["hex"].encode())
    bulunan = next((a["yontem"] for a in ad if jget(f"/rpc/belge/{a['hash']}")["kayitli"]), None)
    kontrol("geriye uyum: eski ozetle kayitli belge sayfa adaylariyla bulunur (yontem=eski)", bulunan == "eski" and jget(f"/rpc/belge/{STD}")["kayitli"] is False)

    # ── 7) Yazici/cikti icin sunucu ucu YOK ──
    for yol in ("/kubra-brain/v1/print", "/kubra-brain/v1/yazdir", "/kubra-brain/v1/belge/pdf", "/rpc/print"):
        s, _ = istek(yol)
        if s not in (404, 405): break
    kontrol("sunucuda yazdirma/PDF ucu yok (404)", s in (404, 405), f"{yol} HTTP {s}")
    kontrol("sayfa: yazdirma = tarayicinin window.print()", "window.print()" in sayfa)
finally:
    subprocess.run(["nginx", "-c", f"{T}/nginx.conf", "-s", "stop"], capture_output=True)
    for p in sureclar: p.terminate()
    for p in sureclar:
        try: p.wait(10)
        except Exception: p.kill()
    shutil.rmtree(T, ignore_errors=True)
print(f"\nSONUC: {sum(sonuc)}/{len(sonuc)} gecti")
sys.exit(0 if all(sonuc) else 1)
