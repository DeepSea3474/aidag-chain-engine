"""AIDAG-Chain Python SDK.

Gelistiricinin AIDAG zincirine baglanmasi icin minimal kutuphane:
  - ed25519 anahtar yonetimi
  - vertex olusturma + imzalama (lsc-engine format ile BIREBIR uyumlu)
  - token kaydi payload'i olusturma
  - RPC ile zincire gonderme (/submit) ve sorgulama (/status, /tokens)

Format kaynagi: lsc-engine/src/dag/vertex.rs + wire.rs + tx.rs + registry.rs.
Gereken paketler: pip install blake3 pynacl requests
"""

import struct
import requests
from blake3 import blake3
from nacl.signing import SigningKey

DOMAIN_TAG = b"AIDAG-vertex-v1\x00"   # 16 bayt
FORMAT_VERSION = 1
WIRE_VERSION = 1
ADDR_LEN = 20
SYMBOL_LEN = 8
TX_TYPE_TOKEN = 2
TX_TYPE_STAKE = 3
TX_TYPE_TRANSFER = 4
TX_TYPE_RECORD = 1
TX_TYPE_KURUM = 5
TX_TYPE_FAUCET = 6
TX_TYPE_AVM_CAGRI = 9
TX_TYPE_ON_SATIS = 10
KURUM_DEVLET = 0
KURUM_OZEL = 1


def public_key_to_adres(public_key: bytes) -> bytes:
    """public_key (32) -> 20 baytlik adres = blake3(public_key)[:20].
    lsc-engine registry.rs::public_key_to_adres ile BIREBIR ayni."""
    return blake3(public_key).digest()[:ADDR_LEN]


class AidagClient:
    def __init__(self, rpc_url, network_id=None, signing_key=None):
        """network_id verilmezse dugumun /status ucundan OTOMATIK okunur.
        Boylece mainnet (3474) / testnet (1) ayrimi kullaniciya birakilmaz;
        yanlis ag ile gonderim (NetworkMismatch ile sessiz ret) onlenir.
        Ulasilamazsa 1 (devnet/testnet) varsayilir."""
        self.rpc_url = rpc_url.rstrip("/")
        if network_id is None:
            try:
                network_id = int(requests.get(f"{self.rpc_url}/status", timeout=5)
                                 .json()["network_id"])
            except Exception:
                network_id = 1
        self.network_id = network_id
        self.sk = SigningKey.generate() if signing_key is None else SigningKey(signing_key)
        self.public_key = bytes(self.sk.verify_key)

    def adres(self):
        return public_key_to_adres(self.public_key)

    def _hash_id(self, parents, timestamp, payload):
        h = blake3()
        h.update(DOMAIN_TAG)
        h.update(bytes([FORMAT_VERSION]))
        h.update(struct.pack("<I", self.network_id))
        h.update(self.public_key)
        h.update(struct.pack("<Q", len(parents)))
        for p in parents:
            h.update(p)
        h.update(struct.pack("<Q", timestamp))
        h.update(struct.pack("<Q", len(payload)))
        h.update(payload)
        return h.digest()

    def vertex_olustur(self, parents, payload, timestamp):
        parents = sorted(set(parents))
        vid = self._hash_id(parents, timestamp, payload)
        signature = self.sk.sign(vid).signature
        out = bytearray()
        out.append(WIRE_VERSION)
        out += struct.pack("<I", self.network_id)
        out += struct.pack("<Q", len(parents))
        for p in parents:
            out += p
        out += struct.pack("<Q", timestamp)
        out += struct.pack("<Q", len(payload))
        out += self.public_key
        out += signature
        out += payload
        return bytes(out)

    @staticmethod
    def token_payload(kanonik_adres, sembol):
        assert len(kanonik_adres) == ADDR_LEN
        sym = sembol.encode("ascii")[:SYMBOL_LEN].ljust(SYMBOL_LEN, b"\x00")
        return bytes([TX_TYPE_TOKEN]) + kanonik_adres + sym

    @staticmethod
    def stake_payload(staker_adres, miktar):
        """Stake payload: [3][staker:20][miktar:8 BIG-ENDIAN].
        DIKKAT: miktar BIG-endian (token sembolunden farkli; tx.rs StakeKaydi)."""
        assert len(staker_adres) == ADDR_LEN
        return bytes([TX_TYPE_STAKE]) + staker_adres + miktar.to_bytes(16, "big")  # u128 (16 bayt)

    @staticmethod
    def transfer_payload(alici_adres, miktar, nonce):
        """Transfer payload: [4][alici:20][miktar:8 BE][nonce:8 BE] -> 37 bayt.
        GONDEREN = vertex imzalayani (bu SDK'nin anahtari); payload'da YOK.
        nonce = gonderenin BEKLENEN nonce'u (replay korumasi). RPC /nonce/<adres>
        ile okunur; yanlis nonce -> transfer reddedilir."""
        assert len(alici_adres) == ADDR_LEN
        return bytes([TX_TYPE_TRANSFER]) + alici_adres + miktar.to_bytes(16, "big") + struct.pack(">Q", nonce)  # miktar u128(16), nonce u64(8)

    @staticmethod
    def record_payload(data_hash):
        """Belge/veri kaydi payload: [1][hash:32]. GERCEK DUNYA dogrulama.
        data_hash = belgenin parmak izi (orn. blake3(belge_icerigi), 32 bayt).
        Belgenin KENDISI zincire GIRMEZ; sadece hash. KAYDEDEN=imzalayan."""
        assert len(data_hash) == 32
        return bytes([TX_TYPE_RECORD]) + data_hash

    @staticmethod
    def kurum_payload(kategori, ad):
        """Kurum/firma kimlik kaydi: [5][kategori:1][ad].
        kategori: KURUM_DEVLET (0) veya KURUM_OZEL (1).
        KAYDEDEN = imzalayan (bu SDK'nin anahtari) -> baskasi adina kurum
        kaydedilemez. ad <= 64 bayt (UTF-8)."""
        assert kategori in (KURUM_DEVLET, KURUM_OZEL)
        ad_bytes = ad.encode("utf-8")
        assert len(ad_bytes) <= 64, "kurum adi 64 bayttan uzun olamaz"
        return bytes([TX_TYPE_KURUM, kategori]) + ad_bytes

    @staticmethod
    def faucet_payload(alici_adres, miktar):
        """Faucet payload: [6][alici:20][miktar:8 BE]. TESTNET test AIDAG.
        GUVENLIK: bu vertex'i FAUCET OWNER imzalamali; baskasi imzalarsa dugum
        reddeder. Yani bu SDK owner anahtariyla kullanilmalidir."""
        assert len(alici_adres) == 20
        return bytes([TX_TYPE_FAUCET]) + alici_adres + miktar.to_bytes(16, "big")  # u128 (16 bayt)

    @staticmethod
    def avm_payload(hedef_adres, deger, nonce, data=b""):
        """AVM cagrisi payload: [9][hedef:20][deger:8 BE][nonce:8 BE][data_len:4 BE][data:N].
        - hedef = sifir adres (20 bayt 0) + data dolu -> KONTRAT DEPLOY (CREATE), data=bytecode
        - hedef dolu + data dolu -> KONTRAT CAGRI (CALL), data=calldata (selector+argumanlar)
        - data bos -> basit LSC deger transferi
        deger = LSC (kontrata/hedefe gonderilen). nonce = replay korumasi.
        GONDEREN = imzalayan (bu SDK'nin anahtari)."""
        assert len(hedef_adres) == ADDR_LEN
        return (bytes([TX_TYPE_AVM_CAGRI]) + hedef_adres
                + deger.to_bytes(16, "big") + struct.pack(">Q", nonce)  # deger u128(16), nonce u64(8)
                + struct.pack(">I", len(data)) + data)

    def avm_deploy(self, bytecode, deger=0):
        """KOLAYLIK: bir sozlesmeyi (bytecode) zincire deploy et.
        Otomatik: nonce okur, tips alir, vertex olusturur, gonderir.
        Doner: submit yaniti (Integrated ise basarili). bytecode: derlenmis EVM kodu (bytes)."""
        import time
        adres_hex = self.adres().hex()
        n = self.nonce(adres_hex)
        sifir = bytes(ADDR_LEN)
        payload = self.avm_payload(sifir, deger, n, bytecode)
        tips = [bytes.fromhex(t) for t in self.tips().get("tips", [])]
        wire = self.vertex_olustur(tips, payload, int(time.time()))
        return self.submit(wire)

    def avm_call(self, kontrat_adres, calldata, deger=0):
        """KOLAYLIK: var olan bir sozlesmeyi cagir.
        kontrat_adres: 20 bayt sozlesme adresi. calldata: selector(4)+argumanlar.
        deger: kontrata gonderilen LSC (kontrat payable degilse 0 olmali).
        Otomatik: nonce/tips/vertex/submit."""
        import time
        assert len(kontrat_adres) == ADDR_LEN
        adres_hex = self.adres().hex()
        n = self.nonce(adres_hex)
        payload = self.avm_payload(kontrat_adres, deger, n, calldata)
        tips = [bytes.fromhex(t) for t in self.tips().get("tips", [])]
        wire = self.vertex_olustur(tips, payload, int(time.time()))
        return self.submit(wire)


    def on_satis_dagit(self, alici_adres, aidag, lsc_hediye, odeme_ref=0):
        """ON SATIS DAGITIM (tip=10). SADECE owner imzalamali (hazine).
        Hazineden aliciya AIDAG satilir + LSC hediye verilir.
        payload: [10][alici:20][aidag:16 u128][lsc_hediye:16 u128][odeme_ref:8 u64]."""
        import time
        assert len(alici_adres) == ADDR_LEN
        payload = (bytes([TX_TYPE_ON_SATIS]) + alici_adres
                   + aidag.to_bytes(16, "big")       # u128 (16 bayt)
                   + lsc_hediye.to_bytes(16, "big")  # u128 (16 bayt)
                   + struct.pack(">Q", odeme_ref))   # odeme_ref u64 (8 bayt) KALIR
        tips = [bytes.fromhex(t) for t in self.tips().get("tips", [])]
        wire = self.vertex_olustur(tips, payload, int(time.time()))
        return self.submit(wire)

    def on_satis_sorgu(self, odeme_ref):
        """Bir odeme referansinin dagitim kaydi (alici, aidag, lsc, zaman)."""
        return requests.get(f"{self.rpc_url}/on-satis/{odeme_ref}").json()

    def on_satis_ozet(self):
        """GENEL seffaflik: toplam satilan, alim sayisi, maskeli liste (zamana sirali)."""
        return requests.get(f"{self.rpc_url}/on-satis-ozet").json()

    def on_satis_adres(self, adres_hex):
        """KISISEL gorunum: bir alicinin kendi tum alimlari + toplam aldigi AIDAG."""
        return requests.get(f"{self.rpc_url}/on-satis-adres/{adres_hex}").json()

    def submit(self, wire_bytes):
        return requests.post(f"{self.rpc_url}/submit", data=wire_bytes.hex()).json()

    def status(self):
        return requests.get(f"{self.rpc_url}/status").json()

    def tokens(self):
        return requests.get(f"{self.rpc_url}/tokens").json()

    def tips(self):
        return requests.get(f"{self.rpc_url}/tips").json()

    def bakiye(self, adres_hex):
        return requests.get(f"{self.rpc_url}/bakiye/{adres_hex}").json()

    def nonce(self, adres_hex):
        """Bir adresin BEKLENEN nonce'u (replay korumasi). Transfer kurmadan
        ONCE cagrilmali; donen degeri transfer_payload'a ver."""
        return requests.get(f"{self.rpc_url}/nonce/{adres_hex}").json().get("nonce", 0)

    def belge_dogrula(self, hash_hex):
        """Bir belge hash'i zincirde kayitli mi? (kim, ne zaman)."""
        return requests.get(f"{self.rpc_url}/belge/{hash_hex}").json()

    def kurum_sorgula(self, adres_hex):
        """Bir adres hangi kurum/firma? (ad, kategori, zaman)."""
        return requests.get(f"{self.rpc_url}/kurum/{adres_hex}").json()

    def faucet(self, adres_hex=None):
        """TESTNET muslugu: bir adrese sabit test AIDAG verir (varsayilan: kendi
        adresim). Test AIDAG'in GERCEK DEGERI YOKTUR; sadece testnet icin."""
        if adres_hex is None:
            adres_hex = self.adres().hex()
        return requests.get(f"{self.rpc_url}/faucet/{adres_hex}").json()

    def faucet_bas(self, alici_adres, miktar=1000):
        """OWNER ICIN: aga-yayilan faucet vertex'i uretir ve gonderir. Bu SDK
        ornegi faucet OWNER anahtariyla olusturulmus olmali (signing_key=owner).
        Owner-imzali oldugu icin tum dugumlerde bakiye senkron olur.
        alici_adres: 20 baytlik adres (bytes). Donus: submit sonucu."""
        import time as _t
        if isinstance(alici_adres, str):
            alici_adres = bytes.fromhex(alici_adres)
        tips = [bytes.fromhex(t) for t in self.tips().get("tips", [])]
        payload = self.faucet_payload(alici_adres, miktar)
        wire = self.vertex_olustur(tips, payload, int(_t.time()))
        return self.submit(wire)

    def test_bakiye_ekle(self, adres_hex, miktar):
        """DEVNET/TEST: bir adrese bakiye basla (gercek arz degil)."""
        # Buyuk degerler (18 ondalik) JSON sayi limitini asar -> string gonder.
        return requests.post(f"{self.rpc_url}/test_bakiye",
                             json={"adres": adres_hex, "miktar": str(miktar)}).json()


# ============================================================
# AVM yardimcilari: keccak256 + fonksiyon selector + ABI encode
# (Bagimliliksiz saf-Python. keccak256 dogrulandi: bos string ->
#  c5d2460186f7233c... ve "kaydet(bytes32)" -> e89e74f7, Solidity ile eslesti.)
# ============================================================

def keccak256(msg: bytes) -> bytes:
    """Ethereum keccak-256 (NOT: standart SHA3-256'dan FARKLI). Saf Python."""
    if isinstance(msg, str):
        msg = msg.encode()
    RC = [0x1,0x8082,0x800000000000808A,0x8000000080008000,0x808B,0x80000001,
          0x8000000080008081,0x8000000000008009,0x8A,0x88,0x80008009,0x8000000A,
          0x8000808B,0x800000000000008B,0x8000000000008089,0x8000000000008003,
          0x8000000000008002,0x8000000000000080,0x800A,0x800000008000000A,
          0x8000000080008081,0x8000000000008080,0x80000001,0x8000000080008008]
    ROT = [[0,36,3,41,18],[1,44,10,45,2],[62,6,43,15,61],[28,55,25,21,56],[27,20,39,8,14]]
    def rol(x,n): return ((x<<n)|(x>>(64-n)))&0xFFFFFFFFFFFFFFFF
    def f(st):
        for r in range(24):
            C=[st[x][0]^st[x][1]^st[x][2]^st[x][3]^st[x][4] for x in range(5)]
            D=[C[(x-1)%5]^rol(C[(x+1)%5],1) for x in range(5)]
            for x in range(5):
                for y in range(5): st[x][y]^=D[x]
            B=[[0]*5 for _ in range(5)]
            for x in range(5):
                for y in range(5): B[y][(2*x+3*y)%5]=rol(st[x][y],ROT[x][y])
            for x in range(5):
                for y in range(5): st[x][y]=B[x][y]^((~B[(x+1)%5][y])&B[(x+2)%5][y])
            st[0][0]^=RC[r]
        return st
    rate=136
    st=[[0]*5 for _ in range(5)]
    m=bytearray(msg); m.append(0x01)
    while len(m)%rate!=0: m.append(0x00)
    m[-1]^=0x80
    for off in range(0,len(m),rate):
        blk=m[off:off+rate]
        for i in range(rate//8):
            st[i%5][i//5]^=int.from_bytes(blk[i*8:i*8+8],'little')
        st=f(st)
    out=b''
    for i in range(4): out+=st[i%5][i//5].to_bytes(8,'little')
    return out[:32]


def fonksiyon_selector(imza: str) -> bytes:
    """Solidity fonksiyon selector: keccak256(imza)[:4].
    Ornek: fonksiyon_selector("kaydet(bytes32)") -> b'\xe8\x9et\xf7'.
    imza: bosluksuz tip listesi, orn "transfer(address,uint256)"."""
    return keccak256(imza.encode())[:4]


def abi_bytes32(deger) -> bytes:
    """32 baytlik degeri (hash gibi) ABI word'e cevirir (zaten 32 bayt)."""
    if isinstance(deger, str):
        deger = bytes.fromhex(deger.replace("0x", ""))
    assert len(deger) == 32, "bytes32 tam 32 bayt olmali"
    return deger

def abi_address(adres) -> bytes:
    """20 baytlik adresi 32 baytlik ABI word'e cevirir (sol 12 bayt sifir dolgu)."""
    if isinstance(adres, str):
        adres = bytes.fromhex(adres.replace("0x", ""))
    assert len(adres) == 20, "address tam 20 bayt olmali"
    return b"\x00" * 12 + adres

def abi_uint256(deger: int) -> bytes:
    """uint256 sayiyi 32 baytlik ABI word'e cevirir (big-endian)."""
    assert deger >= 0, "uint256 negatif olamaz"
    return deger.to_bytes(32, "big")

def avm_calldata(imza: str, *argumanlar) -> bytes:
    """KOLAYLIK: fonksiyon imzasi + argumanlardan calldata uretir.
    calldata = selector(4) + her argumanin 32-bayt ABI word'u.
    Argumanlar onceden encode edilmis 32-bayt word olmali (abi_bytes32/
    abi_address/abi_uint256 ile). Ornek:
        cd = avm_calldata("kaydet(bytes32)", abi_bytes32(belge_hash))
        cli.avm_call(kontrat_adres, cd)
    NOT: Bu BASIT ABI encoder'dir - sadece sabit-boyutlu tipler (bytes32,
    address, uint256). Dinamik tipler (string, bytes, dizi) DESTEKLENMEZ;
    onlar icin tam ABI encoder gerekir."""
    data = fonksiyon_selector(imza)
    for arg in argumanlar:
        assert isinstance(arg, (bytes, bytearray)) and len(arg) == 32, \
            "her arguman 32-bayt ABI word olmali (abi_* yardimcilarini kullan)"
        data += bytes(arg)
    return data
