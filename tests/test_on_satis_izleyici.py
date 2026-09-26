#!/usr/bin/env python3
"""on-satis-izleyici.py + yardimci betikler icin DENETIM testleri.

Her test bir saldiri/ariza senaryosunu kurar ve duzeltmeden SONRA saldirinin
basarisiz oldugunu (tahsis yok / odeme kaybolmaz / dogru miktar) gosterir.
TUM ag cagrilari (BSC RPC, Etherscan, Binance, AIDAG dugumu) ve imzalama
binary'si TAKLIT edilir; hicbir gercek servise istek gitmez, anahtar okunmaz.

Calistirma:  python3 -m unittest -v tests/test_on_satis_izleyici.py
"""
import importlib.util, json, os, sys, tempfile, types, unittest, subprocess, shutil, stat, fcntl
from fractions import Fraction

KOK = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
IZLEYICI = os.path.join(KOK, "on-satis-izleyici.py")

KURUCU = "0x0ffe438e047dfb08c0c79aac9a63ea32d49a272c"
USDT = "0x55d398326f99059ff775485246999027b3197955"
TRANSFER = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
A = "https://bsc.publicnode.com"          # operator: publicnode.com
A2 = "https://bsc-rpc.publicnode.com"     # AYNI operator (bagimsiz degil)
B = "https://bsc.blockrazor.xyz"          # operator: blockrazor.xyz
C = "https://1rpc.io/bnb"                 # operator: 1rpc.io
E18 = 10 ** 18
ALICI = "0x" + "11" * 20
ALICI2 = "0x" + "22" * 20


def pad(a):
    return "0x" + "0" * 24 + a[2:].lower()


def txh(i):
    # gercekci (rastgele gorunumlu) tx hash; ref = int(h[2:16]) * 100 + idx
    import hashlib
    return "0x" + hashlib.sha256(b"tx%d" % i).hexdigest()


def blok_hash(bn):
    return "0x" + ("%064x" % (0xb10c0000 + bn))


# ------------------------------------------------------------------ sahte BSC
class Saglayici:
    def __init__(self, head=1100):
        self.head = head
        self.logs = []        # getLogs'ta dondurulecek log'lar
        self.receipts = {}
        self.txs = {}
        self.blocks = {}
        self.kapali = False
        self.getlogs_cagri = []

    def cagir(self, method, params):
        if self.kapali:
            raise OSError("baglanti reddedildi")
        if method == "eth_blockNumber":
            return hex(self.head)
        if method == "eth_getLogs":
            f = params[0]
            s, e = int(f["fromBlock"], 16), int(f["toBlock"], 16)
            self.getlogs_cagri.append((s, e, self.head))
            # Geride kalan saglayici: head'in otesindeki bloklar icin sessizce [] doner
            return [l for l in self.logs if s <= int(l["blockNumber"], 16) <= min(e, self.head)]
        if method == "eth_getTransactionReceipt":
            return self.receipts.get(params[0])
        if method == "eth_getTransactionByHash":
            return self.txs.get(params[0])
        if method == "eth_getBlockByNumber":
            bn = int(params[0], 16)
            return self.blocks.get(bn, {"transactions": []}) if bn <= self.head else None
        raise RuntimeError("bilinmeyen metot " + method)

    def usdt_odeme(self, h, frm, deger_wei, bn, token=USDT, to=KURUCU, status="0x1",
                   removed=False, log_listede=True, receipt_var=True):
        lg = {"address": token, "topics": [TRANSFER, pad(frm), pad(to)], "data": hex(deger_wei),
              "transactionHash": h, "blockNumber": hex(bn), "removed": removed}
        if log_listede:
            self.logs.append(lg)
        if receipt_var:
            self.receipts[h] = {"status": status, "blockHash": blok_hash(bn), "blockNumber": hex(bn),
                                "transactionHash": h, "logs": [dict(lg, removed=False, topics=list(lg["topics"]))],
                                "from": frm, "to": token}

    def bnb_odeme(self, h, frm, deger_wei, bn, to=KURUCU, status="0x1"):
        tx = {"hash": h, "from": frm, "to": to, "value": hex(deger_wei), "blockNumber": hex(bn)}
        self.txs[h] = tx
        self.blocks.setdefault(bn, {"transactions": []})["transactions"].append(tx)
        self.receipts[h] = {"status": status, "blockHash": blok_hash(bn), "blockNumber": hex(bn),
                            "transactionHash": h, "logs": [], "from": frm, "to": to}


class Ag:
    def __init__(self, urller):
        self.s = {u: Saglayici() for u in urller}
        self.cagrilar = []

    def __getitem__(self, u):
        return self.s[u]

    def rpc_cagir(self, url, method, params):
        self.cagrilar.append((url, method))
        if url not in self.s:
            raise OSError("bilinmeyen saglayici")
        return self.s[url].cagir(method, params)

    def hepsine(self, fn, *a, **k):
        for s in self.s.values():
            getattr(s, fn)(*a, **k)


# ------------------------------------------------------------------ sahte AIDAG dugumu
class Dugum:
    def __init__(self):
        self.acik = True
        self.kayit = {}          # ref -> (alici_hex40, aidag_wei_str)
        self.tips = ["aa" * 32]
        self.submit_hata = set()  # n. submit cagrisinda hata
        self.submit_n = 0
        self.imza_args = []
        self.dis_satilan = 0     # baska yollardan satilan (AIDAG)
        self.bnb_fiyat = "600.00"
        self.etherscan = None     # fn(url)->json
        self.http_cagrilar = []

    def http_json(self, url, data=None, headers=None):
        self.http_cagrilar.append(url)
        if url.startswith("https://api.binance.com"):
            return {"price": self.bnb_fiyat}
        if url.startswith("https://api.etherscan.io"):
            return self.etherscan(url)
        if not self.acik:
            raise OSError("dugum kapali")
        yol = url.split("://", 1)[1].split("/", 1)[1]
        if yol == "on-satis-ozet":
            t = self.dis_satilan * E18 + sum(int(v[1]) for v in self.kayit.values())
            return {"toplam_satilan_aidag": str(t)}
        if yol.startswith("on-satis/"):
            ref = int(yol.split("/")[1])
            if ref in self.kayit:
                a, w = self.kayit[ref]
                return {"ok": True, "bulundu": True, "alici": a, "aidag": w, "lsc_hediye": "0", "zaman": 1}
            return {"ok": True, "odeme_ref": ref, "bulundu": False}
        if yol == "tips":
            return {"tips": list(self.tips)}
        if yol == "submit":
            self.submit_n += 1
            if self.submit_n in self.submit_hata:
                raise OSError("submit gecici hata")
            args = json.loads(bytes.fromhex(json.loads(data)["hex"]).decode())
            self.kayit[int(args[7])] = (args[3][2:].lower(), str(int(args[5]) * E18))
            return {"ok": True}
        raise RuntimeError("bilinmeyen yol " + url)

    def check_output(self, args, *a, **k):
        self.imza_args.append(list(args))
        return json.dumps(list(args)).encode().hex().encode()

    def toplam_aidag(self, alici=None):
        return sum(int(w) // E18 for a, w in self.kayit.values() if alici is None or "0x" + a == alici)


# ------------------------------------------------------------------ yukleyici
class Temel(unittest.TestCase):
    URLLER = [A, B]
    ENV_EK = {}

    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="izleyici-test-")
        self.durum_yolu = os.path.join(self.tmp, "durum.json")
        self.ag = Ag(self.URLLER)
        self.dugum = Dugum()
        self.m = self.yukle()

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def yukle(self, **env):
        ortam = {
            "DURUM": self.durum_yolu,
            "STATE": os.path.join(self.tmp, "eski-islenmis.json"),
            "BLOK_STATE": os.path.join(self.tmp, "eski-blok.json"),
            "ADRES_USD_STATE": os.path.join(self.tmp, "eski-usd.json"),
            "BSC_RPCS": ",".join(self.URLLER),
            "ILK_KURULUM": "1", "NODE_RPC": "http://dugum.test",
            "BIN": "/yok/on-satis-tahsis", "KEY": "/yok/anahtar", "NET": "1",
            "ETHERSCAN_API_KEY": "", "BNB_TARA": "0",
        }
        ortam.update(self.ENV_EK); ortam.update(env)
        eski = {k: os.environ.get(k) for k in ortam}
        os.environ.update(ortam)
        try:
            spec = importlib.util.spec_from_file_location("izleyici_test_%d" % id(self), IZLEYICI)
            m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
        finally:
            for k, v in eski.items():
                if v is None: os.environ.pop(k, None)
                else: os.environ[k] = v
        m.rpc_cagir = self.ag.rpc_cagir
        m.http_json = self.dugum.http_json
        m.subprocess = types.SimpleNamespace(check_output=self.dugum.check_output)
        import time as _t
        m.time = types.SimpleNamespace(time=_t.time, sleep=lambda x: None)
        return m

    def durum_baslat(self, son_blok=1000, **ek):
        d = self.m.bos_durum(); d["son_blok"] = son_blok; d.update(ek)
        json.dump(d, open(self.durum_yolu, "w"))

    def durum(self):
        return json.load(open(self.durum_yolu))

    def calistir(self):
        import contextlib, io
        with contextlib.redirect_stdout(io.StringIO()) as o, contextlib.redirect_stderr(io.StringIO()):
            kod = self.m.main()
        self.son_cikti = o.getvalue()
        return kod


# ================================================================== 1: 2-saglayici dogrulama
class T1_IkiSaglayiciDogrulama(Temel):
    def test_dogru_odeme_iki_saglayici_hemfikir_tahsis_edilir(self):
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(1), ALICI, 100 * E18, 1010)
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 500)  # 100 USD @0.20
        self.assertIn(txh(1), self.durum()["islenmis"])

    def test_tek_saglayici_sahte_log_ve_sahte_receipt_tahsis_yok(self):
        # SALDIRI: A (kotu niyetli) sahte log + sahte receipt uretir; B'de boyle tx yok.
        self.durum_baslat()
        self.ag[A].usdt_odeme(txh(2), ALICI, 10000 * E18, 1010)
        self.calistir(); self.calistir()
        self.assertEqual(self.dugum.kayit, {})
        self.assertEqual(self.dugum.imza_args, [])
        self.assertIn(txh(2), self.durum()["bekleyen"])  # bekleyen kuyrukta kalir

    def test_iki_saglayici_deger_uyusmaz_tahsis_yok(self):
        self.durum_baslat()
        self.ag[A].usdt_odeme(txh(3), ALICI, 10000 * E18, 1010)
        self.ag[B].usdt_odeme(txh(3), ALICI, 100 * E18, 1010)
        self.calistir()
        self.assertEqual(self.dugum.kayit, {})
        self.assertIn("UYUSMUYOR", self.durum()["bekleyen"][txh(3)]["son_neden"])

    def test_iki_saglayici_from_uyusmaz_tahsis_yok(self):
        self.durum_baslat()
        self.ag[A].usdt_odeme(txh(4), ALICI, 100 * E18, 1010)
        self.ag[B].usdt_odeme(txh(4), ALICI2, 100 * E18, 1010)
        self.calistir()
        self.assertEqual(self.dugum.kayit, {})

    def test_receipt_status_0_tahsis_yok(self):
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(5), ALICI, 1000 * E18, 1010, status="0x0")
        self.calistir()
        self.assertEqual(self.dugum.kayit, {})
        d = self.durum()
        self.assertIn(txh(5), d["reddedilen"]); self.assertNotIn(txh(5), d["bekleyen"])

    def test_sahte_token_kontrati_tahsis_yok(self):
        # SALDIRI: saldirgan kendi "USDT" token'ini kurucuya gonderir (Transfer event'i var).
        # Kesif filtresini atlatsa bile (sahte getLogs), receipt'te kontrat USDT degil.
        self.durum_baslat()
        sahte = "0x" + "de" * 20
        for s in (A, B):
            self.ag[s].usdt_odeme(txh(6), ALICI, 5000 * E18, 1010, token=sahte)
            self.ag[s].logs[-1]["address"] = USDT  # kesifte USDT gibi gorunsun
        self.calistir()
        self.assertEqual(self.dugum.kayit, {})
        self.assertIn(txh(6), self.durum()["reddedilen"])

    def test_alici_kurucu_degil_tahsis_yok(self):
        self.durum_baslat()
        for s in (A, B):
            self.ag[s].usdt_odeme(txh(7), ALICI, 5000 * E18, 1010, to="0x" + "33" * 20)
            self.ag[s].logs[-1]["topics"][2] = pad(KURUCU)
        self.calistir()
        self.assertEqual(self.dugum.kayit, {})

    def test_removed_log_atlanir(self):
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(8), ALICI, 100 * E18, 1010, removed=True, receipt_var=False)
        self.calistir()
        d = self.durum()
        self.assertNotIn(txh(8), d["bekleyen"]); self.assertEqual(self.dugum.kayit, {})

    def test_bnb_iki_saglayici_dogrulanir(self):
        self.m = self.yukle(BNB_TARA="1"); self.durum_baslat()
        self.ag.hepsine("bnb_odeme", txh(9), ALICI, E18 // 10, 1010)  # 0.1 BNB * 600 = 60 USD
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 300)

    def test_bnb_deger_uyusmaz_veya_status0_tahsis_yok(self):
        self.m = self.yukle(BNB_TARA="1"); self.durum_baslat()
        self.ag[A].bnb_odeme(txh(10), ALICI, 10 * E18, 1010)
        self.ag[B].bnb_odeme(txh(10), ALICI, E18 // 10, 1010)
        self.ag.hepsine("bnb_odeme", txh(11), ALICI, E18, 1011, status="0x0")
        self.calistir()
        self.assertEqual(self.dugum.kayit, {})

    def test_birden_fazla_transfer_log_belirsiz_tahsis_yok(self):
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(12), ALICI, 100 * E18, 1010)
        for s in (A, B):
            r = self.ag[s].receipts[txh(12)]
            r["logs"].append(dict(r["logs"][0], topics=[TRANSFER, pad(ALICI2), pad(KURUCU)]))
        self.calistir()
        self.assertEqual(self.dugum.kayit, {})


class T1c_EnvIleZayiflatilamaz(Temel):
    def test_min_saglayici_1_verilse_de_2_bagimsiz_saglayici_sart(self):
        self.m = self.yukle(MIN_SAGLAYICI="1")
        self.assertEqual(self.m.MIN_SAGLAYICI, 2)
        self.durum_baslat()
        self.ag[A].usdt_odeme(txh(2), ALICI, 10000 * E18, 1010)
        self.calistir()
        self.assertEqual(self.dugum.kayit, {})


class T1b_AyniOperator(Temel):
    URLLER = [A, A2]   # ikisi de publicnode.com -> TEK operator

    def test_ayni_operatorun_iki_url_si_bagimsiz_sayilmaz(self):
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(20), ALICI, 10000 * E18, 1010)
        self.calistir()
        self.assertEqual(self.dugum.kayit, {})
        self.assertEqual(self.durum()["son_blok"], 1000)  # kesif bile yapilmadi


# ================================================================== 2: onay derinligi
class T2_OnayDerinligi(Temel):
    def test_getlogs_yalniz_head_eksi_15_altina_kadar(self):
        self.durum_baslat()
        self.calistir()
        for s in (A, B):
            for (_, e, head) in self.ag[s].getlogs_cagri:
                self.assertLessEqual(e, head - 15)
        self.assertEqual(self.durum()["son_blok"], 1100 - 15)

    def test_onaysiz_blokta_odeme_simdilik_islenmez_sonra_islenir(self):
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(21), ALICI, 100 * E18, 1095)  # head 1100 -> 5 onay
        self.calistir()
        self.assertEqual(self.dugum.kayit, {})
        self.assertNotIn(txh(21), self.durum()["bekleyen"])
        for s in (A, B): self.ag[s].head = 1120
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 500)

    def test_receipt_blogu_derin_degilse_bekler(self):
        # Kesif (eski onayli aralik) aday verdi ama receipt'e gore tx yeni bloga
        # tasinmis (reorg) -> onay yetersiz -> bekle.
        self.durum_baslat(bekleyen={txh(22): {"tur": "usdt", "gorulme": 0, "deneme": 0}})
        self.ag.hepsine("usdt_odeme", txh(22), ALICI, 100 * E18, 1099, log_listede=False)
        self.calistir()
        self.assertEqual(self.dugum.kayit, {})
        self.assertIn("onay", self.durum()["bekleyen"][txh(22)]["son_neden"])


# ================================================================== 3: kalici kuyruk
class T3_KaliciKuyruk(Temel):
    def test_dugum_kapali_odeme_kuyrukta_kalir_sonraki_turda_islenir(self):
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(30), ALICI, 100 * E18, 1010)
        self.dugum.acik = False
        self.calistir()
        d = self.durum()
        self.assertEqual(d["son_blok"], 1085)          # isaretci ilerledi...
        self.assertIn(txh(30), d["bekleyen"])           # ...ama odeme kuyrukta
        self.assertEqual(self.dugum.kayit, {})
        self.dugum.acik = True
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 500)
        d = self.durum()
        self.assertNotIn(txh(30), d["bekleyen"]); self.assertIn(txh(30), d["islenmis"])
        self.calistir()                                  # tekrar: cifte tahsis yok
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 500)

    def test_geride_kalan_saglayici_odemeyi_kaybettirmez(self):
        # B geride (head 1040): eski kodda 'latest' baska RPC'den gelip getLogs
        # geride kalan RPC'ye gidince [] donuyor ve isaretci ilerliyordu.
        self.durum_baslat()
        self.ag[B].head = 1040
        self.ag.hepsine("usdt_odeme", txh(31), ALICI, 100 * E18, 1060)
        self.calistir()
        self.assertEqual(self.durum()["son_blok"], 1040 - 15)   # min(head) - 15
        self.assertEqual(self.dugum.kayit, {})
        self.ag[B].head = 1200
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 500)

    def test_getlogs_hatasi_isaretciyi_ilerletmez(self):
        self.durum_baslat()
        orj = self.ag[B].cagir
        def bozuk(method, params):
            if method == "eth_getLogs": raise OSError("zaman asimi")
            return orj(method, params)
        self.ag[B].cagir = bozuk
        self.calistir()
        self.assertEqual(self.durum()["son_blok"], 1000)

    def test_bir_saglayici_logu_atlasa_da_birlesim_yakalar(self):
        self.durum_baslat()
        self.ag[A].usdt_odeme(txh(32), ALICI, 100 * E18, 1010, log_listede=False)  # A getLogs'ta vermiyor
        self.ag[B].usdt_odeme(txh(32), ALICI, 100 * E18, 1010)
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 500)


# ================================================================== 4: plan korunur + zincir dogrulama
class T4_Plan(Temel):
    def test_kismi_hata_plan_korunur_toplam_dogru(self):
        self.durum_baslat()
        self.m.CAP_USDT_ADRES = Fraction(10 ** 9)
        self.ag.hepsine("usdt_odeme", txh(40), ALICI, 30000 * E18, 1010)  # 150000 AIDAG, 3 dilim
        self.dugum.submit_hata = {2}
        self.calistir()
        plan1 = self.durum()["bekleyen"][txh(40)]["plan"]
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 50000)
        # Arada baska satis oldu -> satilan degisti; eski kod plani yeniden hesaplardi.
        self.dugum.dis_satilan = 200000
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 150000)   # 30000 USD @0.20
        refs = sorted(self.dugum.kayit)
        self.assertEqual(refs, sorted(d["ref"] for d in plan1["dilimler"]))
        self.assertEqual([d["aidag"] for d in plan1["dilimler"]], [50000, 50000, 50000])
        self.assertEqual(self.durum()["adres_usd"][ALICI], "30000")  # cift sayim yok

    def test_zincirde_ref_farkli_alici_ile_varsa_inceleme_submit_yok(self):
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(41), ALICI, 100 * E18, 1010)
        ref = int(txh(41)[2:16], 16) * 100
        self.dugum.kayit[ref] = ("99" * 20, str(500 * E18))
        self.calistir()
        self.assertEqual(self.dugum.imza_args, [])
        self.assertIn("inceleme", self.durum()["bekleyen"][txh(41)])

    def test_zincirde_ref_farkli_miktarla_varsa_basari_sayilmaz(self):
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(42), ALICI, 100 * E18, 1010)
        ref = int(txh(42)[2:16], 16) * 100
        self.dugum.kayit[ref] = (ALICI[2:], str(1 * E18))
        self.calistir()
        self.assertNotIn(txh(42), self.durum()["islenmis"])

    def test_imza_araci_9_arguman_ve_tips_8e_kirpilir(self):
        self.durum_baslat()
        self.dugum.tips = ["%064x" % i for i in range(12)]
        self.ag.hepsine("usdt_odeme", txh(43), ALICI, 100 * E18, 1010)
        self.calistir()
        a = self.dugum.imza_args[0]
        self.assertEqual(len(a) - 1, 9)
        self.assertEqual(len(a[9].split(",")), 8)


# ================================================================== 5: durum dosyasi
class T5_Durum(Temel):
    def test_bozuk_durum_dosyasi_dur_tahsis_yok(self):
        open(self.durum_yolu, "w").write('{"surum": 1, "bekleyen": {')  # yarim yazilmis
        self.ag.hepsine("usdt_odeme", txh(50), ALICI, 100 * E18, 1010)
        self.assertEqual(self.calistir(), 2)
        self.assertEqual(self.dugum.kayit, {}); self.assertEqual(self.ag.cagrilar, [])
        self.assertEqual(open(self.durum_yolu).read(), '{"surum": 1, "bekleyen": {')  # SIFIRLANMADI

    def test_bozuk_eski_islenmis_dosyasi_dur(self):
        open(os.path.join(self.tmp, "eski-islenmis.json"), "w").write("[\"0xab")
        self.assertEqual(self.calistir(), 2)
        self.assertFalse(os.path.exists(self.durum_yolu))

    def test_durum_hic_yoksa_ilk_kurulum_onayi_olmadan_dur(self):
        self.m = self.yukle(ILK_KURULUM="0")
        self.assertEqual(self.calistir(), 2)
        self.assertEqual(self.ag.cagrilar, [])

    def test_eski_dosyalardan_tasima(self):
        json.dump([txh(51)], open(os.path.join(self.tmp, "eski-islenmis.json"), "w"))
        json.dump({ALICI: 10000.0}, open(os.path.join(self.tmp, "eski-usd.json"), "w"))
        json.dump(1000, open(os.path.join(self.tmp, "eski-blok.json"), "w"))
        self.m = self.yukle(ILK_KURULUM="0")
        self.ag.hepsine("usdt_odeme", txh(51), ALICI, 100 * E18, 1010)   # zaten islenmis -> atlanir
        self.ag.hepsine("usdt_odeme", txh(52), ALICI, 100 * E18, 1011)   # adres tavani dolu -> iade
        self.assertEqual(self.calistir(), 0)
        self.assertEqual(self.dugum.kayit, {})
        d = self.durum()
        self.assertEqual(d["son_blok"], 1085)
        self.assertEqual([i["tx"] for i in d["iade_gerekli"]], [txh(52)])

    def test_atomik_yazim_cokmede_eski_dosya_saglam(self):
        self.durum_baslat(son_blok=777)
        once = open(self.durum_yolu).read()
        orj = self.m.json.dump
        def cok(obj, f, **k):
            f.write('{"surum": 1, "yar'); raise OSError("disk dolu / SIGKILL")
        self.m.json = types.SimpleNamespace(dump=cok, load=json.load)
        with self.assertRaises(OSError):
            self.m.durum_kaydet({"surum": 1})
        self.assertEqual(open(self.durum_yolu).read(), once)
        self.assertEqual([f for f in os.listdir(self.tmp) if f.startswith(".tmp-")], [])

    def test_ikinci_ornek_kilitte_hemen_cikar(self):
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(53), ALICI, 100 * E18, 1010)
        # Baska bir SUREC kilidi tutuyor
        p = subprocess.Popen([sys.executable, "-c",
            "import fcntl,sys,time;f=open(sys.argv[1],'a+');fcntl.flock(f,fcntl.LOCK_EX);print('ok',flush=True);time.sleep(30)",
            self.durum_yolu + ".kilit"], stdout=subprocess.PIPE)
        try:
            self.assertEqual(p.stdout.readline().strip(), b"ok")
            self.assertEqual(self.calistir(), 0)
            self.assertEqual(self.ag.cagrilar, []); self.assertEqual(self.dugum.kayit, {})
        finally:
            p.kill(); p.wait()
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 500)


# ================================================================== 6: tam aritmetik
class T6_TamAritmetik(Temel):
    def test_55_usd_055_fiyatta_100_aidag(self):
        self.assertEqual(self.m.usd_ile_aidag(Fraction(55), 1470000), 100)
        self.assertEqual(int(55 / 0.55), 99)  # eski float hatasi (kanit)

    def test_10_10000_tam_usd_ve_kurus_birebir(self):
        f = self.m.usd_ile_aidag
        # Referans: Faz-1 kademe-1 (satilan=0, fiyat 0.20): aidag = floor(kurus*5/100)
        # ve Faz-2 son kademe (satilan=1470000, fiyat 0.55): floor(kurus/55)
        for usd in range(10, 10001):
            self.assertEqual(f(Fraction(usd), 0), usd * 5)
            self.assertEqual(f(Fraction(usd), 1470000), (usd * 100) // 55)
        for kurus in range(1000, 1000001):
            u = Fraction(kurus, 100)
            self.assertEqual(f(u, 0), (kurus * 5) // 100, kurus)
            self.assertEqual(f(u, 1470000), kurus // 55, kurus)

    def test_kademe_gecisi_fraction_referansi(self):
        # Referans: kademeleri AIDAG AIDAG (birim adim) dolduran bagimsiz tam hesap.
        def ref(usd, satilan):
            usd = Fraction(usd); n = 0
            fiyatlar = [(s, Fraction(p)) for s, p in [(210000, "0.20"), (420000, "0.25"), (630000, "0.30"),
                (840000, "0.35"), (1050000, "0.40"), (1260000, "0.45"), (1470000, "0.50"), (1680000, "0.55")]]
            nokta = satilan
            while True:
                fy = next((p for s, p in fiyatlar if nokta < s), None)
                if fy is None or usd < fy: return n
                usd -= fy; n += 1; nokta += 1
        for satilan in (209990, 419950, 629999, 1679990):
            for kurus in range(1000, 1000001, 997):
                self.assertEqual(self.m.usd_ile_aidag(Fraction(kurus, 100), satilan),
                                 ref(Fraction(kurus, 100), satilan), (satilan, kurus))
        self.assertEqual(self.m.aidag_maliyeti(20, 209990), Fraction(10 * 20 + 10 * 25, 100))


# ================================================================== 7: asgari alim
class T7_MinUsd(Temel):
    """KARARLAR K-20: minimum 10 USDT. Altina tahsis yazilmaz; ayri 'minimum_alti' kaydi
    + gunluk satiri + (varsa) bildirim komutu; iade icin."""

    def test_min_usd_alti_tahsis_yok_minimum_alti_kaydi(self):
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(70), ALICI, 999 * E18 // 100, 1010)   # 9.99 USD
        self.ag.hepsine("usdt_odeme", txh(71), ALICI2, 10 * E18, 1011)          # tam 10 USD
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 0, "minimum alti: tahsis yok")
        self.assertEqual(self.dugum.toplam_aidag(ALICI2), 50, "tam 10 USD: tahsis var")
        d = self.durum()
        self.assertEqual([(k["tx"], k["adres"], k["usd"], k["tur"]) for k in d["minimum_alti"]],
                         [(txh(70), ALICI, "999/100", "usdt")])
        self.assertEqual(d["minimum_alti"][0]["blok"], 1010)
        self.assertEqual(d["iade_gerekli"], [], "minimum alti ayri kayitta, genel iade listesinde degil")
        self.assertIn(txh(70), d["islenmis"], "tekrar islenmez")
        self.assertIn("MINIMUM ALTI ODEME (iade gerekli): tx=" + txh(70), self.son_cikti)
        # Ikinci tur: ayni tx tekrar kaydedilmez / tahsis edilmez.
        self.calistir()
        self.assertEqual(len(self.durum()["minimum_alti"]), 1)
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 0)

    def _bildirimli(self, komut):
        self.m = self.yukle(BILDIRIM_KOMUTU=komut)
        self.m.subprocess = types.SimpleNamespace(check_output=self.dugum.check_output,
                                                  run=subprocess.run, DEVNULL=subprocess.DEVNULL)

    def test_bildirim_komutu_kaydi_json_olarak_alir(self):
        cikti = os.path.join(self.tmp, "bildirim.json")
        self._bildirimli("cat > " + cikti)
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(72), ALICI, 5 * E18, 1010)             # 5 USD
        self.calistir()
        k = json.load(open(cikti))
        self.assertEqual((k["tx"], k["adres"], k["usd"], k["minimum_usd"]), (txh(72), ALICI, "5", "10"))

    def test_bildirim_komutu_hata_verse_de_akis_bozulmaz(self):
        self._bildirimli("exit 3")
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(73), ALICI, 3 * E18, 1010)             # 3 USD
        self.ag.hepsine("usdt_odeme", txh(74), ALICI2, 20 * E18, 1011)           # 20 USD
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI2), 100, "gecerli odeme yine tahsis edilir")
        self.assertEqual([k["tx"] for k in self.durum()["minimum_alti"]], [txh(73)])
        self.assertIn("minimum alti bildirimi gonderilemedi", self.son_cikti)

    def test_minimum_alti_alani_olmayan_eski_durum_okunur(self):
        d = self.m.bos_durum(); d["son_blok"] = 1000; d.pop("minimum_alti")
        json.dump(d, open(self.durum_yolu, "w"))
        self.ag.hepsine("usdt_odeme", txh(75), ALICI, 2 * E18, 1010)
        self.assertEqual(self.calistir(), 0)
        self.assertEqual([k["tx"] for k in self.durum()["minimum_alti"]], [txh(75)])

    def test_minimum_alti_listesi_salt_okunur(self):
        self.durum_baslat(minimum_alti=[{"tx": txh(76), "adres": ALICI, "usd": "999/100",
                                         "tur": "usdt", "blok": 1010, "zaman": 1}])
        once = open(self.durum_yolu).read()
        self.ag.hepsine("usdt_odeme", txh(77), ALICI2, 20 * E18, 1011)   # listeleme tahsis YAPMAMALI
        import contextlib, io
        eski_argv = sys.argv; sys.argv = ["on-satis-izleyici.py", "--minimum-alti"]
        try:
            with contextlib.redirect_stdout(io.StringIO()) as o:
                kod = self.m.main()
        finally:
            sys.argv = eski_argv
        self.assertEqual(kod, 0)
        self.assertIn("tx=" + txh(76), o.getvalue())
        self.assertIn("9.99 USD", o.getvalue())
        self.assertEqual(open(self.durum_yolu).read(), once, "durum dosyasi degismedi")
        self.assertEqual(self.dugum.toplam_aidag(), 0, "listeleme tahsis yapmaz")


# ================================================================== 9: Etherscan yolu
class T9_Etherscan(Temel):
    ENV_EK = {"ETHERSCAN_API_KEY": "TESTANAHTAR"}

    def _etherscan(self, tokentx):
        def f(url):
            if "action=eth_blockNumber" in url:
                return {"result": hex(1100)}
            sayfa = int(url.split("&page=")[1].split("&")[0])
            if "action=tokentx" in url:
                r = tokentx[(sayfa - 1) * 1000: sayfa * 1000]
            else:
                r = []
            self.etherscan_urller.append(url)
            return {"status": "1", "message": "OK", "result": r} if r else {"status": "0", "message": "No transactions found", "result": []}
        return f

    def test_sayfalama_ve_etherscan_sahte_kaydi_dogrulanmadan_tahsis_yok(self):
        self.etherscan_urller = []
        self.durum_baslat()
        kayitlar = [{"hash": txh(1000 + i), "from": ALICI, "to": KURUCU, "value": str(100 * E18),
                     "timeStamp": "1790000000", "contractAddress": USDT} for i in range(1005)]
        self.dugum.etherscan = self._etherscan(kayitlar)
        # Yalniz SONUNCU (2. sayfadaki) zincirde gercek; digerleri Etherscan'in (sahte) iddiasi.
        self.ag.hepsine("usdt_odeme", txh(1000 + 1004), ALICI, 100 * E18, 1010, log_listede=False)
        self.calistir()
        self.assertTrue(any("&page=2&" in u for u in self.etherscan_urller))
        self.assertTrue(all("startblock=1001&endblock=1085" in u for u in self.etherscan_urller))
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 500)       # yalniz gercek olan
        self.assertEqual(len(self.durum()["bekleyen"]), 1004)       # digerleri dogrulanmadi


# ================================================================== 14: canli saglayici sinirlari
# Canli kurulumda gorulen: publicnode eski bloklar icin getLogs'u 403 ile reddediyor,
# blockrazor 25 bloktan buyuk araligi reddediyor. Kesif/dogrulama CALISMAYAN saglayiciyi
# atlamali ama guvenlik sarti (en az 2 bagimsiz BASARILI + birebir ayni) korunmali.
class T14_SaglayiciSinirlari(Temel):
    URLLER = [A, B, C]

    def _getlogs_reddet(self, url):
        orj = self.ag[url].cagir
        def f(method, params):
            if method == "eth_getLogs":
                raise OSError("HTTP Error 403: Forbidden (archive)")
            return orj(method, params)
        self.ag[url].cagir = f

    def test_ilk_saglayici_getlogs_reddetse_de_kesif_ilerler_ve_tahsis_olur(self):
        self.durum_baslat()
        self._getlogs_reddet(A)
        self.ag.hepsine("usdt_odeme", txh(1), ALICI, 100 * E18, 1010)
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 500)
        self.assertGreater(self.durum()["son_blok"], 1000)
        # parcalar ADIM (25) blogu asmaz
        self.assertTrue(all(e - s + 1 <= 25 for s, e, _ in self.ag[B].getlogs_cagri))

    def test_yalniz_bir_saglayici_basariliysa_isaretci_ilerlemez(self):
        self.durum_baslat()
        self._getlogs_reddet(A); self._getlogs_reddet(B)
        self.ag.hepsine("usdt_odeme", txh(2), ALICI, 100 * E18, 1010)
        self.calistir()
        self.assertEqual(self.durum()["son_blok"], 1000, "tek saglayiciyla isaretci ilerlememeli")
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 0)

    def test_dogrulamada_hatali_saglayici_atlanir_farkli_cevap_bekletir(self):
        # A receipt'te hata verir; B ve C ayni -> tahsis
        self.durum_baslat()
        self.ag.hepsine("usdt_odeme", txh(3), ALICI, 100 * E18, 1010)
        orj = self.ag[A].cagir
        self.ag[A].cagir = lambda m, p: (_ for _ in ()).throw(OSError("403")) if m == "eth_getTransactionReceipt" else orj(m, p)
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 500)

    def test_basarili_cevaplardan_biri_farkliysa_tahsis_yok(self):
        # C sahte (farkli deger) receipt verir -> A,B ayni olsa bile UYUSMAZLIK -> bekler
        self.durum_baslat()
        self.ag[A].usdt_odeme(txh(4), ALICI, 100 * E18, 1010)
        self.ag[B].usdt_odeme(txh(4), ALICI, 100 * E18, 1010)
        self.ag[C].usdt_odeme(txh(4), ALICI, 9999 * E18, 1010)
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 0)
        self.assertIn(txh(4), self.durum()["bekleyen"])


class T15_YenidenDeneme(Temel):
    def test_gecici_hata_bir_kez_yeniden_denenir_kalici_denenmez(self):
        m = self.yukle()
        cagri = []
        def ham(url, method, params):
            cagri.append(method)
            if len(cagri) == 1:
                raise OSError("HTTP Error 429: Too Many Requests")
            return "0x10"
        m._rpc_cagir_ham = ham
        # ozgun rpc_cagir (test yamasi olmadan) modulden yeniden yuklenir
        spec = importlib.util.spec_from_file_location("izl_ham", IZLEYICI)
        orj = importlib.util.module_from_spec(spec); spec.loader.exec_module(orj)
        orj._rpc_cagir_ham = ham
        orj.time = types.SimpleNamespace(time=__import__("time").time, sleep=lambda x: None)
        self.assertEqual(orj.rpc_cagir("u", "eth_blockNumber", []), "0x10")
        self.assertEqual(len(cagri), 2, "gecici hata bir kez yeniden denendi")
        cagri.clear()
        def kalici(url, method, params):
            cagri.append(method); raise OSError("HTTP Error 403: Forbidden")
        orj._rpc_cagir_ham = kalici
        with self.assertRaises(OSError):
            orj.rpc_cagir("u", "eth_getLogs", [])
        self.assertEqual(len(cagri), 1, "kalici hata yeniden denenmez")


# ================================================================== 16: yakalama modu
X = "https://bsc.rpc.blxrbdn.com"   # genis aralik veren saglayici (yakalama tercihi)

class T16_YakalamaModu(Temel):
    URLLER = [A, X, C]

    def _yukseklik(self, h):
        for u in self.URLLER:
            self.ag[u].head = h

    def test_geride_tek_saglayiciyla_hizli_kesif_tahsis_2_dogrulamayla(self):
        self.durum_baslat(son_blok=1000)
        self._yukseklik(11_000)
        # Gercek odeme: log yalniz X'te (digerleri eski blok vermiyor), receipt HEPSINDE
        self.ag[X].usdt_odeme(txh(7), ALICI, 100 * E18, 5_000)
        for u in (A, C):
            self.ag[u].usdt_odeme(txh(7), ALICI, 100 * E18, 5_000, log_listede=False)
        self.calistir()
        d = self.durum()
        self.assertEqual(d["son_blok"], 11_000 - 15 - 1000, "yakalama uca ESIK/2 kala durur")
        self.assertTrue(all(e - s + 1 <= 5000 for s, e, _ in self.ag[X].getlogs_cagri))
        self.assertEqual(d["tek_kaynakli"], [[1001, d["son_blok"]]], "tek kaynakli aralik kayitli")
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 500, "2 saglayici dogrulamasiyla tahsis")

    def test_yakalamada_tek_saglayicinin_sahte_iddiasi_tahsis_ettirmez(self):
        self.durum_baslat(son_blok=1000)
        self._yukseklik(11_000)
        self.ag[X].usdt_odeme(txh(8), ALICI, 100 * E18, 5_000)   # yalniz X iddia ediyor
        self.calistir()
        self.assertEqual(self.dugum.toplam_aidag(ALICI), 0)
        self.assertIn(txh(8), self.durum()["bekleyen"], "aday kuyrukta bekler (dogrulanamadi)")

    def test_uca_yakinken_normal_kural_iki_saglayici(self):
        self.durum_baslat(son_blok=1000)
        self._yukseklik(1_500)   # 485 blok geride < esik
        for u in (A, C):
            orj = self.ag[u].cagir
            self.ag[u].cagir = (lambda o: lambda m, p: (_ for _ in ()).throw(OSError("403")) if m == "eth_getLogs" else o(m, p))(orj)
        self.calistir()
        self.assertEqual(self.durum()["son_blok"], 1000, "uca yakinken tek saglayici yetmez")


# ================================================================== betikler (10-13)
def calistir_betik(args, env=None, cwd=None):
    return subprocess.run(args, capture_output=True, text=True, env=env, cwd=cwd, timeout=60)


class T10_Betikler(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="betik-test-")
        self.bin = os.path.join(self.tmp, "bin"); os.makedirs(self.bin)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def arac(self, ad, govde):
        p = os.path.join(self.bin, ad)
        open(p, "w").write("#!/bin/bash\n" + govde + "\n")
        os.chmod(p, 0o755)
        return p

    def ortam(self, **ek):
        e = {"PATH": self.bin + ":/usr/bin:/bin", "HOME": self.tmp}
        e.update(ek); return e

    def test_bash_n_tum_betikler(self):
        for b in ("on-satis-kaydet.sh", "tge-ayarla-kaydet.sh", "yayinla.sh"):
            r = calistir_betik(["bash", "-n", os.path.join(KOK, b)])
            self.assertEqual(r.returncode, 0, b + r.stderr)

    def _sahte_curl(self, tge="2000000000"):
        tips = ",".join('"%064x"' % i for i in range(12))
        return self.arac("curl", f'''
for a in "$@"; do u="$a"; done
for a in "$@"; do case "$a" in http*) u="$a";; esac; done
case "$u" in
  */tips) echo '{{"tips":[{tips}]}}';;
  */on-satis-tahsis/*) echo '{{"tge":{tge}}}';;
  */on-satis-ozet) echo '{{"toplam_satilan_aidag":"0"}}';;
  */on-satis/*) echo '{{"ok":true,"bulundu":false}}';;
  */submit) echo '{{"ok":true}}';;
esac''')

    def test_on_satis_kaydet_binary_ye_9_arguman_ve_en_fazla_8_tip(self):
        self._sahte_curl()
        kayit = os.path.join(self.tmp, "args.txt")
        sahte_bin = self.arac("on-satis-tahsis", f'printf "%s\\n" "$#" "$@" > {kayit}; echo deadbeef')
        anahtar = os.path.join(self.tmp, "sahte.key"); open(anahtar, "wb").write(b"\x01" + b"\x00" * 32)
        r = calistir_betik(["bash", os.path.join(KOK, "on-satis-kaydet.sh"), ALICI, "500", "2", "123"],
                           env=self.ortam(BIN=sahte_bin, KEY=anahtar, RPC="http://sahte"))
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        satir = open(kayit).read().splitlines()
        self.assertEqual(satir[0], "9")
        self.assertEqual(satir[1:], [anahtar, "1", ALICI, ALICI, "500", "2", "123", satir[8], satir[9]])
        self.assertEqual(len(satir[9].split(",")), 8)

    def test_tge_ayarla_kaydet_en_fazla_8_tip(self):
        self._sahte_curl(tge="4102444800")
        kayit = os.path.join(self.tmp, "args.txt")
        sahte_bin = self.arac("tge-ayarla", f'printf "%s\\n" "$#" "$@" > {kayit}; echo deadbeef')
        r = calistir_betik(["bash", os.path.join(KOK, "tge-ayarla-kaydet.sh"), "4102444800"],
                           env=self.ortam(BIN=sahte_bin, KEY="/yok", RPC="http://sahte"))
        satir = open(kayit).read().splitlines()
        self.assertEqual(satir[0], "5")
        self.assertEqual(len(satir[5].split(",")), 8)

    # ---- yayinla.sh
    def _repo(self):
        uzak = os.path.join(self.tmp, "uzak.git")
        calistir_betik(["git", "init", "-q", "--bare", uzak])
        repo = os.path.join(self.tmp, "repo"); os.makedirs(repo)
        g = lambda *a: calistir_betik(["git", "-C", repo, "-c", "user.name=t", "-c", "user.email=t@t", *a])
        g("init", "-q", "-b", "main")
        open(os.path.join(repo, "README.md"), "w").write("10 tests, fmt\n")
        g("add", "."); g("commit", "-q", "-m", "ilk"); g("remote", "add", "origin", uzak)
        open(os.path.join(repo, "README.md"), "a").write("degisti\n")
        open(os.path.join(repo, "gizli.key"), "w").write("x")          # izlenmeyen
        return repo, uzak

    def _yayinla(self, cargo_cikti, kod=0):
        # GUVENLIK: eski yayinla.sh DIZIN'i sabit /root/aidag-lsc aliyor ve orada commit+push yapiyordu.
        # Betik DIZIN ortam degiskenine uymuyorsa CALISTIRMADAN kal (gercek depoya dokunulmaz).
        kaynak = open(os.path.join(KOK, "yayinla.sh")).read()
        if 'DIZIN="${DIZIN:-' not in kaynak:
            self.fail("yayinla.sh DIZIN ortam degiskenine uymuyor; gercek depoya dokunmamak icin calistirilmadi")
        repo, uzak = self._repo()
        self.arac("cargo", f"cat <<'X'\n{cargo_cikti}\nX\nexit {kod}")
        r = calistir_betik(["bash", os.path.join(KOK, "yayinla.sh"), "deneme"],
                           env=self.ortam(DIZIN=repo, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t",
                                          GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@t"))
        dallar = calistir_betik(["git", "--git-dir", uzak, "branch", "--list"]).stdout
        return r, repo, dallar

    def test_yayinla_ikinci_grup_kirmizi_ise_durur(self):
        r, _, dallar = self._yayinla("test result: ok. 100 passed; 0 failed\ntest result: FAILED. 5 passed; 2 failed")
        self.assertNotEqual(r.returncode, 0); self.assertEqual(dallar.strip(), "")

    def test_yayinla_derleme_hatasi_durur(self):
        r, _, dallar = self._yayinla("error[E0425]: cannot find value\ntest result: ok. 1 passed; 0 failed", kod=101)
        self.assertNotEqual(r.returncode, 0); self.assertEqual(dallar.strip(), "")

    def test_yayinla_error_satiri_cikis_0_olsa_bile_durur(self):
        r, _, dallar = self._yayinla("error: could not compile\ntest result: ok. 1 passed; 0 failed")
        self.assertNotEqual(r.returncode, 0); self.assertEqual(dallar.strip(), "")

    def test_yayinla_yesil_ise_dala_pushlar_main_e_degil_izlenmeyeni_eklemez(self):
        r, repo, dallar = self._yayinla("test result: ok. 100 passed; 0 failed\ntest result: ok. 7 passed; 0 failed")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertNotIn("main", dallar.replace("yayin/", ""))
        self.assertIn("yayin/", dallar)
        izlenen = calistir_betik(["git", "-C", repo, "ls-files"]).stdout
        self.assertNotIn("gizli.key", izlenen)
        self.assertIn("107 tests, fmt", open(os.path.join(repo, "README.md")).read())

    # ---- zincire-yaz.js
    def test_zincire_yaz_anahtarsiz_hata_verir_sabit_seed_yok(self):
        # DENETIM K-08: anahtar yalniz AIDAG_KAYIT_ANAHTARI dosyasindan; 0600 zorunlu; demo tohum reddedilir.
        js = os.path.join(KOK, "zincire-yaz.js")
        kaynak = open(js).read()
        self.assertNotIn("fill(7)", kaynak)
        self.assertIn("Rejected", kaynak)                      # /submit reddi hata sayilir
        if not shutil.which("node"):
            self.skipTest("node yok")
        e = {"PATH": "/usr/bin:/bin", "HOME": self.tmp}
        r = calistir_betik(["node", js], env=e)
        self.assertNotEqual(r.returncode, 0); self.assertIn("AIDAG_KAYIT_ANAHTARI", r.stderr)
        acik = os.path.join(self.tmp, "acik"); open(acik, "wb").write(os.urandom(32)); os.chmod(acik, 0o644)
        r = calistir_betik(["node", js], env=dict(e, AIDAG_KAYIT_ANAHTARI=acik))
        self.assertNotEqual(r.returncode, 0); self.assertIn("0600", r.stderr)
        demo = os.path.join(self.tmp, "demo"); open(demo, "wb").write(b"\x07" * 32); os.chmod(demo, 0o600)
        r = calistir_betik(["node", js], env=dict(e, AIDAG_KAYIT_ANAHTARI=demo))
        self.assertNotEqual(r.returncode, 0); self.assertIn("demo", r.stderr)

    def test_eski_belge_sayfasi_yonlendirme(self):
        s = open(os.path.join(KOK, "web", "belge-dogrulama.html")).read()
        self.assertIn('http-equiv="refresh" content="0; url=/belge"', s)
        self.assertIn('href="/belge"', s)
        self.assertNotIn("NETWORK_ID", s)

    def test_durum_dosyalari_git_te_izlenmiyor(self):
        r = calistir_betik(["git", "-C", KOK, "ls-files", ".on-satis-islenmis.json", ".on-satis-adres-usd.json"])
        self.assertEqual(r.stdout.strip(), "")
        r = calistir_betik(["git", "-C", KOK, "check-ignore", ".on-satis-islenmis.json",
                            ".on-satis-adres-usd.json", ".on-satis-durum.json"])
        self.assertEqual(len(r.stdout.split()), 3)


if __name__ == "__main__":
    unittest.main(verbosity=2)
