# AIDAG-Chain — Uzak Dugum Testi (Prosedur)

Amac: mainnet'e **sunucu disindaki** bir cihazdan (telefon ya da baska bir bilgisayar),
**gercek internet uzerinden** baglanan bir dugumle cok dugumlu mutabakati tekrarlanabilir
sekilde kanitlamak ve kanitlari (loglar) repoda saklamak.

Ilk (elle, tek seferlik) deneme: 18 Temmuz 2026 — bkz. `TESTLER.md` "TARIHCE".

Dogrulanacaklar:
1. Uzak dugum **ayni pinli genesis**'i yukler (`b82345008ae109d8`).
2. Uzak dugum sunucudaki dugumlerle **ayni vertex sayisina** senkronize olur, **orphan = 0**.
3. **Belge yayilimi**: uzak dugume gonderilen bir belge kaydi sunucu dugumunde gorunur
   (istege bagli: ters yon da).
4. **Restart kaliciligi**: uzak dugum durdurulup ayni veri dosyasiyla yeniden baslatilinca
   ayni durumu diskten kurar.
5. Sunucu tarafinda uzak dugum **dis es** olarak gorunur.

## Guvenlik kurallari (once oku)

- Uzak dugum **listen** modunda calisir: genesis uretmez, vertex uretmez; yalniz dinler ve
  senkronize olur.
- 3. adimdaki belge kaydi **mainnet'e kalici** olarak yazilir (geri alinamaz). Icerik yalniz
  bir test cumlesidir; **kisisel veri yazma**.
- Belge gonderen anahtar her calistirmada **gecici** uretilir (diske yazilmaz).
- Uzak dugumun olusturdugu `aidag-key-<port>.bin` ve `aidag-data-*.log` dosyalari **repoya
  girmez** (`.gitignore` kapsamindadir). Loglar repoya **`.txt`** olarak kopyalanir.
- Repoya girecek kayitlarda uzak cihazin **IP adresi maskelenir** (8. adim).
- Sunucudaki canli dugumlere (`lsc-node`, `aidag-mainnet`) **dokunulmaz**: yeniden baslatma,
  durdurma, deploy YOK. Bu test yalniz okur + bir belge kaydi gonderir.

## On kosullar

- Uzak cihaz sunucudan **farkli bir agda** olmali (telefonda mobil veri ideal; Wi-Fi de olur,
  ama sunucuyla ayni yerel ag olmamali).
- Sunucuda P2P portu acik: `40001/tcp` (ufw kuralinda "AIDAG p2p").
- Uzak cihazda: `git`, Rust (`cargo`), Python 3 + `pip install blake3 pynacl requests`.
  - Android (Termux): `pkg install git rust clang python` — derleme telefonda uzun surebilir.
- `<SUNUCU_IP>`: mainnet dugumunun genel IP'si (operator bilir; belgeye yazilmaz).

## 0. Kayit klasoru (sunucuda, repo icinde)

    cd /root/aidag-lsc
    T=$(date +%F); K=test-kayitlari/uzak-dugum/$T; mkdir -p $K

## 1. Sunucu — onceki durum

    { date -u; for p in 8645 8655; do curl -s 127.0.0.1:$p/status; echo; done
      echo "dis es (40001):"; ss -tn state established '( sport = :40001 )'; } \
      > $K/01-sunucu-once.txt

Beklenen: iki dugumde ayni `vertex_count`, `orphan_count 0`, `genesis b82345008ae109d8`.

## 2. Uzak cihaz — derleme

    git clone https://github.com/DeepSea3474/aidag-chain-engine.git
    cd aidag-chain-engine
    cargo build --release -p lsc-net

## 3. Uzak cihaz — mainnet'e baglan (listen)

    mkdir -p $HOME/aidag-uzak && cd $HOME/aidag-uzak
    LSC_MAINNET=1 LSC_PRODUCTION=1 \
    LSC_BOOTSTRAP=/ip4/<SUNUCU_IP>/tcp/40001 \
    LSC_RPC_ADDR=127.0.0.1:8645 \
      ~/aidag-chain-engine/target/release/lsc-node /ip4/0.0.0.0/tcp/40002 \
      $HOME/aidag-uzak/aidag-data-mainnet.log listen 2>&1 | tee -a uzak-dugum-log.txt

Veri dosyasi **mutlak yolla** verilir (aksi halde restart'ta yuklenmez).

## 4. Senkron + ayni genesis + orphan=0 (uzak cihazda, ikinci terminal)

    curl -s 127.0.0.1:8645/status | tee uzak-status-1.json

Beklenen: `network_id 3474`, `genesis b82345008ae109d8`, `orphan_count 0`, `vertex_count`
sunucudakiyle ayni (test sirasinda yeni vertex geldiyse birkac saniye sonra tekrar bak).

Sunucuda uzak dugumun dis es olarak gorundugunu kaydet. Dugum gunlugu her baglantida
`ES BAGLANDI: peer=<id> yon=gelen|giden adres=/ip4/a.b.x.x/tcp/<port>` yazar (IP'nin son iki
okteti maskeli; 45.13.x.x ve 172.x.x.x sunucunun kendi adresleridir, dis dugum bunlardan farklidir):

    { ss -tn state established '( sport = :40001 )'
      journalctl -u lsc-node --since "-30 min" --no-pager | grep -E "ES (BAGLANDI|AYRILDI)"; } \
      > $K/04-sunucu-dis-es.txt

## 5. Belge yayilimi — uzak dugum -> sunucu

Uzak cihazda (`aidag-chain-engine/sdk/python` klasorunde):

    python3 - <<'PY' | tee belge-uzak.txt
    import hashlib, time, secrets
    from aidag_sdk import AidagClient
    c = AidagClient("http://127.0.0.1:8645")          # network_id /status'tan okunur (3474)
    metin = f"AIDAG uzak dugum testi {time.strftime('%F %T')} {secrets.token_hex(4)}".encode()
    h = hashlib.blake2b(metin, digest_size=32).digest()
    tips = [bytes.fromhex(t) for t in c.tips().get("tips", [])]
    print("metin:", metin.decode()); print("hash:", h.hex())
    print("submit:", c.submit(c.vertex_olustur(tips, c.record_payload(h), int(time.time()))))
    time.sleep(3); print("uzak /belge:", c.belge_dogrula(h.hex()))
    PY

Sunucuda (hash'i `belge-uzak.txt`'den al):

    curl -s 127.0.0.1:8645/belge/<HASH> | tee $K/05-belge-sunucuda.json

Beklenen: `"kayitli": true`, `kaydeden` = uzak cihazdaki gecici adres.

(Istege bagli, ters yon) Ayni betigi sunucuda `http://127.0.0.1:8645` ile calistir, cikan
hash'i uzak cihazda `curl -s 127.0.0.1:8645/belge/<HASH>` ile dogrula.

## 6. Restart kaliciligi (uzak dugum)

Uzak dugumu `Ctrl+C` ile durdur, **3. adimdaki komutu aynen** tekrar calistir. Logda:

    Diskten <N> vertex yuklendi: toplam_vertex=<N>, bekleyen_orphan=0

satiri gorulmeli. Sonra:

    curl -s 127.0.0.1:8645/status | tee uzak-status-2-restart.json
    curl -s 127.0.0.1:8645/belge/<HASH> | tee belge-restart-sonrasi.json

Beklenen: ayni genesis, `vertex_count` >= restart oncesi, `orphan_count 0`, belge hala kayitli.

## 7. Sunucu — sonraki durum

    { date -u; for p in 8645 8655; do curl -s 127.0.0.1:$p/status; echo; done; } \
      > $K/07-sunucu-sonra.txt

## 8. Kayitlari repoya al

Uzak cihazdaki `uzak-dugum-log.txt`, `uzak-status-*.json`, `belge-uzak.txt`,
`belge-restart-sonrasi.json` dosyalarini sunucuda `$K/` altina kopyala (scp ya da elle).
Uzun logu kisalt ve uzak IP'yi maskele:

    cd $K
    { head -n 200 uzak-dugum-log.txt; echo "..."; tail -n 200 uzak-dugum-log.txt; } > 03-uzak-dugum-log.txt
    rm uzak-dugum-log.txt
    sed -i -E 's/<UZAK_IP>/x.x.x.x/g' *.txt *.json     # uzak cihazin genel IP'si

`$K/OZET.md` olustur:

    # Uzak dugum testi — <TARIH>
    Uzak cihaz: <telefon/Termux | bilgisayar>, ag: <mobil veri | ...>
    - [ ] Ayni genesis (b82345008ae109d8)
    - [ ] Senkron: uzak vertex_count = sunucu vertex_count, orphan = 0
    - [ ] Sunucuda dis es gorundu (04-sunucu-dis-es.txt)
    - [ ] Belge yayilimi uzak -> sunucu (05-belge-sunucuda.json)
    - [ ] (istege bagli) Belge yayilimi sunucu -> uzak
    - [ ] Restart kaliciligi ("Diskten N vertex yuklendi", belge hala kayitli)
    Notlar / sorunlar:

Commit (dalda, PR ile):

    git switch -c uzak-dugum-testi-$T
    git add $K && git commit -m "Uzak dugum testi kayitlari ($T)"

## Temizlik

Uzak dugumu durdur. `$HOME/aidag-uzak` (veri dosyasi + `aidag-key-40002.bin`) silinebilir;
repoya girmez.

## Sorun giderme

- `vertex_count` 1'de kaliyor / baglanti yok: `<SUNUCU_IP>` ve `40001/tcp` erisimini kontrol et;
  mobil operator giden TCP'yi engelliyor olabilir (baska ag dene).
- `genesis` farkli: `LSC_MAINNET=1` verilmemistir.
- Restart'ta `vertex_count=0`: veri dosyasi mutlak yolla verilmemistir.
- Belge `kayitli: false`: birkac saniye bekle; `submit` cevabini `belge-uzak.txt`'de kontrol et.
