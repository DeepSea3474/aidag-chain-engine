#!/usr/bin/env python3
# ============================================================================
# on-satis-izleyici.py — OTOMATIK ON-SATIS ODEME IZLEYICI
#
# Kurucu cuzdanina (BSC) gelen USDT/BNB odemelerini izler; her DOGRULANMIS odeme
# icin ODEYENIN adresine otomatik TAHSIS (tip=10) kaydeder (owner imzalama araci).
# Odeyen adres = alicinin adresi = TGE'de claim adresi -> manuel eslestirme YOK.
#
# ASAMALAR OTOMATIK DEVAM EDER: Faz-1 (630.000) dolunca fiyat kendiliginden Faz-2
# (Rezerv) kademelerine gecer. Toplam tavan (1.680.000 AIDAG) MOTORDA uygulanir.
#
# GUVENLIK MODELI (denetim 2026-09-24):
#  1. KESIF != KANIT. eth_getLogs / blok tarama / Etherscan yalnizca ADAY tx hash
#     uretir. Tahsis icin her aday, EN AZ 2 BAGIMSIZ saglayicidan (farkli operator
#     alan adi) alinan eth_getTransactionReceipt ile dogrulanir; ikisi birebir
#     ayni sonucu vermeli (status=1, USDT kontrati, Transfer, to=kurucu, from,
#     value, blockHash). Uyusmazlik/eksik -> tahsis YOK, aday "bekleyen"de kalir.
#  2. ONAY DERINLIGI: yalniz (saglayicinin guncel blogu - ONAY_DERINLIGI) altina
#     kadar taranir; "latest" kullanilmaz.
#  3. KALICI KUYRUK: bulunan her aday once DURUM dosyasindaki "bekleyen"
#     kuyruguna yazilir; blok isaretcisi ayni ATOMIK yazimla ilerler. Her turda
#     bekleyenler yeniden denenir; basarili olan "islenmis"e tasinir.
#  4. SABIT PLAN: bir tx'in dilim plani (miktarlar, ref'ler, fiyat) ilk
#     hesaplamada kalici yazilir; yeniden denemede AYNI plan kullanilir. Tahsis
#     zincirden alici + miktar ile dogrulanir (GET /on-satis/:ref).
#  5. DURUM: tek dosya, tmp+os.replace ile atomik; fcntl.flock ile tek ornek;
#     bozuk durum dosyasinda SIFIRLAMA YOK -> dur.
#  6. TAM ARITMETIK: Fraction (float yok).
#  7. ASGARI ALIM: MIN_USD alti odemeler tahsis edilmez -> "iade_gerekli".
#
# Calistirma: periyodik (systemd timer / cron), ornegin her 2-3 dakikada.
#   NET=3474 NODE_RPC=https://aidag-chain.com/rpc python3 on-satis-izleyici.py
# ============================================================================
import json, subprocess, urllib.request, urllib.parse, os, sys, time, fcntl, tempfile, re
from fractions import Fraction
from decimal import Decimal, InvalidOperation

# --- Ayarlar (env ile override edilir) ---
# ODEME ADRESI: alicilarin USDT/BNB gonderdigi proje cuzdani. TEK KAYNAK:
# /var/www/aidag-chain/public/on-satis.html icindeki ODEME_ADRESI ile AYNI olmali.
KURUCU   = os.environ.get("KURUCU", "0x0ffe438e047dfb08c0c79aac9a63ea32d49a272c").lower()
NODE_RPC = os.environ.get("NODE_RPC", "http://127.0.0.1:8645")
NET      = os.environ.get("NET", "1")                 # 1=devnet, 3474=mainnet
KEY      = os.environ.get("KEY", "/root/aidag-lsc/aidag-kurucu.key")
BIN      = os.environ.get("BIN", "/root/aidag-lsc/target/release/on-satis-tahsis")
LSC_GIFT = int(os.environ.get("LSC_GIFT", "2"))       # her tahsiste LSC gaz hediyesi
# TEK durum dosyasi (bekleyen kuyruk + islenmis + adres USD + blok isaretcisi).
DURUM    = os.environ.get("DURUM", "/root/aidag-lsc/.on-satis-durum.json")
KILIT    = os.environ.get("KILIT", DURUM + ".kilit")
# ESKI (tasinacak) durum dosyalari: DURUM yoksa bir kez okunup DURUM'a tasinir.
STATE           = os.environ.get("STATE", "/root/aidag-lsc/.on-satis-islenmis.json")
BLOK_STATE      = os.environ.get("BLOK_STATE", "/root/aidag-lsc/.on-satis-son-blok.json")
ADRES_USD_STATE = os.environ.get("ADRES_USD_STATE", "/root/aidag-lsc/.on-satis-adres-usd.json")
# BSC zincirinden DOGRUDAN okuma (ANAHTARSIZ public RPC'ler).
BSC_RPCS = [u.strip() for u in os.environ.get("BSC_RPCS",
    "https://bsc.publicnode.com,https://bsc-rpc.publicnode.com,https://bsc.blockrazor.xyz,"
    "https://bsc-mainnet.public.blastapi.io,https://bsc.rpc.blxrbdn.com,https://1rpc.io/bnb").split(",") if u.strip()]
# ONEMLI: public RPC'ler tarayici gibi gorunmeyen istegi 403 ile reddeder -> User-Agent SART.
BSC_UA = ("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
          "(KHTML, like Gecko) Chrome/124.0 Safari/537.36")
USDT_BSC = "0x55d398326f99059ff775485246999027b3197955"  # BSC USDT (18 ondalik)
# ERC-20 Transfer(address,address,uint256) event topic0
TRANSFER_TOPIC = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
ONAY_DERINLIGI = int(os.environ.get("ONAY_DERINLIGI", "15"))   # blok
MIN_SAGLAYICI  = max(2, int(os.environ.get("MIN_SAGLAYICI", "2")))  # asla 2'nin altina inmez
ILK_GERI_BLOK  = int(os.environ.get("ILK_GERI_BLOK", "20"))
MAX_TUR        = int(os.environ.get("MAX_TUR_BLOK", "1200"))
# Parca boyu: public saglayicilarin eth_getLogs siniri (blockrazor 25 blok) ile uyumlu.
ADIM           = int(os.environ.get("ADIM_BLOK", "25"))
# Public saglayici hiz sinirlari (429 / "limit exceeded" / zaman asimi): istekler
# arasi tempo + gecici hatada geri cekilerek bir yeniden deneme.
RPC_TEMPO_SN   = float(os.environ.get("RPC_TEMPO_SN", "0.3"))
RPC_GECICI_BEKLE_SN = float(os.environ.get("RPC_GECICI_BEKLE_SN", "2"))
ETHERSCAN_KEY  = os.environ.get("ETHERSCAN_API_KEY", "")
BNB_TARA       = os.environ.get("BNB_TARA", "0") == "1"   # native BNB icin blok tarama (agir)
ETHERSCAN_SAYFA = 1000
BASLANGIC = int(os.environ.get("BASLANGIC", "1785024000"))  # 2026-07-26
# Asgari alim (USD) — site ile AYNI (on-satis sayfasi min 10 USDT).
MIN_USD = Fraction(os.environ.get("MIN_USD", "10"))
ISLEM_UST = 50000  # ON_SATIS_ISLEM_UST_SINIR (AIDAG): tek tahsis bunu ASAMAZ
MAX_PARENTS = 8    # lsc-engine dag::vertex::MAX_PARENTS
CAP_USDT_ADRES = Fraction(os.environ.get("ADRES_TAVAN_USD", "10000"))
ONDALIK = 10 ** 18
# Kademeler: (kumulatif AIDAG siniri, USD fiyat). Frontend TIERS ile AYNI olmali.
#   Faz-1        : 3 x 210k @ $0,20 / 0,25 / 0,30              (630.000)
#   Faz-2 Rezerv : 5 x 210k @ $0,35 / 0,40 / 0,45 / 0,50 / 0,55 (1.050.000)
TIERS = [(s, Fraction(f)) for s, f in [
    (210000, "0.20"), (420000, "0.25"), (630000, "0.30"),
    (840000, "0.35"), (1050000, "0.40"), (1260000, "0.45"), (1470000, "0.50"), (1680000, "0.55"),
]]

class DurumHatasi(Exception):
    """Durum dosyasi okunamadi/bozuk -> izleyici DURUR (sifirlama yok)."""

class UyusmazlikHatasi(Exception):
    """Zincirdeki tahsis kaydi plandakiyle uyusmuyor -> elle inceleme."""

# ---------------------------------------------------------------- yardimcilar
def http_json(url, data=None, headers=None):
    req = urllib.request.Request(url, data=data, headers=headers or {})
    with urllib.request.urlopen(req, timeout=12) as r:
        return json.load(r)

def frac_str(x):
    x = Fraction(x)
    return str(x.numerator) if x.denominator == 1 else f"{x.numerator}/{x.denominator}"

def frac_oku(s):
    return Fraction(str(s))

def usd_goster(x):
    return f"{float(x):.2f}"

ADRES_RE = re.compile(r"^0x[0-9a-f]{40}$")
HASH_RE = re.compile(r"^0x[0-9a-f]{64}$")

def topic_adres(t):
    t = (t or "").lower()
    if not re.fullmatch(r"0x[0-9a-f]{64}", t) or t[2:26] != "0" * 24:
        return None
    return "0x" + t[26:]

# ---------------------------------------------------------------- durum (atomik)
def atomik_yaz(yol, obj):
    d = os.path.dirname(os.path.abspath(yol))
    fd, tmp = tempfile.mkstemp(prefix=".tmp-", dir=d)
    try:
        with os.fdopen(fd, "w") as f:
            json.dump(obj, f, sort_keys=True)
            f.flush(); os.fsync(f.fileno())
        os.replace(tmp, yol)
    except BaseException:
        try: os.unlink(tmp)
        except OSError: pass
        raise
    try:
        dfd = os.open(d, os.O_RDONLY)
        try: os.fsync(dfd)
        finally: os.close(dfd)
    except OSError:
        pass

def _json_siki_oku(yol):
    """Dosya YOKSA None; varsa ama okunamiyor/bozuksa DurumHatasi."""
    if not os.path.exists(yol):
        return None
    try:
        with open(yol) as f:
            return json.load(f)
    except Exception as e:
        raise DurumHatasi(f"{yol} okunamadi/bozuk: {e}")

def bos_durum():
    return {"surum": 1, "son_blok": None, "bekleyen": {}, "islenmis": [],
            "adres_usd": {}, "iade_gerekli": [], "reddedilen": {}}

def durum_yukle():
    d = _json_siki_oku(DURUM)
    if d is None:
        # Eski dosyalardan TEK SEFERLIK tasima (bozuksa DUR).
        if not any(os.path.exists(y) for y in (STATE, ADRES_USD_STATE, BLOK_STATE)) \
                and os.environ.get("ILK_KURULUM") != "1":
            # Hic durum yok: kaybolmus durumla (or. git pull eski dosyalari sildi)
            # sifirdan baslamak cifte-tahsis / tavan sifirlanmasi demektir -> DUR.
            raise DurumHatasi(f"{DURUM} ve eski durum dosyalari YOK; gercekten ilk kurulumsa ILK_KURULUM=1 ile calistir")
        d = bos_durum()
        isl = _json_siki_oku(STATE)
        if isl is not None:
            if not isinstance(isl, list):
                raise DurumHatasi(f"{STATE} liste degil")
            d["islenmis"] = sorted(set(str(x).lower() for x in isl))
        au = _json_siki_oku(ADRES_USD_STATE)
        if au is not None:
            if not isinstance(au, dict):
                raise DurumHatasi(f"{ADRES_USD_STATE} sozluk degil")
            d["adres_usd"] = {k.lower(): frac_str(Fraction(str(v))) for k, v in au.items()}
        sb = _json_siki_oku(BLOK_STATE)
        if sb is not None:
            if not isinstance(sb, int):
                raise DurumHatasi(f"{BLOK_STATE} tamsayi degil")
            d["son_blok"] = sb
        return d
    if not isinstance(d, dict) or d.get("surum") != 1:
        raise DurumHatasi(f"{DURUM}: beklenmeyen bicim/surum")
    for k, tip in (("bekleyen", dict), ("islenmis", list), ("adres_usd", dict),
                   ("iade_gerekli", list), ("reddedilen", dict)):
        if not isinstance(d.get(k), tip):
            raise DurumHatasi(f"{DURUM}: '{k}' alani eksik/bozuk")
    if d.get("son_blok") is not None and not isinstance(d["son_blok"], int):
        raise DurumHatasi(f"{DURUM}: son_blok bozuk")
    return d

def durum_kaydet(d):
    atomik_yaz(DURUM, d)

def kilit_al():
    """Tek calisma kilidi. Baska ornek calisiyorsa None doner (cagiran hemen cikar)."""
    f = open(KILIT, "a+")
    try:
        fcntl.flock(f.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        f.close()
        return None
    return f

# ---------------------------------------------------------------- kademe (tam aritmetik)
def usd_ile_aidag(usd, satilan):
    """Kademelere gore: 'satilan' noktasindan basla, USD'yi TAM AIDAG'a cevir (asagi yuvarla)."""
    usd = Fraction(usd); nokta = Fraction(satilan)
    aidag = Fraction(0); kalan = usd
    for sinir, fiyat in TIERS:
        if nokta >= sinir:
            continue
        dilim = sinir - nokta
        maliyet = dilim * fiyat
        if kalan <= maliyet:
            return int((aidag + kalan / fiyat) // 1)
        aidag += dilim; kalan -= maliyet; nokta = Fraction(sinir)
    return int(aidag // 1)  # toplam tavan doldu

def aidag_maliyeti(aidag, satilan):
    """'satilan' noktasindan itibaren 'aidag' kadarinin kademeli USD maliyeti (Fraction)."""
    usd = Fraction(0); nokta = Fraction(satilan); kalan = Fraction(aidag)
    for sinir, fiyat in TIERS:
        if nokta < sinir and kalan > 0:
            al = min(kalan, sinir - nokta)
            usd += al * fiyat; kalan -= al; nokta += al
    return usd

# ---------------------------------------------------------------- BSC saglayicilari
def saglayici_kimligi(url):
    """Operator kimligi = kayitli alan adi (son iki etiket). bsc.publicnode.com ve
    bsc-rpc.publicnode.com AYNI operatordur -> bagimsiz SAYILMAZ."""
    host = (urllib.parse.urlparse(url).hostname or url).lower()
    parca = host.split(".")
    return ".".join(parca[-2:]) if len(parca) >= 2 else host

def bagimsiz_saglayicilar():
    """Her operatorden bir URL listesi (operator -> [url...])."""
    gruplar = {}
    for u in BSC_RPCS:
        gruplar.setdefault(saglayici_kimligi(u), []).append(u)
    return gruplar

def _gecici_mi(e):
    m = str(e).lower()
    return any(k in m for k in ("429", "too many", "limit exceeded", "rate", "timed out", "timeout", "temporarily"))

def rpc_cagir(url, method, params):
    """TEK saglayiciya JSON-RPC cagrisi; gecici hatada (hiz siniri/zaman asimi) bir kez
    geri cekilip yeniden dener. Kalici hata -> istisna. result None olabilir."""
    try:
        time.sleep(RPC_TEMPO_SN)
        return _rpc_cagir_ham(url, method, params)
    except Exception as e:
        if not _gecici_mi(e):
            raise
        time.sleep(RPC_GECICI_BEKLE_SN)
        return _rpc_cagir_ham(url, method, params)

def _rpc_cagir_ham(url, method, params):
    """TEK saglayiciya tek JSON-RPC cagrisi. Hata -> istisna."""
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(url, data=body,
        headers={"Content-Type": "application/json", "User-Agent": BSC_UA, "Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=15) as r:
        d = json.load(r)
    if "error" in d and d["error"]:
        raise RuntimeError(f"{url} {method}: {d['error']}")
    if "result" not in d:
        raise RuntimeError(f"{url} {method}: result yok")
    return d["result"]

def operator_cagir(urller, method, params):
    """Ayni operatorun URL'lerini sirayla dener (ayni operator = ayni kanit kaynagi)."""
    son = None
    for u in urller:
        try:
            return u, rpc_cagir(u, method, params)
        except Exception as e:
            son = e
    raise RuntimeError(f"operator yanit vermedi: {son}")

def canli_operatorler(gerekli=None):
    """eth_blockNumber'a cevap veren bagimsiz operatorler -> [(kimlik, url, guncel_blok)].
    Varsayilan: TUM cevap verenler (bazi saglayicilar getLogs/eski blok/receipt icin
    reddedebilir; cagiran basarili olanlardan en az MIN_SAGLAYICI tanesini kullanir).
    Her operator icin sonraki cagrilar AYNI url ile."""
    sonuc = []
    for kimlik, urller in bagimsiz_saglayicilar().items():
        try:
            url, bn = operator_cagir(urller, "eth_blockNumber", [])
            sonuc.append((kimlik, url, int(bn, 16)))
        except Exception as e:
            print(f"saglayici {kimlik} yanitsiz: {e}")
        if gerekli is not None and len(sonuc) >= gerekli:
            break
    return sonuc

# ---------------------------------------------------------------- kesif (aday uretimi)
def aralik_tara(url, start, end):
    """TEK saglayicidan [start,end] araligindaki ADAY odemeler -> {hash: tur}.
    Hata -> istisna (cagiran isaretciyi ilerletmez)."""
    adaylar = {}
    kurucu_topic = "0x" + "0" * 24 + KURUCU[2:]
    logs = rpc_cagir(url, "eth_getLogs", [{
        "fromBlock": hex(start), "toBlock": hex(end),
        "address": USDT_BSC, "topics": [TRANSFER_TOPIC, None, kurucu_topic]}])
    if logs is None:
        raise RuntimeError("eth_getLogs null")
    for lg in logs:
        if lg.get("removed"):
            continue
        h = (lg.get("transactionHash") or "").lower()
        if HASH_RE.match(h):
            adaylar[h] = "usdt"
    # BNB (native transfer log yaymaz -> blok tarama). OPSIYONEL: BNB_TARA=1.
    if BNB_TARA:
        for bn in range(start, end + 1):
            blk = rpc_cagir(url, "eth_getBlockByNumber", [hex(bn), True])
            time.sleep(0.15)
            if blk is None:
                raise RuntimeError(f"blok {bn} yok (saglayici geride)")
            for tx in blk.get("transactions", []):
                if ((tx.get("to") or "").lower() == KURUCU
                        and int(tx.get("value", "0x0"), 16) > 0):
                    h = (tx.get("hash") or "").lower()
                    if HASH_RE.match(h) and h not in adaylar:
                        adaylar[h] = "bnb"
    return adaylar

def scan_ether_sayfali(action, extra, startblock, endblock):
    """Etherscan V2 (chainid=56) — SAYFALI. Tum sayfalar alinamazsa istisna."""
    tum = []
    sayfa = 1
    while True:
        url = (f"https://api.etherscan.io/v2/api?chainid=56&module=account&action={action}"
               f"&address={KURUCU}{extra}&startblock={startblock}&endblock={endblock}"
               f"&page={sayfa}&offset={ETHERSCAN_SAYFA}&sort=asc&apikey={ETHERSCAN_KEY}")
        d = http_json(url)
        r = d.get("result")
        if not isinstance(r, list):
            if str(d.get("message", "")).lower().startswith("no transactions"):
                return tum
            raise RuntimeError(f"Etherscan {action}: {d.get('message')} {r}")
        tum.extend(r)
        if len(r) < ETHERSCAN_SAYFA:
            return tum
        sayfa += 1
        if sayfa * ETHERSCAN_SAYFA > 10000:
            raise RuntimeError("Etherscan sayfa siniri (10000) asildi; aralik daralt")

def kesif_ether(durum):
    """Etherscan yalniz ADAY uretir; tahsis yine 2-saglayici receipt dogrulamasina tabi."""
    d = http_json(f"https://api.etherscan.io/v2/api?chainid=56&module=proxy&action=eth_blockNumber&apikey={ETHERSCAN_KEY}")
    guncel = int(d["result"], 16)
    return guncel, lambda s, e: _ether_aralik(s, e)

def _ether_aralik(start, end):
    adaylar = {}
    for tx in scan_ether_sayfali("tokentx", f"&contractaddress={USDT_BSC}", start, end):
        if ((tx.get("to") or "").lower() == KURUCU and int(tx.get("timeStamp", "0")) >= BASLANGIC
                and (tx.get("contractAddress") or "").lower() == USDT_BSC):
            h = (tx.get("hash") or "").lower()
            if HASH_RE.match(h):
                adaylar[h] = "usdt"
    for tx in scan_ether_sayfali("txlist", "", start, end):
        if ((tx.get("to") or "").lower() == KURUCU and int(tx.get("timeStamp", "0")) >= BASLANGIC
                and int(tx.get("value", "0")) > 0):
            h = (tx.get("hash") or "").lower()
            if HASH_RE.match(h) and h not in adaylar:
                adaylar[h] = "bnb"
    return adaylar

def kesif(durum):
    """Onayli (guncel - ONAY_DERINLIGI) araligi tara; adaylari 'bekleyen'e yaz ve
    isaretciyi AYNI atomik yazimla ilerlet. Her tarama saglayicisi icin
    eth_blockNumber ile eth_getLogs AYNI url'den alinir. Varsayilan yol: 2 bagimsiz
    operatorun BIRLESIMI (biri odemeyi atlasa digeri yakalar); isaretci ancak
    hepsi basarili olursa ilerler."""
    if ETHERSCAN_KEY:
        try:
            guncel, tarayici = kesif_ether(durum)
        except Exception as e:
            print("Etherscan blok hatasi:", e); return
        tarayicilar = [("etherscan", guncel, tarayici)]
    else:
        ops = canli_operatorler()
        if len(ops) < MIN_SAGLAYICI:
            print(f"KESIF ATLANDI: {len(ops)} bagimsiz saglayici (en az {MIN_SAGLAYICI} gerekli)")
            return
        tarayicilar = [(k, bn, (lambda u: lambda s, e: aralik_tara(u, s, e))(u)) for k, u, bn in ops]
    guvenli_ust = min(bn for _, bn, _ in tarayicilar) - ONAY_DERINLIGI
    son = durum.get("son_blok")
    if son is None:
        durum["son_blok"] = max(0, guvenli_ust - ILK_GERI_BLOK)
        durum_kaydet(durum)
        son = durum["son_blok"]
    if guvenli_ust <= son:
        return
    hedef = min(guvenli_ust, son + MAX_TUR)
    islenmis = set(durum["islenmis"])
    start = son + 1
    while start <= hedef:
        # Etherscan tek kaynak (sayfalamali) -> buyuk parca; public RPC getLogs -> ADIM.
        end = min(start + (400 if ETHERSCAN_KEY else ADIM) - 1, hedef)
        # Parca icin operatorler sirayla denenir; EN AZ MIN_SAGLAYICI bagimsiz operator
        # BASARIYLA cevap vermeli (getLogs siniri/arsiv/hiz siniri olan atlanir). Adaylar
        # basarili operatorlerin BIRLESIMI (biri odemeyi atlasa digeri yakalar). Yeterli
        # basari yoksa isaretci ILERLEMEZ (odeme kaybi yok).
        adaylar = {}
        basarili, hatalar = 0, []
        # Etherscan modunda tek kesif kaynagi vardir (yalniz ADAY uretir; tahsis icin
        # 2 saglayicili receipt dogrulamasi yine zorunludur).
        gerekli = len(tarayicilar) if ETHERSCAN_KEY else MIN_SAGLAYICI
        # Yuk dagitimi: her parcada operator sirasi doner (ayni saglayici hep ilk olmaz).
        kaydir = ((start // max(ADIM, 1)) % len(tarayicilar)) if tarayicilar else 0
        for kimlik, _, tara in tarayicilar[kaydir:] + tarayicilar[:kaydir]:
            if basarili >= gerekli:
                break
            try:
                bulunan = tara(start, end)
            except Exception as e:
                hatalar.append(f"{kimlik}: {e}")
                continue
            basarili += 1
            adaylar.update({h: t for h, t in bulunan.items() if h not in adaylar})
        if basarili < gerekli:
            print(f"kesif hatasi [{start},{end}]: yalniz {basarili} saglayici basarili "
                  f"({'; '.join(hatalar)[:300]}) -> isaretci ilerlemez"); return
        yeni = 0
        for h, tur in adaylar.items():
            if h in islenmis or h in durum["bekleyen"] or h in durum["reddedilen"]:
                continue
            durum["bekleyen"][h] = {"tur": tur, "gorulme": int(time.time()), "deneme": 0}
            yeni += 1
        durum["son_blok"] = end           # kuyruk + isaretci TEK atomik yazim
        durum_kaydet(durum)
        if yeni:
            print(f"kesif [{start},{end}]: {yeni} aday kuyruga yazildi")
        start = end + 1

# ---------------------------------------------------------------- dogrulama (2 saglayici)
def _receipt_ozet(tur, receipt, tx):
    """Tek saglayicinin receipt(+tx)'inden karsilastirilabilir ozet. Gecersizse (neden) doner."""
    if receipt is None:
        return None, "receipt yok"
    ozet = {"blockHash": (receipt.get("blockHash") or "").lower(),
            "blockNumber": int(receipt.get("blockNumber") or "0x0", 16),
            "hash": (receipt.get("transactionHash") or "").lower()}
    if (receipt.get("status") or "").lower() != "0x1":
        return ("BASARISIZ", ozet["blockHash"], ozet["blockNumber"]), "status!=1"
    if tur == "usdt":
        eslesen = []
        for lg in receipt.get("logs", []):
            if lg.get("removed"):
                continue
            t = [x.lower() for x in (lg.get("topics") or [])]
            if ((lg.get("address") or "").lower() == USDT_BSC and len(t) == 3
                    and t[0] == TRANSFER_TOPIC and topic_adres(t[2]) == KURUCU):
                eslesen.append((topic_adres(t[1]), int(lg.get("data") or "0x0", 16)))
        if len(eslesen) != 1 or eslesen[0][0] is None:
            return ("GECERSIZ", ozet["blockHash"], ozet["blockNumber"]), f"kurucuya {len(eslesen)} gecerli USDT Transfer"
        ozet["from"], ozet["value"] = eslesen[0]
    else:
        if tx is None:
            return None, "tx yok"
        if ((tx.get("to") or "").lower() != KURUCU or (receipt.get("to") or "").lower() != KURUCU):
            return ("GECERSIZ", ozet["blockHash"], ozet["blockNumber"]), "to != kurucu"
        if (tx.get("hash") or "").lower() != ozet["hash"]:
            return None, "tx/receipt hash uyusmaz"
        ozet["from"] = (tx.get("from") or "").lower()
        ozet["value"] = int(tx.get("value") or "0x0", 16)
        if (receipt.get("from") or "").lower() != ozet["from"]:
            return None, "tx/receipt from uyusmaz"
    if ozet["value"] <= 0 or not ADRES_RE.match(ozet["from"] or ""):
        return ("GECERSIZ", ozet["blockHash"], ozet["blockNumber"]), "deger/adres gecersiz"
    return ozet, None

def odeme_dogrula(h, tur, ops):
    """(sonuc, neden). sonuc: dict (dogrulandi) | 'KESIN_GECERSIZ' | None (bekle)."""
    # Cevap veren TUM operatorler sorulur; hata/eksik cevap veren ATLANIR, ama BASARILI
    # cevaplar arasinda herhangi bir fark varsa odeme bekletilir (tek kotu saglayici
    # sahte receipt ile tahsis yaptiramaz; en az MIN_SAGLAYICI birebir ayni cevap sart).
    ozetler, atlanan = [], []
    for kimlik, url, guncel in ops:
        try:
            rc = rpc_cagir(url, "eth_getTransactionReceipt", [h])
            tx = rpc_cagir(url, "eth_getTransactionByHash", [h]) if tur == "bnb" else None
        except Exception as e:
            atlanan.append(f"{kimlik}: {e}")
            continue
        oz, neden = _receipt_ozet(tur, rc, tx)
        if oz is None:
            atlanan.append(f"{kimlik}: {neden}")
            continue
        bn = oz["blockNumber"] if isinstance(oz, dict) else oz[2]
        if bn <= 0 or bn > guncel - ONAY_DERINLIGI:
            return None, f"{kimlik}: onay derinligi yetersiz"   # gecersizlik de reorg'a acik
        if isinstance(oz, dict) and oz["hash"] != h:
            return None, f"{kimlik}: receipt hash farkli"
        ozetler.append((kimlik, oz, neden))
    if len(ozetler) < MIN_SAGLAYICI:
        return None, "yetersiz saglayici (" + "; ".join(atlanan)[:200] + ")"
    ilk = ozetler[0][1]
    if any(o != ilk for _, o, _ in ozetler[1:]):
        return None, "saglayicilar UYUSMUYOR"
    if not isinstance(ilk, dict):
        return "KESIN_GECERSIZ", ozetler[0][2]  # tum bagimsiz saglayicilar ayni gecersizligi soyluyor
    return ilk, None

# ---------------------------------------------------------------- dugum (AIDAG-Chain)
def satilan_aidag():
    d = http_json(f"{NODE_RPC}/on-satis-ozet")
    return Fraction(int(d.get("toplam_satilan_aidag", "0")), ONDALIK)

def bnb_fiyat():
    """BNB/USDT fiyati (str, Decimal). Alinamazsa None -> BNB odemesi bekler."""
    try:
        d = http_json("https://api.binance.com/api/v3/ticker/price?symbol=BNBUSDT")
        p = Decimal(str(d["price"]))
        return str(p) if p > 0 else None
    except (Exception, InvalidOperation):
        return None

def tahsis_sorgu(ref):
    """GET /on-satis/:ref -> None (kayit yok) | (alici_hex40, aidag_wei_str). Hata -> istisna."""
    k = http_json(f"{NODE_RPC}/on-satis/{ref}")
    if "bulundu" not in k:
        raise RuntimeError(f"sorgu hatasi: {k}")
    if not k.get("bulundu"):
        return None
    return (str(k.get("alici", "")).lower().removeprefix("0x"), str(k.get("aidag", "")))

def tahsis_eslesir_mi(ref, alici, aidag):
    """Zincirde ref var mi ve alici+miktar planla AYNI mi? Farkliysa UyusmazlikHatasi."""
    k = tahsis_sorgu(ref)
    if k is None:
        return False
    beklenen = (alici.lower().removeprefix("0x"), str(int(aidag) * ONDALIK))
    if k != beklenen:
        raise UyusmazlikHatasi(f"ref={ref} zincirde {k}, plan {beklenen}")
    return True

def tahsis_kaydet(alici, aidag, ref):
    """Bir <=50k dilim tahsisi. IDEMPOTENT + ZINCIRDEN DOGRULAMALI (alici+miktar)."""
    if tahsis_eslesir_mi(ref, alici, aidag):
        return True
    tips = http_json(f"{NODE_RPC}/tips").get("tips", [])[:MAX_PARENTS]
    tips_arg = ",".join(tips) or "-"
    now = str(int(time.time()))
    # ARAC IMZASI (9 arguman): <key> <net> <alici> <odeme_adresi> <aidag> <lsc> <ref> <ts> <tips>
    hexo = subprocess.check_output(
        [BIN, KEY, NET, alici, alici, str(aidag), str(LSC_GIFT), str(ref), now, tips_arg]
    ).decode().strip()
    http_json(f"{NODE_RPC}/submit",
              data=json.dumps({"hex": hexo}).encode(),
              headers={"Content-Type": "application/json"})
    time.sleep(0.6)
    return tahsis_eslesir_mi(ref, alici, aidag)

# ---------------------------------------------------------------- planlama + isleme
def _iade(durum, h, addr, usd, sebep):
    durum["iade_gerekli"].append({"tx": h, "adres": addr, "usd": frac_str(usd), "sebep": sebep,
                                  "zaman": int(time.time())})
    print(f"IADE GEREKLI tx={h} {addr} {usd_goster(usd)} USD: {sebep}")

def _islendi(durum, h):
    durum["bekleyen"].pop(h, None)
    if h not in durum["islenmis"]:
        durum["islenmis"].append(h)

def _bekleyen_ayrilmis_aidag(durum, haric):
    """Planlanmis ama zincire henuz yazilmamis dilimler (fiyat sirasi icin rezerv)."""
    t = 0
    for h, k in durum["bekleyen"].items():
        if h == haric or "plan" not in k:
            continue
        t += sum(d["aidag"] for d in k["plan"]["dilimler"] if not d.get("tamam"))
    return t

def plan_olustur(durum, h, oz, tur, satilan_zincir):
    """Ilk hesaplama: plan KALICI yazilir. None -> plan olusturulamadi (bekle).
    Doner: 'bitti' (iade/sifir) | 'plan'."""
    k = durum["bekleyen"][h]
    addr = oz["from"]
    fiyat = None
    if tur == "usdt":
        usd = Fraction(oz["value"], ONDALIK)
    else:
        fiyat = bnb_fiyat()
        if fiyat is None:
            return None
        usd = Fraction(oz["value"], ONDALIK) * Fraction(fiyat)
    if usd < MIN_USD:
        _iade(durum, h, addr, usd, f"asgari alim {frac_str(MIN_USD)} USD alti")
        _islendi(durum, h); return "bitti"
    onceki = frac_oku(durum["adres_usd"].get(addr, "0"))
    kalan_hak = CAP_USDT_ADRES - onceki
    if kalan_hak <= 0:
        _iade(durum, h, addr, usd, "cuzdan tavani dolu")
        _islendi(durum, h); return "bitti"
    efektif = min(usd, kalan_hak)
    satilan = satilan_zincir + _bekleyen_ayrilmis_aidag(durum, h)
    aidag = usd_ile_aidag(efektif, satilan)
    harcanan = aidag_maliyeti(aidag, satilan)
    if usd - efektif > 0:
        _iade(durum, h, addr, usd - efektif, "cuzdan tavani asimi (kismi)")
    if efektif - harcanan >= 1:
        _iade(durum, h, addr, efektif - harcanan, "toplam tavan (kismi)")
        efektif = harcanan
    if aidag <= 0:
        _islendi(durum, h); return "bitti"
    base = int(h[2:16], 16)  # 56-bit; *100+idx u64'e sigar
    dilimler, kalan, idx = [], aidag, 0
    while kalan > 0 and idx < 100:
        d = min(kalan, ISLEM_UST)
        dilimler.append({"ref": base * 100 + idx, "aidag": d, "tamam": False})
        kalan -= d; idx += 1
    k["plan"] = {"alici": addr, "tur": tur, "usd": frac_str(usd), "efektif_usd": frac_str(efektif),
                 "aidag": aidag, "satilan_baz": frac_str(satilan), "bnb_fiyat": fiyat,
                 "blockHash": oz["blockHash"], "dilimler": dilimler}
    # Cuzdan tavani PLAN aninda ayrilir (yeniden denemede cift sayim yok).
    durum["adres_usd"][addr] = frac_str(onceki + efektif)
    durum_kaydet(durum)   # plan, HERHANGI bir gonderimden ONCE kalici
    return "plan"

def plani_uygula(durum, h):
    k = durum["bekleyen"][h]; p = k["plan"]
    for d in p["dilimler"]:
        if d.get("tamam"):
            continue
        try:
            ok = tahsis_kaydet(p["alici"], d["aidag"], d["ref"])
        except UyusmazlikHatasi as e:
            k["inceleme"] = str(e); durum_kaydet(durum)
            print(f"INCELEME GEREKLI tx={h}: {e}"); return False
        except Exception as e:
            print(f"HATA tx={h} ref={d['ref']}: {e}"); ok = False
        print(f"  {'OK ' if ok else 'RED'} {d['aidag']} AIDAG -> {p['alici']} (ref={d['ref']})")
        if not ok:
            k["deneme"] = k.get("deneme", 0) + 1; durum_kaydet(durum)
            return False
        d["tamam"] = True; durum_kaydet(durum)
    _islendi(durum, h); durum_kaydet(durum)
    print(f"OK tx={h} ({usd_goster(frac_oku(p['efektif_usd']))} USD -> {p['aidag']} AIDAG, {len(p['dilimler'])} dilim)")
    return True

def bekleyenleri_isle(durum):
    if not durum["bekleyen"]:
        return
    ops = None
    satilan = None
    for h in sorted(durum["bekleyen"]):
        k = durum["bekleyen"].get(h)
        if k is None or k.get("inceleme"):
            continue
        if "plan" not in k:
            if ops is None:
                ops = canli_operatorler()
            if len(ops) < MIN_SAGLAYICI:
                print(f"DOGRULAMA ATLANDI: {len(ops)} bagimsiz saglayici"); continue
            oz, neden = odeme_dogrula(h, k["tur"], ops)
            if oz == "KESIN_GECERSIZ":
                durum["bekleyen"].pop(h); durum["reddedilen"][h] = neden; durum_kaydet(durum)
                print(f"REDDEDILDI tx={h}: {neden} (tum saglayicilar hemfikir)"); continue
            if oz is None:
                k["deneme"] = k.get("deneme", 0) + 1; k["son_neden"] = neden; durum_kaydet(durum)
                print(f"BEKLIYOR tx={h}: {neden}"); continue
            if satilan is None:
                try:
                    satilan = satilan_aidag()
                except Exception as e:
                    print("dugum yanitsiz (on-satis-ozet):", e); return
            if plan_olustur(durum, h, oz, k["tur"], satilan) != "plan":
                continue
        plani_uygula(durum, h)

def main():
    kilit = kilit_al()
    if kilit is None:
        print("baska bir izleyici ornegi calisiyor -> cikiliyor"); return 0
    try:
        try:
            durum = durum_yukle()
        except DurumHatasi as e:
            print(f"DUR: durum dosyasi bozuk/okunamadi -> TAHSIS YAPILMADI: {e}", file=sys.stderr)
            return 2
        kesif(durum)
        bekleyenleri_isle(durum)
        durum_kaydet(durum)
        if not durum["bekleyen"]:
            print("bekleyen odeme yok")
        return 0
    finally:
        kilit.close()

if __name__ == "__main__":
    sys.exit(main())
