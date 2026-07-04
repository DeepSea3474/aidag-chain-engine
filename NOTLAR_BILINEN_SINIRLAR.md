# AIDAG-Chain / LSC — Bilinen Sınırlamalar ve Gelecek İşler

Bu dosya, bilinçli olarak "şimdilik basit" bırakılan ve ileride
sağlamlaştırılması GEREKEN konuları kaydeder. Her madde, teknik borç
(technical debt) olarak takip edilmelidir.

## 1. Node Senkronizasyonu (Sync) — ÇÖZÜLDÜ (temel) ✓
**Durum:** ÇÖZÜLDÜ (8 Haz 2026). Pull-sync (request-response) calisiyor;
3-node zincir topolojisinde (A<-B<-C) yakinsama dogrulandi: gec katilan C,
B'den genesis dahil gecmisi cekti, ucu de ayni DAG'a ulasti (toplam=8).
**ESKI (cozulmeden once):** "Push sync" denendi, ÇALIŞMADI (kanıtlandı).
**Push sync neden çalışmadı (8 Haz 2026 keşfi):**
Periyodik olarak vertex'leri gossipsub'a yeniden publish etmeye çalıştık.
Ama gossipsub'in seen-cache'i (blake3 message_id, DoS/loop korumasi) eski
mesajları "Duplicate" diye REDDETTI. Yani gossipsub publish, gec katilan
node'a GECMISI yeniden gondermek icin KULLANILAMAZ. Bu, gossipsub'in
dogasi (her mesaj bir kez yayilir) ile sync ihtiyaci (gecmisi tekrar
gonder) arasindaki temel celiskidir.
**ÇÖZÜM (yapilacak):** PULL SYNC = request-response protokolu.
- Yeni node, baglandigi peer'a "GetVertices" ISTEGI yollar.
- Peer, export_vertices() ile TUM vertex'leri DOGRUDAN (gossipsub'siz,
  seen-cache'siz) cevap olarak gonderir.
- Alici ingest_synced (orphan+cascade) ile sirasiz gelenleri cozer.
- libp2p request_response::Behaviour, mevcut LscBehaviour'a eklenir.
**Yapildi:** Message kolu ingest -> ingest_networked'e cevrildi (orphan-
bilincli, DOGRU degisiklik, korundu). Push sync kaldirildi (calismiyordu).
**ESKI NOT (gecersiz):** Su an "push sync" kullanılıyor (basit, geçici).
**Nasıl çalışıyor:** Yeni bir peer abone olunca, mevcut node tüm
vertex'lerini (export_vertices) gossipsub topic'ine yeniden yayınlar.
**Neden geçici/yetersiz:**
- Her yeni peer'da TÜM geçmiş, TÜM ağa yeniden yayınlanır (verimsiz).
- Büyük ağda / büyük zincirde ölçeklenmez (bant genişliği patlar).
- Yeni gelene değil, herkese gider (gereksiz trafik).
**İleride GEREKLİ çözüm:** "Pull sync" (request-response protokolü).
- Yeni node, bir peer'dan SADECE eksik olanı ISTER.
- libp2p request-response behaviour eklenir.
- Belki: checkpoint/snapshot, parça parça (chunked) transfer.
**Ne zaman:** Çok-node ölçek testlerinde (Adım 3 sonrası) veya gerçek
testnet'e geçmeden ÖNCE. Gerçek dünyada node'lar sürekli katılır/ayrılır;
bu mekanizma sağlam olmadan testnet açılmamalı.

## 2. Orphan'ların Diske Yazılması
**Durum:** Sadece Integrated (graf'a giren) vertex'ler diske yazılıyor.
Orphan (Buffered) vertex'ler diske YAZILMIYOR.
**Sonuç:** Bir vertex orphan'dayken cascade ile çözülürse, o an diske
yazılmıyor. Restart'ta ağdan tekrar gelir (kayıp yok ama optimal değil).
**İleride:** Cascade ile çözülen vertex'i de diske yazma mekanizması.

## 3. libp2p Peer Kimliği Kalıcılığı
**Durum:** Vertex imzalama anahtarı kalıcı (Adım 2). Ama libp2p'nin kendi
peer_id'si (with_new_identity) her başlangıçta YENİDEN üretiliyor.
**İleride:** libp2p keypair'ini de diske kaydet (kalıcı peer_id).

## 4. Pull-sync: "tum gecmisi cek" (verimsizlik) — GELECEK
**Durum:** Calisiyor ama optimal degil. Yeni baglanan node, peer'dan
SyncRequest ile TUM vertex'leri ceker (export_vertices). Kucuk zincirde
sorun yok; buyuk zincirde (binlerce vertex) verimsiz: gereksiz veri + tek
mesajda sigmama riski.
**Gelecek cozum:** "Eksik olani cek" — istek "su id'den sonrasini ver"
veya checkpoint tabanli; cevap parca parca (chunked). Gercek olcek
testlerinde ele alinacak.

## 5. mDNS: sadece yerel ag (LAN) — GELECEK
**Durum:** mDNS otomatik kesif CALISIYOR ama sadece ayni yerel agda
(multicast). Internet uzerinden (farkli aglardaki node'lar) kesif YAPMAZ.
**Gelecek cozum:** Bootstrap node'lar (bilinen sabit adresler) + Kademlia
DHT (libp2p kad) ile internet-olcegi peer kesfi. Gercek testnet (farkli
sunucularda node'lar) icin SART. mDNS yerel gelistirme/test icin yeterli.

## 6. Cozulen: Iki-genesis cakismasi (8 Haz 2026) ✓
Iki uretici ayni anda baslayinca her biri kendi genesis'ini uretip ag
boluniyordu. COZULDU: ikinci uretici (produce modu), ortak genesis'i
edinene kadar uretim yapmaz (genesis_id().is_some() bekler). 2-uretici
paralel DAG yakinsamasi dogrulandi (orphan=0).

## 7. Cozulen/Dogrulanan: Node restart toparlanmasi (8 Haz 2026) ✓
**Senaryo (Adim 6a):** Bir node (B) cokup yeniden baslayinca toparlaniyor mu?
**Test:** A+B senkronize (B=3 vertex) -> B OLDURULDU -> A kapaliyken uretmeye
devam etti (16 vertex daha) -> B AYNI port/veri ile YENIDEN baslatildi.
**Sonuc — TAM TOPARLANMA:**
- B diskten eski verisini yukledi: "Diskten 3 vertex yuklendi" (KALICILIK ✓)
- B, A'ya tekrar baglanip kacirdiklarini cekti (PULL-SYNC ✓)
- B son durum: toplam_vertex=20 (A=19+genesis ile YAKINSADI ✓)
**Anlami:** Adim 1 (kalicilik) + Adim 3 (pull-sync) BIRLIKTE calisip gercek
cokme-toparlanma senaryosunu cozuyor. Mevcut kod bunu zaten yapabiliyordu
(yeni kod gerekmedi) -> onceki parcalarin saglamligi kaniti. Gercek agda
node'lar surekli iner/kalkar; sistem buna dayanikli.
**NOT:** Bu senaryo timing'e bagli oldugu icin test-convergence.sh'e
EKLENMEDI (flaky olabilir); elle dogrulandi + burada kayitli.

## 8. Dogrulanan: Bozuk/gecersiz vertex'e dayaniklilik (Adim 6b) ✓
**Soru:** Aga cop/bozuk/saldirgan vertex gelirse node saglam kalir mi?
**Durum:** ZATEN KORUMALI + test edilmis (yeni kod gerekmedi).
**Engine savunmasi (unit testlerde kanitli):**
- ingest_garbage_leaves_state_untouched: cop gelince DURUM BOZULMAZ
- networked_garbage_rejected: ag yolu copu reddeder
- wire katmani: wrong_version, empty_buffer, oversized_parent/payload
  (before_alloc -> bellek-tasmasi saldirisina karsi bile korumali),
  declared_*_exceeds_buffer, trailing_bytes -> hepsi REDDEDILIR
**Ag savunmasi:** mesaj kolu NetworkIngestOutcome::Rejected'i handle eder ->
bozuk vertex "reddedildi" loglanir, node COKMEZ, devam eder.
**Anlami:** Bozuk veri, gecersiz imza, bellek-tasmasi denemeleri yapisal
olarak reddediliyor -> mimari bastan saglam kurulmus.

## DAYANIKLILIK OZETI (Adim 6)
- 6a Node restart toparlanmasi: DOGRULANDI (bkz #7)
- 6b Bozuk vertex dayanikligi: ZATEN KORUMALI (bu madde)
Gercek agin iki temel dayaniklilik gereksinimi de karsilanmis.

## KUZEY YILDIZI — "Kalkanli DEX" (uzun vadeli vizyon, 8 Haz 2026)
Bu bir HEDEF/yon notudur; bugun kod YOK. Yarin+ icin baslangic noktasi.

**Fikir:** Kullanicilari sahte/taklit token'lardan koruyan ("kalkanli") durust
bir DEX. Insanlar sahte coin korkusu olmadan, dogrulanmis token'larla islem yapsin.

**Kimlik = KONTRAT ADRESI** (isim/logo DEGIL — onlar taklit edilebilir). Ayni
isim+logo ama farkli adresli taklitler reddedilir. Kanonik adres kaydi tutulur.

**Listeleme sartlari (nesnel kriterler):**
1. Kilitli likidite (rug-pull'u zorlastirir)
2. Dogrulanmis kanonik kontrat adresi
3. Verify edilmis kaynak kod
4. Audit KAYDI (not: "audit var" kaydi tutulabilir; kalite GARANTI EDILEMEZ)

**DURUST ILKE:** Zincir KRITERLERI kontrol eder; "proje guvenli" GARANTISI VERMEZ.
Sorumluluk kullanicida. Amac "guvende hissettirmek" degil "gercegi gorebilmek".

**Adim sirasi:** token sistemi -> likidite havuzu -> kilit -> kanonik adres
kaydi + taklit reddi -> takas (DEX). Su an token sistemi bile YOK; yol uzun.

**Risk:** DEX = para + regulasyon (SPK/SEC) + hukuki sorumluluk. Ileride ekip +
denetim + hukuk SART.

**Kok tema:** Dogrulanabilir kayit. Koken: kurucu tir soforu — sahte belge
sezgisi = sahte token sezgisi, ayni kok.

---

## Canli cok-node kaniti (2026-06-09)

Kalkan + staking + slashing, gercek node'lar arasinda canli dogrulandi:

- **2 node:** stake -> gercek token (kabul) -> taklit token (red + slash).
  Sonuc her ikisinde tutarli: token=1, stake=0.
- **3 node:** ayni senaryo, peer=2, token=1, stake=0 (tutarli).
- **5 node:** TAM yakinsama — bes node da birebir ayni: vertex=7, peer=4,
  token=1, stake=0. GHOSTDAG yakinsamasi + mDNS kesfi 5 node'da kusursuz.

Kanit: uretici stake yatirir, gercek token kaydeder (kabul), sonra taklit
token dener -> kalkan reddeder (token sabit kalir) + uretici stake'i yakilir
(slashing). Bu sonuc tum node'lara yayilir ve tutarli kalir.

Sinir: tek VPS + mDNS (LAN). Buyuk olcek (yuzlerce/binlerce node) icin Kademlia
+ NAT + coklu makine gerekir — henuz yok, ileride.

---

## PERFORMANS DARBOGAZI (tespit: 2026-06-09) — COZULECEK

**Olcum:** tps_olcum testi (node.rs, #[ignore]). 10 vertex saf ingest = 0.265s
(~38 TPS, ~26ms/vertex). 1000+ vertex pratikte bitmiyor.

**Kok neden:** O(n^2) olceklenme. integrate_vertex (node.rs:265) her vertex
eklendiginde ghostdag.update() cagiriyor -> update_with_weight (ghostdag.rs:228)
-> her cagrida topological_order(graph) (consensus/mod.rs:144) TUM grafi bastan
sirala (Kahn, O(n)). Vertex islemenin kendisi incremental (hesaplanmisi atlar),
AMA topolojik sira her seferinde tum graf icin yeniden hesaplaniyor. N vertex ->
N cagri -> O(n^2).

**Cozum secenekleri (taze kafayla degerlendir):**
1. Incremental topological sort: sadece yeni (data'da olmayan) vertex'leri sirala.
2. update'i batch cagir (her vertex'te degil, periyodik).
3. topological_order yerine insert-sirasini kullan (append-only DAG'da gecerli).
Her secenek konsensus belirlenimciligini KORUMALI; 186 test + compute_with_weight
bit-bit esitligi guard'i ile dogrula.

**Onem:** "Hiz" iddiasi icin bu COZULMELI. Su an sistem kucuk olcekte (~yuzlerce
vertex) calisir, buyuk olcekte (hiz hedefi) calismaz. Durust durum: olcup bulundu.

---

## PERFORMANS — ILERLEME + ELENEN YAKLASIM (2026-06-10)

**COZULDU (commit 90673db):** topological_order darbogazi. update_with_weight
artik topological_order_eksik kullaniyor (zaten-hesaplanan vertex'leri siralamaya
koymaz). 10 vertex INGEST 0.265s -> 0.002s (~150x). 186 test gecti, bit-bit
determinizm korundu. KUCUK olcekte buyuk kazanc.

**ELENEN YAKLASIM (denendi, CALISMADI):** past() icin naive cache (past_cache:
BTreeMap<VertexId, BTreeSet<VertexId>>). Her vertex'in tum past-set'ini cache'le.
SONUC: testler gecti (dogru sonuc) AMA performansi KOTULESTIRDI — n=200 eski 34.5s,
cache ile n=300 bile 90s+ timeout. NEDEN: her compute'da dev BTreeSet klonlama
(acc.clone() + cache'e clone) + bellek O(n^2). Klonlama maliyeti, kacinilan
BFS'ten pahali. Geri alindi (perf-past-cache dali, commit'siz, silindi).

**KALAN DARBOGAZ:** compute_vertex_data icindeki past(graph,id)+past(graph,sp)
(2x O(n)) + 379/546'daki past (subset siralama). N vertex -> O(n^2). n=300+
pratikte bitmiyor.

**DOGRU COZUM (gelecek, BUYUK is):** Interval labeling / reachability index.
past-set SAKLAMADAN O(1) ata kontrolu. Selected-parent AGACINDA interval, diger
parent'lar icin hibrit (sinirli BFS + memoization). GhostdagData'ya DOKUNMA
(PartialEq/Eq + bit-bit test riski) — ayri reachability yapisinda tut. Cok
dikkatli + bol test gerektirir; tek oturmada degil. Kaspa-tarzi.

## PERF TESHIS (olcumle kanitli) — GERCEK DARBOGAZ: blue_set_in_view
- tps_olcum (zincir senaryosu) olcumu: her vertex'te blue_len +1 buyuyor (1,2,..n),
  mergeset_len HEP 0.
- SONUC: Darbogaz `compute_vertex_data`'daki `blue_set_in_view(data, sp)` (sp-zincirini
  yurur -> zincirde O(n)) + onu izleyen `for b in &blue { anticone_within_ri(...) }`
  baslangic dongusu (O(blue)=O(n)) -> her vertex O(n) -> TOPLAM O(n^2).
- reachability/interval isi (sp_tree_intervals, ReachIndex, is_ancestor_rec,
  mergeset_of, anticone_within_ri, A1 inkremental iv) DOGRU ve test edildi AMA bu
  zincir senaryosunda mergeset bos oldugu icin HIC IS YAPMIYOR -> bu testteki
  yavasligin sebebi DEGIL. Gercek DAG'da (paralel bloklar) bu altyapi gerekli kalir.
- DERS: ONCE OLC, sonra optimize et. eprintln olcumu en bastan yapilmaliydi.
- SONRAKI HEDEF: blue_set_in_view + anticone baslangic dongusunu inkremental/akilli
  yap. ONCE eprintln ile zaman olc, gercek pay-i dogrula, sonra dokun.

## PERF — GERCEK DAG (PARALEL) TESHISI (olcumle kanitli)
- Zincir senaryosu COZULDU (anticone/blue mergeset-bos kisa-devre + topo_eksik_hizli
  + interval kalan/2): n=10000 149s->20.8s.
- ANCAK paralel/diamond DAG (dolu mergeset) HALA cok yavas: 150 vertex = 66s.
- OLCUM (kat=10): is_ancestor_rec hizli_yol=7395 vs parent_yuruyus=1698355 (%99.6 YAVAS).
- KOK NEDEN: interval SADECE sp-agaci atalik O(1) verir. Paralel DAG'da anticone
  sorgulari sp-agaci DISI (paralel kardesler) -> interval tutmaz -> is_ancestor_rec
  parent yuruyusune duser (O(n)) -> anticone dongusu O(n^3).
- COZUM (buyuk, ayri): GERCEK reachability index = sp-agac interval + sp-agaci DISI
  atalar icin ek yapi (Kaspa'nin asil zor kismi). Cok-oturumluk, en hassas is.
- NOT: A2 (tam Kaspa modeli) GERCEK DAG icin gercekten gerekli oldugu artik
  OLCUMLE kanitli (zincir icin degildi). Taze kafayla, tugla tugla yapilacak.

## A2 REACHABILITY — TASARIM YOL HARITASI (oturum sezgileriyle)
Bugun kurulan (commit'li, test'li): bridge_lists (kopruler kucuk, O(n)),
is_ancestor_bridged (past ile birebir, paralel DAG'da dogru), interval budama
(koprusuz + koprulu dal suzme). OLCUM: budama anlik haliyle 1.64x tavan -> YETMEZ.
KOK: kopruyu anlik suzmek, koprunun arkasindaki yapiyi gormez.

SONRAKI ADIM = gercek covering set ("torba" modeli):
1. TORBALAR: her vertex, ait oldugu soylari interval-araliklari KUMESI olarak tutar
   (tek tek ata/kopru degil). "X atam mi" = X, torbalarimdan birinde mi (interval).
2. INKREMENTAL MIRAS: her vertex DOGARKEN (ingest aninda), sp-atasindan torba
   kumesini devralir + kendi sp-olmayan parent'larini (koprulerini) ekler.
   Sonradan tum-graf taramasi YOK. update_with_weight'in tek-tek ingest yapisina uyar.
3. SIKISTIRMA (ZORUNLU): her torun TUM kopruleri tasimaz -> yoksa "labirent"
   (kume siser, sorgu yavaslar, O(n^2) bellek = dunku cache tuzagi). Ortusen/kapsanan
   araliklari birlestir, sadece geçmisi gercekten genisleten kopruleri tut. Kume kucuk kalmali.
DOGRULAMA: her adim is_ancestor_bridged_past_ile_birebir testi (paralel DAG) ile
bit-bit; sonra budama_hiz_olcumu ile oran (hedef >>1.64x). Taze kafa ister (konsensus kalbi).

## A2 ENTEGRASYON TAMAM + SONRAKI DARBOGAZ (olcumle)
A2 torba gercek hatta bagli. PARALEL DAG 150 vertex: 66s -> 5.5s (12x). 197 test bit-bit.
KALAN DARBOGAZ (kat=30): anticone=5.65s (%95), mergeset=0.11s, blue=0.002s.
KOK: anticone_within_ri 10965 kez cagriliyor, her cagri ort 94 elemanlik blue'yu bastan
tariyor -> ~2M torba sorgusu. Torba sorguyu O(1) yapti ama CAGRI SAYISI hala O(n^2).
SONRAKI IGNE = ARTIMLI anticone ("yeni elemanin etkisini gor"): blue'ya yeni mavi
eklendiginde tum anticone'lari yeniden hesaplama, sadece yeni elemanin etkisini guncelle.
DIKKAT: blue dongu icinde buyuyor, renklendirme sirali bagimli -> bit-bit korunmali
(konsensus kalbi). Taze kafa + satir satir analiz + compute ile bit-bit dogrulama sart.

## ANTICONE DARBOGAZI — KASPA KAYNAGI (dogrulandi, web arastirmasi)
Kaspa (Wyborski, GHOSTDAG yaraticisi): k-cluster ARTIMLI korunur. Yeni blok tum
k-cluster'i hesaplamaz -> COGUNU selected-parent'tan MIRAS alir. Geri kalan, sp'nin
anticone'undan secilir; k-cluster oldugu icin EN FAZLA k yeni mavi eklenebilir.
"Her blok en fazla k ek blok izler: kendi mavi gecmisinde olup sp'nin mavi gecmisinde
OLMAYANLAR." (kullanici "miras vermek zorunda" sezgisi = DOGRU, Kaspa boyle yapiyor.)
Iki kisit (rusty-kaspa protocol.rs): (1) |anticone(cand) ∩ yeni_mavi_kume| <= k.
(2) her mavi b: |(anticone(b) ∩ yeni_mavi) ∪ {cand}| <= k.

BIZIM SORUN: anticone_size'i her compute'ta SIFIRDAN kuruyoruz (baslangic dongusu
10390 cagri). Kaspa SIFIRDAN kurmuyor -> sp'den miras + en fazla k yeni blok isler.
DOGRU IGNE: anticone_size + mavi kume sp'den MIRAS (kalici sakla, iv/torba gibi);
baslangic dongusunu KALDIR; sadece sp-anticone adaylarini isle. Bu, GHOSTDAG cekirdek
akisini Kaspa modeline yaklastirir = BUYUK + HASSAS degisiklik. Tugla tugla, bit-bit
(compute vs update) dogrulanmali. Kaynak: rusty-kaspa consensus/src/processes/ghostdag/protocol.rs

## ANTICONE — KASPA GERCEK KOD (DeepWiki 4.3, dogrulandi, satir no'lu)
Senin sezgilerin DOGRU cikti, Kaspa birebir boyle yapiyor:
1. SAKLAMA: GhostdagData'da blues_anticone_sizes saklanir (her blok, KENDI
   mergeset_blues'undaki her mavinin anticone boyutu). = "kasada sakla" + "her blok
   kendi payi" (Secenek 2). [stores/ghostdag.rs 22-30]
2. OKUMA (senin "f'ye yaz, geriye ara" sezgin): blue_anticone_size (protocol.rs
   230-244): bir mavinin boyutunu, sp-ZINCIRINDE GERIYE yuruyerek ilk kayitta bulur.
   TUM DAG degil, sadece sp-zinciri (kisa). Artimlı korundugu icin verimli.
3. EN KRITIK: Kaspa "baslangicta tum mavilerin anticone'unu sifirdan hesapla" YAPMAZ
   (bizim 10390-cagrilik baslangic dongumuz = YANLIS yaklasim). Onun yerine
   check_blue_candidate_with_chain_block (protocol.rs 168-226): her aday icin sp-ZINCIRINI
   yurur, her zincir blogunun mergeset_blues'uyla karsilastirir; aday'in atasi olan
   zincir blokuna ulasinca DURUR (kalan hepsi adayin gecmisinde). anticone(cand) ∩ blue
   k'yi asinca da durur. Yani tarama = sp-zinciri x k civarinda, n degil.
4. Iki kisit (protocol.rs 168-226): cand_anticone_size>k -> kirmizi; peer_anticone==k
   ise (aday eklenince k'yi asar) -> kirmizi.

YENIDEN TASARIM (sonraki oturum, BUYUK): bizim "blue_set_in_view + sifirdan baslangic
dongusu" mimarisini, Kaspa'nin "sp-zinciri yuruyusu + blues_anticone_sizes miras"
mimarisine tasimak. GhostdagData'ya blues_anticone_sizes alani gerekebilir (DIKKAT:
konsensus hash - ama Kaspa'da zaten var, bizim de determinizmi korur). Tugla tugla,
compute-vs-update bit-bit. Kaynak: rusty-kaspa consensus/src/processes/ghostdag/protocol.rs

## ANTICONE — KASPA GERCEK KOD (DeepWiki 4.3, dogrulandi)
Kullanici sezgileri DOGRU, Kaspa boyle yapiyor:
1. SAKLAMA: GhostdagData.blues_anticone_sizes (her blok KENDI mergeset_blues'undaki
   mavilerin anticone boyutu). = kasa + Secenek2. [stores/ghostdag.rs 22-30]
2. OKUMA: blue_anticone_size (protocol.rs 230-244): sp-ZINCIRINDE geriye yuruyerek ilk
   kayitta bulur. Tum DAG degil, sadece sp-zinciri. ("f'ye yaz, geri ara" sezgisi)
3. KRITIK: Kaspa baslangicta tum mavileri sifirdan hesaplamaz (bizim 10390-cagri
   baslangic dongusu YANLIS). check_blue_candidate_with_chain_block (protocol.rs 168-226):
   her aday icin sp-zincirini yurur, zincir blogunun mergeset_blues'uyla kiyaslar,
   adayin atasi olan zincir bloguna ulasinca DURUR. Tarama ~k, n degil.
4. Kisit: cand_anticone>k -> kirmizi; peer_anticone==k -> kirmizi.
YENIDEN TASARIM (buyuk): blue_set_in_view+baslangic dongusu -> sp-zinciri yuruyusu +
blues_anticone_sizes miras. Tugla tugla, compute-vs-update bit-bit. Mevcut yavas ama dogru.

## OLCEKLEME OLCUMU + YENI DARBOGAZ (coloring_kaspa sonrasi)
Paralel DAG INGEST (W=5): 150v=0.43s, 300v=3.46s, 600v=27.9s. -> hala O(n^2)+ (2x vertex
~8x sure). Anticone darbogazi cozuldu ama buyuk n'de YENI katman baskin.
TESHIS (600v, 29s): coloring=21.96s (%75), torba=6.92s (%24), iv_rebuild=0.12s.
- coloring_kaspa: kucuk n hizli (sp-zinciri kisa) ama buyuk n'de sp-zinciri UZAR; ayrica
  her peer icin boyut_bul sp-zincirini BASTAN tarar (ic ice zincir) -> O(n^2). COZUM:
  boyut_bul cache'leme / anticone_sizes'a dogrudan erisim (zinciri her seferinde tarama).
- torba_guncelle_tek sikistirma O(tabela^2); buyuk n'de tabela artiyor olabilir -> olc+cozulecek.
ACILIYET DUSUK: 150v=0.47s (pratik kullanim - testnet/demo/hackathon - yeterli). 600+ tek
seferde ingest gercek kullanimda nadir (bloklar zamanla gelir). Yeni katman = ayri oturum.
DURUM: en buyuk iki darbogaz (topological_order, anticone) COZULDU. 66s->0.47s @150v (~140x).

## BUYUK-N DARBOGAZ — TAM TESHIS (8 tur olcum, kaynak bulundu, kolay cozum YOK)
KAYNAK (kesin, olculdu @600v): coloring_kaspa'da ri.atalik cagri SAYISI = 451250.
Bunlarin %75'i toplam INGEST suresi (22s/29s). chain yuruyusu ort=38.8 blok/cand,
cand=2375. atalik tek tek O(1) ama 451k kez = yavas. n ile O(n^2) (chain uzar).
ELENEN COZUM YONLERI (hepsi olcumle curudu):
- boyut_bul DEGIL (BB cagri=0, paralel deseninde hic cagrilmiyor).
- "kisa yol/ziplama" YOK (Kaspa blue_anticone_size de zinciri yurur).
- K+1 kisayolu (Kaspa'da var, EKLENDI commit 61068ee, esik=k, 199 test birebir) ama
  paralel DAG'da nadir tetiklenir -> hiz ayni (29s). Yogun DAG'da faydali, kalsin.
- ONBELLEK (atalik cache): cagri-ici tekrar=1.00x (tek coloring_kaspa cagrisi icinde
  HIC tekrar yok). 5x tekrar CAGRILAR-ARASI -> global cache gerek -> n^2 BELLEK TUZAGI
  (+DoS). KOTU takas, YAPILMADI.
- chain yuruyusu DOGRU calisiyor (koke_gitti=0, kritik1_break=2375 hepsi; israf yok).
  38.8 YAPISAL: cand'in atasi sp-zincirinde ort 38.8 blok geride (paralel DAG geometrisi).
GERIYE KALAN TEK YON (zor, somut fikir YOK henuz): chain yuruyusunu YAPISAL kisaltmak
(38.8 -> az). Muhtemelen interval-tabanli atlama (cand'in atasinin oldugu bolgeye iv ile
zipla, blok blok yurume) VEYA farkli veri yapisi. YENI DERIN TASARIM oturumu ister.
ONCE: Kaspa'nin gercek ort_chain'i nedir (bizim 38.8 anormal mi?) - arastir.
ACILIYET DUSUK: 150v=0.47s pratik icin fazlasiyla yeterli. 600+ tek-batch ingest nadir.

## BUYUK-N — GUVENLI KISMI KOSUL BULUNDU (atlama icin zemin)
HIPOTEZ: cand_anticone bos ise chain'e girme, direkt mavi (atla). Bos oldugunu O(1)
bilmek gerek. ARANIYOR: "bos" sinyali.
KOSUL TEST 1 (parent sp'nin atasi mi): cand'in TUM parent'lari sp'nin atasi/kendisi mi?
  Olcum @600v: bos&kosul=515, bos_kosulsuz=1860, DOLU_KOSUL=0.
  -> GUVENLI (dolu_kosul=0: kosul tutunca anticone KESIN bos, yanlis mavi YOK -> konsensus
     bozulmaz). Ama KISMI: bos'un %22'sini yakaliyor (515/2375), %78 kacak.
KOSUL TEST 2 (sp VEYA onceki mavi cand'larin atasi): AYNI sonuc (515/1860/0) - genisletme
  HIC fark etmedi. -> bos'un %78'i, onceki-mavi-cand ata-torun iliskisinden DEGIL, baska
  sebepten. (Kullanici sezgisi "digerleri oncekilerden miras almali" bu desende tutmadi.)
DURUM: guvenli kismi kosul (TEST 1) elde -> kodlanip %22 chain atlanabilir (dolu_kosul=0
oldugu icin bit-bit guvenli; kalan %78 icin chain yine yurur). Ama TAM cozum, bos'un %78'inin
NEDEN bos oldugunu bulmak (yeni bakis/taze kafa). bos-anticone'un yapisal nedeni paralel
DAG sikiligindan ama O(1) tespit formulu henuz YOK.
SONRAKI: ya (a) guvenli kismi koÅul TEST 1'i kodla (kismi kazanc, bit-bit), ya (b) bos'un
%78'inin nedenini bul (mergeset yapisi? topo-sira? sp-tree konumu?).

## BUYUK-N — FINALITY-PRUNING COZER (olcumle KANITLANDI) + GUVENLIK SINIRI
OLCUM @600v: chain'e giren 1860 cand'in %100'u (1860/1860) cipanin ALTINA iniyor.
chain ortalama derinlik = 446400/1860 = ~240 blue_score. Pruning derinligi 6.
-> chain, cipanin ~40 KAT altina iniyor. chain'i cipada kessek 240->6 = ~40x kisa chain.
Gemini madde 2 (finality-pruning) BU DARBOGAZI COZER - olcum dogruladi. Algoritmik, gercek.
finality.rs zaten MEVCUT+denetimli (final_block, pruning_anchor, FinalityState monoton).

*** KRITIK GUVENLIK BULGUSU ***
DEFAULT_PRUNING_DEPTH=6 < DEFAULT_K(~18). Anticone k=18'e kadar uzayabilir; cipa derinlik
6'da ise k-CLUSTER PENCERESININ ICINDE kalir. chain'i derinlik-6 cipada kesmek, k
penceresindeki peer'leri KACIRABILIR -> anticone eksik -> KONSENSUS BOZULUR.
=> chain kesme derinligi pruning_depth(6) DEGIL, k'dan BUYUK olmali (derinlik > k).
Kesinlesmis blok teorik olarak yeni blogun anticone'una giremez AMA bunu k=18 ile
dogrulamak SART. Guvenli kesme esigi: en az k+guvenlik_payi (orn 2k veya pruning_depth'i
k'dan buyuk sec). Bu kanitlanmadan KOD YOK.
SONRAKI: (1) guvenli kesme derinligini belirle (>k), (2) "cipanin altinda cand'in
anticone'una peer giremez" guvenlik kanit/test (cok desen), (3) coloring_kaspa chain'i
o derinlikte DURDUR (sp-zinciri reference cand'in kendi sp'si, GLOBAL tip degil - cand
gecmiste olabilir, dikkat). Buyuk+riskli konsensus isi, taze+uzun oturum.

## MERGE_DEPTH — KASPA'NIN GARANTILI YOLU (kurmadan once tam ogren)
Kaspa "bounded merge depth" kurali kullanir: bir blogun mergeset'i, selected-parent-chain'de
merge_depth'ten (test-pruning'de 128 ~6k) daha derindeki bloklari merge EDEMEZ; o kadar
derindeki bloklar zorla kirmizi/dislanir. Bu, chain/mergeset yuruyusunu SABIT derinlige
baglar -> O(n^2) kirilir. BIZE UYARLAMA: coloring_kaspa chain'i merge_depth derinliginde
DURDURulabilir (k DEGIL ~6k - Kaspa k'da kesmiyor, 6k'da kesiyor -> k tek basina GUVENSIZ).
EKSIK (kurmadan once SART): Kaspa mergeset.rs/merge_depth kaynagini SATIR SATIR oku - kodda
TAM nasil kesiyor (mergeset BFS'i mi sinirliyor, ayri bounded_merge_depth_root mu, kirmizi
zorlama mi?). Sadece deger (128) biliniyor, MEKANIZMA degil. Yarim anlasilmis kesme =
gizli risk, garanti DEGIL.
KURULUS PLANI (taze oturum): (1) Kaspa merge_depth kaynak oku, (2) bizim parametre sec
(~6k=108, guvenli), (3) coloring_kaspa'da bit-bit dogrulanan kesme (eski yuruyusle ayni
sonuc mu - coloring_kaspa_birebir gibi), (4) cok-desen test, (5) entegre. KONSENSUS KALBI -
acele YOK, denetim ideal.

## *** KESIN BULGU: KASPA DA CHAIN'I SINIRSIZ YURUR (protocol.rs okundu) ***
Kaspa check_blue_candidate (protocol.rs): chain'i selected-parent zincirinde, Blue (cand'in
atasina ulasti) ya da Red donene kadar SINIRSIZ yurur. HICBIR derinlik/merge_depth kesimi YOK.
merge_depth BASKA amac icin (mergeset uyeligi), coloring chain'ini KESMIYOR.
=> Bizim coloring_kaspa BIREBIR Kaspa gibi. Bizim 38.8 adim "darbogaz" Kaspa'da da AYNEN olurdu.
Hata/eksik DEGIL - sentetik dar-paralel 600v deseninin dogal sonucu.
Kaspa neden yavaslamaz: (1) is_dag_ancestor_of gercekten O(1) (reachability interval - bizim
torba/interval gibi), (2) GERCEK DAG'da cand'in atasi YAKIN (mergeset kucuk) -> chain KISA biter.
SONUC: chain'i merge_depth ile KESMEK = Kaspa'dan SAPMA = konsensus farki riski. "Garanti"
istiyorsak Kaspa ne yapiyorsa: chain'i KESME, atalik'i O(1) tut (torba/interval - YAPILDI).
Buyuk-n "darbogazi" sadece sentetik tek-batch dar-paralel testte; gercek kullanimda (bloklar
zamanla, mergeset kucuk, chain kisa) PROBLEM YOK. 150v=0.47s zaten yeterli.
KARAR: merge_depth-kesme YOLU TERK (Kaspa'dan sapardik). Mevcut coloring_kaspa Kaspa-dogru.
Binary lifting (ata_bul_up) izole/dogrulanmis kaldi - ileride atalik O(1) zaten yetiyorsa
gerek olmayabilir. perf-interval kazanimlari (anticone 140x) GECERLI ve Kaspa-uyumlu.

## *** YENI GERCEK DARBOGAZ: topological_order_eksik_hizli O(n^2) (saf zincirde bile!) ***
OLCUM: saf zincir (W=1, EN gercekci desen): 1000 blok=0.25s ama 10000 blok=22s (512 TPS).
10x blok -> ~80x sure = O(n^2). coloring DEGIL (saf zincirde mergeset bos, coloring calismaz).
ZAMAN PROFILI @10000: topo=14.6s (%65!), cvd=0.06, iv=0.08, torba=0.03, up=0.17. SUCLU: topo.
KAYNAK: consensus/mod.rs:200 topological_order_eksik_hizli her ingest'te graph.ids() ile TUM
grafi (n) tarayip "mevcut'ta yok mu" filtreliyor -> her ingest O(n), n ingest = O(n^2).
9999 vertex'i bosuna tarayip 1 yeni buluyor.
KASPA YOLU (DeepWiki 4.2 Block Processing Pipeline okundu): OLAY-GUDUMLU. validate_and_insert_block
TEK blok girer, BlockTaskDependencyManager parent'lar hazir mi bakar, DbStatusesStore her blogun
durumunu (StatusHeaderOnly/UTXOValid) tutar. TUM GRAFI TARAMAZ - gelen blogu dogrudan isler.
COZUM (Kaspa-dogru): node.rs zaten ingest ettigi vertex'i biliyor. update'e "tum graf, eksik ara"
yerine, gelen vertex'i DOGRUDAN ver/isle (parent'lari hazirsa). Orphan/cascade dikkatli ele alin.
KONSENSUS MANTIGINA DOKUNMAZ - sadece "hangi vertex islenecek" akisi. coloring/GhostdagData aynen kalir.
ETKI: saf zincir 22s -> muhtemelen ~1s (topo O(n^2) -> O(n)). GERCEK kazanc, gercek kullanim deseni.
ONCELIK: YUKSEK - bu coloring "darbogazindan" daha gercek (saf zincirde, en gercekci desende var).
NOT: bu duzeltme update/ingest akisi; taze kafa + orphan/cascade testleri + 200-test bit-bit.

## KALAN ARTIK: interval rebuild O(n^2) (update_one sonrasi, %41)
update_one ile topo O(n^2) cozuldu (10000 blok 22s->2.8s, 6.6x). KALAN super-lineer kaynak:
assign_interval_incremental -> sp_tree_intervals_gapped REBUILD. Profil @10000: iv=1.15s (%41),
digerleri kucuk (up=0.09, cvd=0.02, torba=0.01).
NEDEN: saf zincirde her blok sp'nin kalan boslugunun YARISINI alir (SLICE yerine, akilli ama)
-> 2^60 boSluk ~60 adimda usttel tukenir -> her ~60 blokta REBUILD. 10000/60=166 rebuild
(olculen rebuild_say=166). Her rebuild O(n) (tum iv bastan) -> 166*O(n) = super-lineer.
COZUM ADAYLARI (taze kafa, KONSENSUS-KRITIK - atalik temeli, dikkat):
  (a) rebuild'i amortize/artimli yap (tum agac degil, etkilenen alt-agac)
  (b) bosluk dagitimini zincirde tukenmeyecek sekilde yeniden tasarla
  (c) interval yerine farkli reachability (Kaspa reachability/inquirer.rs incele)
DURUM: ACIL DEGIL. 3548 TPS zaten iyi. "Calismiyor" degil "daha hizli olabilir".
Bit-bit + cok-desen test SART (atalik bozulursa sessiz konsensus hatasi).

## AG KATMANI (lsc-net) — COK DUGUM SENKRONIZASYON TESTI (BASARILI)

**Tarih:** bu oturum. **Bulgu:** lsc-net P2P agi calisiyor; iki dugum birbirini bulup senkronize ediyor.

**Mevcut durum (lsc-net ~1000 satir, libp2p tabanli):**
- mDNS otomatik kesif: dugumler birbirini buluyor (ayni makine/ag).
- 3 mod: ana uretici (genesis+vertex), `produce` (genesis yok, baglanir+paralel vertex), `listen` (sadece dinler).
- Pull-sync: yeni dugum mevcut gecmisi ceker (genesis + tum vertex).
- Canli yayin (gossipsub): uretici yeni vertex uretince dinleyici aninda ingest eder.
- Kalkan/token/stake sistemi ag uzerinde calisiyor (token kaydi, slashing).
- Kalicilik: porta gore otomatik data + kimlik dosyasi.

**KRITIK DERS — dinleme adresi / ilan adresi uyusmazligi:**
- localhost (`/ip4/127.0.0.1/tcp/N`) dinletince mDNS dugumu YANLIS IP'lerde (Docker 172.17.0.1, public 45.x) ilan etti -> kesif oldu AMA baglanti KURULMADI (peer=0, NoPeersSubscribedToTopic).
- COZUM: `/ip4/0.0.0.0/tcp/N` (tum arayuzler) dinlet + gerekirse manuel dial. O zaman peer=1, pull-sync + canli yayin calisti.
- DOGRULANDI: dugum-2 (listen) dugum-1'in genesis'ini cekti + uretilen her vertex'i aninda ingest etti (vertex 1->6).

**SIRADAKI (ag cephesi):**
- 3+ dugum + paralel uretim (gercek W>1 ag DAG'i).
- GERCEK PROPAGATION GECIKMESI olcumu (vertex uretimi -> diger dugumde ingest, ms). Bu, BPS/k parametre ayarinin TEMELI ("neye gore ayarlanacak" sorusunun cevabi). Su an localhost; gercek cografi gecikme icin farkli sunucular gerekir.

---

## HIZ OLCUMU (14 Haziran 2026, gercek sayilar)

### 1. Saf motor hizi (agsiz, release/optimize, tek dugum)
`cargo test --release tps_olcum -- --ignored` ile olculdu:
- **INGEST (saf islem isleme): ~5912 TPS** (saniyede ~5900 vertex motora islenir)
- Uretim (imza+hash): ~25097 vertex/s
- Paralel ingest (kat=10, W=5): ~1757 TPS

### 2. Uctan uca gecikme (gercek kosul, HTTPS uzerinden)
`https://aidag-chain.com/rpc` uzerinden, owner faucet islemi, 20 olcum (isinma sonrasi):
- En hizli : 199 ms
- Ortanca  : 228 ms
- Ortalama : 233 ms
- En yavas : 295 ms
- (Ilk-baglanti anomalisi: ilk istek bir kez 135 s surdu — TLS/DNS isinmasi, olcume katilmadi.)

### DURUST CERCEVE (neyin olculdugu, sinirlar)
- Saf motor TPS'i **tek dugum, tek makine, agsiz** olcumdur. Gercek dagitik agda
  (gossip yayilimi, cok dugum) efektif TPS DUSER — her zincirde boyledir.
- Uctan uca ~230 ms'nin cogu **ag + TLS + Nginx + 2 RPC cagrisi** (tips+submit);
  dugumun saf isleme suresi bunun cok altinda (motor TPS'inden gorulur).
- Bu sayilar "isleme" (ingest) hizidir; KESINLIK/finality ayri bir konudur ve
  su an erken-asama test ortaminda basittir.
- Olculmus, abartisiz sayilar: "tek dugum ~5900 TPS isleme; uctan uca kullanici
  islemi ~230 ms (HTTPS dahil)". Olcum kosulu degisirse sayilar degisir.

---

## KALICILIK DOGRULAMASI (14 Haziran 2026)

### Onarilan bug
RPC `/submit`'ten gelen vertex'ler (faucet/transfer) diske YAZILMIYORDU
(submit_rx kanali sadece gossip publish ediyordu). Ayrica orphan vertex'ler
ve sirasiz yukleme MAX_ORPHANS=1024 limitine takiliyordu. Sonuc: reboot'ta
"parent zinciri kopuk", bakiyeler sifirlaniyordu.

### Onarim (commit f8b3fcb)
1. submit_rx kanalina `append_vertex` eklendi (RPC vertex'i diske yazilir).
2. Buffered/orphan vertex de diske yazilir (gecerli vertex, parent'i sonra gelir).
3. Diskten yukleme TOPOLOJIK sirali (parent-once) — orphan havuzuna hic dusmez.

### Test sonuclari
- **Kucuk olcek:** 3 faucet (5000/3000/1500) -> reboot -> 3/3 tam geri geldi,
  bekleyen_orphan=0.
- **Buyuk olcek:** 100 faucet (her biri 100 AIDAG) -> reboot ->
  "Diskten 118 vertex yuklendi, bekleyen_orphan=0", 100/100 adres tam,
  toplam 10000/10000 AIDAG. KAYIP SIFIR.

### Notlar / bilinen sinirlar
- Domain (HTTPS) uzerinden cok hizli art-arda sorgu RATE-LIMIT'e takiliyor
  (403 Forbidden) — bu veri kaybi DEGIL, sunucu DDoS korumasi. Localhost'tan
  (proxy yok) tum sorgular dogru dondu. Toplu sorgu gerekiyorsa yavaslat.
- Disk yazma hala 5 ayri noktada (lib.rs: gossip, sync, genesis, submit_rx, vb).
  Calisiyor ama DAGINIK. TEMIZ COZUM (ileride): yazmayi tek merkezi noktaya
  (engine-ici, her basarili ingest'te) toplamak. Snapshot ile birlikte yapilacak.
- Snapshot/pruning HENUZ YOK. Su an tum zincir diske append-only yaziliyor +
  acilista tamami yeniden yukleniyor. Bu olcekte (binler) sorunsuz; zincir
  cok buyuyunce (Kaspa modeli) snapshot+pruning gerekecek — temiz zemin hazir.


---

## OLCEK TESHISI: GHOSTDAG O(n^2) DARBOGAZI (24 Haziran 2026, kesin teshis)

### Olculen davranis (guncel binary, AVM dahil)
olcek_egrisi testi (node.rs, #[ignore], lineer zincir W=1, saf ingest):
- 10.000 vertex : 2.88s  (3473 TPS)
- 50.000 vertex : 63.4s  (788 TPS)
- 100.000 vertex: 297s   (336 TPS)
Kanit: 10k->50k vertex 5x artti, sure 22x (~5^2); 50k->100k vertex 2x, sure
4.7x (~2^2). KESIN O(n^2). (Kucuk DAG'da 5987 TPS, ama DAG buyudukce coker.)

### Kok sebep (kod seviyesinde bulundu)
Yer: consensus/ghostdag.rs, assign_interval_incremental -> sp_tree_intervals_gapped.
Mekanizma: sp-agac interval atamasinda her cocuk, ebeveynin KALAN boslugunun
YARISINI alir (pay = kalan/2). Lineer/derin zincirde bosluk ustel daralir ->
~60 vertex sonra tukenir -> sp_tree_intervals_gapped TUM DAG'i yeniden numaralar
(O(n)). Olculdu: 10k vertex'te 166 rebuild, tam 60'ar arayla. Toplam: (n/60)*O(n)
= O(n^2). Rebuild fonksiyonunun kendisi DOGRU (2^60 genis aralik verir); sorun
rebuild'in COK SIK tetiklenmesi.

### KRITIK BAGLAM: bu darbogaz hangi isi etkiler?
- Belge DOGRULAMA (ucuncu taraf sorgusu, AIDAG'in asil isi): record_registry
  HashMap O(1). DAG buyumesinden ETKILENMEZ. Milyonlarca belgede bile anlik.
- Bakiye/kurum/nonce sorgulari: ayni sekilde O(1)/O(log n) registry lookup.
- O(n^2) YALNIZCA yeni vertex EKLEME (ingest) yolunda. Yani yuksek YAZMA
  yukunde (saniyede binlerce surekli yeni kayit) sorun olur. Belge dogrulama
  senaryosunda yazma yuku dusuktur (kurumlar gunde sinirli belge kaydeder) ->
  bu darbogaz mevcut/yakin kullanim icin ACIL DEGIL.

### Cozum yonu (gelecek, dikkatli/cok-adimli is)
- Rebuild'siz artimli numaralama: yeni vertex tum DAG'i degil sadece kendi
  numarasini almali (cocuk atasina kadar gitmeden atasini bilmeli).
- PASIF up tablosu (binary lifting, ghostdag.rs ~satir 200) aktif edilmeli:
  atalik sorgusu O(log n) sicrama, tam-tarama yok.
- Alternatif: rebuild O(n) yerine O(degisen alt-agac) yapilmali (Kaspa benzeri
  amortized reindex), ama kendi kullanimımıza ozel daha basit cozum aranabilir.
- KIRMIZI CIZGI: interval degerleri konsensusu ETKILEMEZ (sadece atalik
  hizlandirma), ama atalik sorgusunun DOGRULUGU korunmali -> her degisiklik
  267 birim test + atalik dogrulugu ile kanitlanmali.


---

## O(n^2) COZUM TASARIMI (24 Haziran 2026, cozum yonu netlesti)

### Kok sebep (kanitlandi)
assign_interval_incremental: her cocuk bosluğun YARISINI alir (pay=kalan/2).
Lineer/derin zincirde ustel daralma -> ~60 vertex'te tukenme -> tum DAG rebuild
(sp_tree_intervals_gapped, O(n)). 10k vertex'te 166 rebuild, tam 60'ar arayla.
Toplam O(n^2): 10k=3473 TPS, 50k=788, 100k=336.

### Denenenler (hepsi geri alindi)
- ODA (sabit 2^20 pay): kardes dalinda yine daralma, 22'ser rebuild. Kotu.
- Aralik genisletme: yanlis yer, etki yok.
- "Genis arazi" (pay=kalan-1): rebuild=0 oldu, TPS 3473->5769 (KAZANIM!), AMA
  3 test kirildi (incremental_equals_full). Sebep: cocuk araziyi neredeyse
  tumuyle alinca KARDES'e yer kalmadi -> kardes araliklari CAKISTI -> atalik
  sorgusu (sa<=sb && eb<=ea) yanlis -> blue_score yanlis (6 yerine 7) -> blue
  set bozuldu. ONEMLI DERS: interval konsensusu dolayli etkiler; atalik
  dogrulugu BOZULURSA blue set bozulur. Hiz icin dogruluktan TAVIZ YOK.

### COZUM (net, uygulanacak)
Strateji: "yaygin durumu hizli, nadir durumu dogru yap."
Cogu vertex AZ cocuklu (lineer baskin; cok-cocuk/genis catallanma NADIR).
1. GENIS BASLA: cocuga comert pay ver (rebuild cogu zaman hic olmaz -> hizli).
2. CAKISMA YASAGI: kardes araliklari asla ortusmemeli, hepsi ata icinde kalmali.
   ("Genis arazi"nin bozdugu yer buydu — bu sefer kontrol edilecek.)
3. LOKAL REBUILD: yer bitince TUM DAG degil, SADECE o atanin alt-agacini
   genis araziyle yeniden numarala (O(alt-agac), O(n) degil). Nadir -> ucuz.

### Uygulama icin gerekenler (taze oturum, dikkatli, her adim 267 test + atalik)
- Kalici children_sp (struct alani, her vertex eklenince incremental guncelle)
- Alt-agac yeniden numaralama fn (sp_tree_intervals_gapped'in lokal hali)
- Kapsama kuralinin (sa<=sb && eb<=ea) hem ic hem dis korunma kaniti
- Torba + binary lifting (up) etkilenirse guncelle
KIRMIZI CIZGI: her degisiklik 267 birim test + atalik dogrulugu ile kanitlanir;
bozulursa ANINDA geri alinir. Hiz, dogruluktan once GELMEZ.


---

## O(n^2) COZULDU - OLCUM KANITI (24 Haziran 2026)

Cozum uygulandi ve olculdu (genis arazi + lokal rebuild guvenlik agi):
- children_sp (kalici sp-cocuk listesi) + subtree_reindex (alt-agac esit yeniden
  numarala) + lokal_rebuild_dene (bosluk dolunca once lokal, yetmezse tam rebuild)
- assign_interval: kalan/2 -> kalan-1 (genis), cakisma olursa lokal rebuild duzeltir

OLCUM (lineer zincir, saf ingest, tek makine, ag/imza/disk HARIC):
  vertex   | ESKI (kalan/2) | YENI (genis+lokal)
  10.000   | 3473 TPS       | 5291 TPS
  50.000   |  788 TPS       | 4427 TPS  (5.6x)
  100.000  |  336 TPS       | 4681 TPS  (~14x)
  1.000.000| (cokerdi)      | 3535 TPS  (283s)

SONUC: O(n^2) coktu. 10x veri (100k->1M) icin TPS sadece 4681->3535 (1.32x dususe,
logaritmik) -> O(n^2) DEGIL. Sistem milyonlarda OLCEKLENIYOR. 267 birim test
YESIL (atalik/blue set/total_order korundu; differential incremental=full gecti).

DURUST SINIR (gercekle ortusmeli):
- Bu TPS SAF INGEST'tir (GHOSTDAG renklendirme + interval). Gercek node TPS'i
  imza dogrulama + P2P + disk + mempool ile DAHA DUSUK olur. Henuz uctan-uca
  gercek-node TPS olculmedi -> baska zincirlerle (Kaspa/ETH/Solana) KIYAS icin
  o olcum sart; mevcut rakam onlarla DOGRUDAN kiyaslanamaz (elma-armut).
- TPS tam sabit degil, hafif logaritmik dususlu (BTreeMap log n + ara sira lokal
  rebuild). O(n^2) degil ama O(1) de degil; kabul edilebilir olceklenir davranis.
