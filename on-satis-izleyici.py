#!/usr/bin/env python3
# ============================================================================
# on-satis-izleyici.py — OTOMATIK ON-SATIS ODEME IZLEYICI
#
# Kurucu cuzdanina (BSC) gelen USDT/BNB odemelerini izler; her YENI odeme icin
# ODEYENIN adresine otomatik TAHSIS (tip=10) kaydeder (owner imzalama araciyla).
# Odeyen adres = alicinin adresi = TGE'de claim adresi -> manuel eslestirme YOK.
#
# Faz-1 tavani (630.000 AIDAG) MOTORDA otomatik uygulanir; asilirsa tahsis reddedilir.
# Idempotent: islenmis tx hash'leri state dosyasinda tutulur (cifte-tahsis yok).
#
# Calistirma: periyodik (systemd timer / cron), ornegin her 2-3 dakikada.
#   NET=3474 NODE_RPC=https://aidag-chain.com/rpc python3 on-satis-izleyici.py
# ============================================================================
import json, subprocess, urllib.request, os, time

# --- Ayarlar (env ile override edilir) ---
KURUCU   = os.environ.get("KURUCU", "0x57241fb83E0Ee8624399A9Ad0f4ccf2B1dE4e716").lower()
NODE_RPC = os.environ.get("NODE_RPC", "http://127.0.0.1:8645")
NET      = os.environ.get("NET", "1")                 # 1=devnet, 3474=mainnet
KEY      = os.environ.get("KEY", "/root/aidag-lsc/aidag-kurucu.key")
BIN      = os.environ.get("BIN", "/root/aidag-lsc/target/release/on-satis-tahsis")
LSC_GIFT = int(os.environ.get("LSC_GIFT", "2"))       # her tahsiste LSC gaz hediyesi
STATE    = os.environ.get("STATE", "/root/aidag-lsc/.on-satis-islenmis.json")
# BSC zincirinden DOGRUDAN okuma (ANAHTARSIZ) — BscScan V1 kalkti, V2 anahtar
# istiyor; bunun yerine public BSC RPC + eth_getLogs/eth_getBlockByNumber.
BSC_RPCS = os.environ.get("BSC_RPCS",
    "https://bsc.publicnode.com,https://bsc-rpc.publicnode.com,https://bsc.blockrazor.xyz,"
    "https://bsc-mainnet.public.blastapi.io,https://bsc.rpc.blxrbdn.com,https://1rpc.io/bnb").split(",")
# ONEMLI: public RPC'ler tarayici gibi gorunmeyen istegi 403 ile reddeder -> User-Agent SART.
BSC_UA = ("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
          "(KHTML, like Gecko) Chrome/124.0 Safari/537.36")
BLOK_STATE = os.environ.get("BLOK_STATE", "/root/aidag-lsc/.on-satis-son-blok.json")
USDT_BSC = "0x55d398326f99059ff775485246999027b3197955"  # BSC USDT (18 ondalik)
# ERC-20 Transfer(address,address,uint256) event topic0
TRANSFER_TOPIC = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
# Ilk calismada kac blok geriye bakilsin (baslangic noktasi; ~son 1 saat).
ILK_GERI_BLOK = int(os.environ.get("ILK_GERI_BLOK", "20"))
# GUVENILIR YOL: Etherscan V2 (UCRETSIZ anahtar, chainid=56 BSC). Ayarliysa public
# RPC yerine bunu kullanir -> rate-limit derdi YOK, USDT + BNB tek cagriyla, guvenilir.
# Anahtar: etherscan.io/apis (ucretsiz, 2 dk). export ETHERSCAN_API_KEY=...
ETHERSCAN_KEY = os.environ.get("ETHERSCAN_API_KEY", "")
# Etherscan yolunda: bu unix-sn ONCESI odemeler ATLANIR (eski islemler tahsis olmasin).
BASLANGIC = int(os.environ.get("BASLANGIC", "1785024000"))  # 2026-07-26
# Faz-1 tier'lari: (kumulatif AIDAG siniri, USD fiyat). Frontend TIERS ile ayni.
TIERS = [(210000, 0.20), (420000, 0.25), (630000, 0.30)]

def http_json(url, data=None, headers=None):
    req = urllib.request.Request(url, data=data, headers=headers or {})
    with urllib.request.urlopen(req, timeout=12) as r:
        return json.load(r)

def islenmis_yukle():
    try:
        return set(json.load(open(STATE)))
    except Exception:
        return set()

def islenmis_kaydet(s):
    json.dump(sorted(s), open(STATE, "w"))

def satilan_aidag():
    d = http_json(f"{NODE_RPC}/on-satis-ozet")
    return int(d.get("toplam_satilan_aidag", "0")) / 1e18

def bnb_fiyat():
    try:
        d = http_json("https://api.binance.com/api/v3/ticker/price?symbol=BNBUSDT")
        return float(d["price"])
    except Exception:
        return 0.0

def usd_ile_aidag(usd, satilan):
    # Tier'lara gore: 'satilan' noktasindan basla, USD'yi AIDAG'a cevir.
    aidag = 0.0; nokta = satilan; kalan = usd
    for sinir, fiyat in TIERS:
        if nokta >= sinir:
            continue
        dilim = sinir - nokta
        maliyet = dilim * fiyat
        if kalan <= maliyet:
            return int(aidag + kalan / fiyat)
        aidag += dilim; kalan -= maliyet; nokta = sinir
    return int(aidag)  # tavan asilirsa motor keser

def bsc(method, params):
    """Public BSC RPC cagrisi (ANAHTARSIZ). RPC'leri sirayla dener; hiz-limitinde
    kisa bekleyip yeniden dener (public RPC'ler art arda cagriyi kisitlar)."""
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    son_hata = None
    for deneme in range(3):
        for rpc in BSC_RPCS:
            try:
                req = urllib.request.Request(rpc.strip(), data=body,
                    headers={"Content-Type": "application/json", "User-Agent": BSC_UA, "Accept": "application/json"})
                with urllib.request.urlopen(req, timeout=15) as r:
                    d = json.load(r)
                if "result" in d and d["result"] is not None:
                    return d["result"]
                son_hata = d.get("error")
            except Exception as e:
                son_hata = e
        time.sleep(1.5)  # hiz-limiti: bekle, sonra tekrar dene
    raise RuntimeError(f"BSC RPC hatasi: {son_hata}")

def son_blok_yukle():
    try:
        return int(json.load(open(BLOK_STATE)))
    except Exception:
        return None

def son_blok_kaydet(b):
    json.dump(b, open(BLOK_STATE, "w"))

def scan_ether(action, extra=""):
    """Etherscan V2 (chainid=56 BSC) — UCRETSIZ anahtarla guvenilir. txlist/tokentx."""
    url = (f"https://api.etherscan.io/v2/api?chainid=56&module=account&action={action}"
           f"&address={KURUCU}{extra}&startblock=0&endblock=99999999&sort=asc&apikey={ETHERSCAN_KEY}")
    r = http_json(url).get("result")
    return r if isinstance(r, list) else []

def gelen_odemeler_ether(islenmis):
    """Etherscan V2 ile: kurucu'ya gelen YENI USDT + BNB odemeleri (guvenilir yol)."""
    yeni = []
    def yeni_mi(tx):
        return (tx.get("to", "").lower() == KURUCU
                and tx.get("hash") not in islenmis
                and int(tx.get("timeStamp", "0")) >= BASLANGIC)  # eski odemeleri atla
    try:
        for tx in scan_ether("tokentx", f"&contractaddress={USDT_BSC}"):  # USDT
            if yeni_mi(tx):
                usd = int(tx.get("value", "0")) / 1e18
                if usd > 0:
                    yeni.append((tx["hash"], tx["from"], usd))
    except Exception as e:
        print("Etherscan USDT hatasi:", e)
    try:
        fiyat = bnb_fiyat()
        if fiyat > 0:
            for tx in scan_ether("txlist"):  # BNB
                if yeni_mi(tx) and int(tx.get("value", "0")) > 0 and tx.get("isError", "0") == "0":
                    yeni.append((tx["hash"], tx["from"], (int(tx["value"]) / 1e18) * fiyat))
    except Exception as e:
        print("Etherscan BNB hatasi:", e)
    return yeni

def gelen_odemeler(islenmis):
    """Kurucu'ya gelen YENI USDT + BNB odemeleri -> [(hash, odeyen, usd)].
    ANAHTARSIZ: BSC zincirinden DOGRUDAN — USDT=eth_getLogs, BNB=blok tarama.
    Blok bazli takip (BLOK_STATE): son islenen bloktan ileri; ilk calismada
    ILK_GERI_BLOK geriden. Her turda en fazla MAX_TUR blok (yavas ise yakalar)."""
    # GUVENILIR YOL: Etherscan anahtari varsa onu kullan (rate-limit derdi yok).
    if ETHERSCAN_KEY:
        return gelen_odemeler_ether(islenmis)
    MAX_TUR = int(os.environ.get("MAX_TUR_BLOK", "400"))
    ADIM = 400  # getLogs/tarama parca boyu
    yeni = []
    latest = int(bsc("eth_blockNumber", []), 16)
    son = son_blok_yukle()
    if son is None:
        son = max(0, latest - ILK_GERI_BLOK)
        son_blok_kaydet(son)
    if latest <= son:
        return []
    hedef = min(latest, son + MAX_TUR)             # tur basi ust sinir
    kurucu_topic = "0x" + "0" * 24 + KURUCU[2:]    # adres -> 32-bayt topic
    fiyat_bnb = bnb_fiyat()
    islenen = son
    start = son + 1
    while start <= hedef:
        end = min(start + ADIM - 1, hedef)
        # 1) USDT (Transfer event -> kurucu)
        try:
            logs = bsc("eth_getLogs", [{
                "fromBlock": hex(start), "toBlock": hex(end),
                "address": USDT_BSC, "topics": [TRANSFER_TOPIC, None, kurucu_topic]}]) or []
            for lg in logs:
                h = lg["transactionHash"]
                if h in islenmis:
                    continue
                odeyen = "0x" + lg["topics"][1][-40:]
                usd = int(lg["data"], 16) / 1e18
                if usd > 0:
                    yeni.append((h, odeyen, usd))
        except Exception as e:
            print("USDT getLogs hatasi:", e); break  # ilerleme kaydetme, sonraki tur tekrar
        # 2) BNB (native transfer -> bloklari tara). OPSIYONEL: BNB_TARA=1 ile ac.
        #    Native transfer log yaymadigi icin blok taramak gerekir (agir + rate-limit
        #    riski). Varsayilan KAPALI -> USDT (getLogs) guvenilir kalir. Acilirsa
        #    her getBlock arasi kisa gecikme ile rate-limit'ten kacinilir.
        if os.environ.get("BNB_TARA", "0") == "1" and fiyat_bnb > 0:
            try:
                for bn in range(start, end + 1):
                    blk = bsc("eth_getBlockByNumber", [hex(bn), True])
                    time.sleep(0.15)  # rate-limit'ten kacin
                    if not blk:
                        continue
                    for tx in blk.get("transactions", []):
                        if ((tx.get("to") or "").lower() == KURUCU
                                and int(tx.get("value", "0x0"), 16) > 0
                                and tx.get("hash") not in islenmis):
                            usd = (int(tx["value"], 16) / 1e18) * fiyat_bnb
                            yeni.append((tx["hash"], tx["from"], usd))
            except Exception as e:
                print("BNB blok tarama hatasi:", e); break
        islenen = end
        start = end + 1
    son_blok_kaydet(islenen)
    return yeni

ISLEM_UST = 50000  # ON_SATIS_ISLEM_UST_SINIR (AIDAG): tek tahsis bunu ASAMAZ
# Cuzdan basina KUMULATIF alim tavani (USD). Bir adres TOPLAMDA bunu asamaz —
# frontend'den BAGIMSIZ gercek koruma (dogrudan gonderim + coklu islem dahil).
CAP_USDT_ADRES = float(os.environ.get("ADRES_TAVAN_USD", "10000"))
ADRES_USD_STATE = os.environ.get("ADRES_USD_STATE", "/root/aidag-lsc/.on-satis-adres-usd.json")

def adres_usd_yukle():
    try:
        return json.load(open(ADRES_USD_STATE))
    except Exception:
        return {}

def adres_usd_kaydet(d):
    json.dump(d, open(ADRES_USD_STATE, "w"))

def ref_kayitli(ref):
    """Bu odeme_ref zincire kaydedilmis mi? (bulundu:true)"""
    try:
        return bool(http_json(f"{NODE_RPC}/on-satis/{ref}").get("bulundu"))
    except Exception:
        return False

def tahsis_kaydet(alici, aidag, ref):
    """Bir <=50k dilim tahsisi kaydet. IDEMPOTENT + DOGRULAMALI.
    Zaten kayitliysa True. Kaydolduysa True. submit ok YETERLI DEGIL —
    motor sinir/tavan nedeniyle sessizce reddedebilir; gercekten kaydoldu mu dogrula."""
    if ref_kayitli(ref):
        return True  # zaten var (cifte-tahsis / yeniden calisma guvenli)
    tips = ",".join(http_json(f"{NODE_RPC}/tips").get("tips", [])) or "-"
    now = str(int(time.time()))
    hexo = subprocess.check_output(
        [BIN, KEY, NET, alici, str(aidag), str(LSC_GIFT), str(ref), now, tips]
    ).decode().strip()
    http_json(f"{NODE_RPC}/submit",
              data=json.dumps({"hex": hexo}).encode(),
              headers={"Content-Type": "application/json"})
    time.sleep(0.6)  # ingest + state yeniden-uygulama icin kisa bekle
    return ref_kayitli(ref)  # KANIT: tahsis gercekten kaydoldu mu?

def main():
    islenmis = islenmis_yukle()
    yeni = gelen_odemeler(islenmis)
    if not yeni:
        print("yeni odeme yok"); return
    satilan = satilan_aidag()
    adres_usd = adres_usd_yukle()
    for h, odeyen, usd in yeni:
        addr = odeyen.lower()
        # CUZDAN BASINA kumulatif USD tavani — GERCEK koruma (frontend'den bagimsiz)
        kalan_hak = CAP_USDT_ADRES - float(adres_usd.get(addr, 0))
        if kalan_hak <= 0:
            print(f"TAVAN {addr}: zaten {CAP_USDT_ADRES:.0f} USD dolu -> bu odeme ({usd:.2f} USD) TAHSIS EDILMEDI, IADE gerekir tx={h[:12]}")
            islenmis.add(h); continue
        efektif_usd = min(usd, kalan_hak)
        if efektif_usd < usd:
            print(f"TAVAN {addr}: {usd:.2f} USD'nin {efektif_usd:.2f}'i tahsis edilir; {usd-efektif_usd:.2f} USD tavan asimi -> IADE gerekir tx={h[:12]}")
        aidag = usd_ile_aidag(efektif_usd, satilan)
        if aidag <= 0:
            islenmis.add(h); continue
        # >50k odemeyi <=50k DILIMLERE bol (motor tek tahsiste 50k'yi asamaz).
        # Her dilim benzersiz + DETERMINISTIK ref: base*100+idx (yeniden-calisma guvenli).
        base = int(h[2:16], 16)  # 56-bit; *100+idx u64'e sigar
        dilimler, kalan, idx = [], aidag, 0
        while kalan > 0 and idx < 100:
            d = min(kalan, ISLEM_UST); dilimler.append((base * 100 + idx, d)); kalan -= d; idx += 1
        hepsi_ok = True
        for ref, d in dilimler:
            try:
                ok = tahsis_kaydet(odeyen, d, ref)
            except Exception as e:
                print(f"HATA ({odeyen}, ref={ref}): {e}"); ok = False
            print(f"  {'OK ' if ok else 'RED'} {d} AIDAG -> {odeyen} (ref={ref})")
            if ok:
                satilan += d
            else:
                hepsi_ok = False
        if hepsi_ok:
            islenmis.add(h)  # SADECE tum dilimler DOGRULANIP kaydolunca isle
            adres_usd[addr] = float(adres_usd.get(addr, 0)) + efektif_usd  # kumulatif tavani guncelle
            adres_usd_kaydet(adres_usd)
            print(f"OK tx={h[:12]} ({efektif_usd:.2f} USD -> {aidag} AIDAG, {len(dilimler)} dilim, adres toplam={adres_usd[addr]:.2f}/{CAP_USDT_ADRES:.0f})")
        else:
            # Kismi/basarisiz ( or. Faz-1 tavani doldu): 'islendi' isaretleme -> sonraki
            # turda basarisiz dilimler tekrar denenir; basarililar ref ile atlanir.
            print(f"UYARI tx={h[:12]}: kismi/basarisiz — islenmedi. Tavan dolduysa fazlaya IADE gerekir.")
    islenmis_kaydet(islenmis)

if __name__ == "__main__":
    main()
