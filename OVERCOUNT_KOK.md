# GHOSTDAG OVERCOUNT bug - kok bulundu, cozum bekliyor (2026-07-07)

## DURUM
Undercount COZULDU (commit bdd2e6f/5dc280b, pushed). Bu AYRI bir overcount bug'i.
Fuzz testi (fuzz_dogrula) buldu: tur=5, v=7821. Deterministik.

## BELIRTI
update_one, compute_default'a gore bir blogu FAZLA mavi sayiyor (overcount).
fuzz_dogrula FUZZ FARK tur=5 n=46 v=7821: inc_bs=34 ref_bs=33.
zincir_goster: 46 vertex'ten SADECE 7821 FARK. 7821 mergeset={69ef}, sp=90be.
inc 69ef'i MAVI, ref KIRMIZI sayiyor.

## KOK SEBEP (kanitla)
Iki renklendirme yolu, ayni blue kumesinde FARKLI anticone sayiyor:
- REF (compute_default, None dali): blue_set_in_view + anticone_within_ri (DOGRUDAN).
  69ef icin: anticone_within_ri(69ef, blue_view_33) = 23 (>18=k) -> KONTROL 1'den KIRMIZI. DOGRU.
- INC (coloring_kaspa, update yolu): chain-dongusu ile sp-zinciri boyunca peer toplami (DOLAYLI).
  69ef icin: chain-toplami = 14 (<18) -> MAVI. YANLIS.
Ikisi de AYNI blue_view (33 blok, birebir - kanitlandi). Ama chain-dongusu 9 anticone uyesini KACIRIYOR.
Chain-dongusu sp-zincirini gezip her chain-block'un mergeset_blues'undan peer topluyor;
blue_view'deki bazi anticone uyeleri bu sp-zinciri peer'lerinde gorunmuyor.

## MUHTEMEL COZUM
coloring_kaspa'da dolaylı chain-toplami yerine DOGRUDAN anticone_within_ri kullan.
Ikisi ayni blue_view'e sahip oldugu icin sonuc birebir olur (INC de 23 sayar -> 69ef kirmizi).
Hiz etkisi olcul melir (chain-toplami optimizasyondu). Once dogruluk (fuzz gecsin), sonra hiz.

## ELENEN (bu overcount icin - TEKRAR DENEME)
- atla kisayolu kaldirma -> cozmedi (69ef atla'dan geciyordu ama sorun degildi)
- peer kontrolu == -> >= -> cozmedi (peer_sz k'yi asmiyor)
- KRITIK 1 chain-break'e saf_atalik bekci -> cozmedi
- peer eleme kapisina saf_atalik bekci -> cozmedi
- new_block gecisi (mergeset_blues peer) -> cozmedi (7821 mergeset tek blok)

## ARACLAR (git checkout ile silindi, yeniden ekle)
- fuzz_dogrula: rastgele DAG, update_one vs compute_default, FUZZ_TUR env.
- zincir_goster: tur=5 DAG'ini vertex vertex OK/FARK dokuyor.

## COZULDU (2026-07-08 gece) — mavi_boncuk
Overcount + undercount COZULDU. coloring_kaspa (chain-dongusu + atla kisayolu, invariant
kirik) yerine 'mavi_boncuk': AIDAG'in kendi renklendirme sistemi. Anticone'u blue_view'de
SAF-dogrulanmis atalik (saf_atalik_rec) ile DOGRUDAN sayar. Deney kaniti: torba VE
coloring_kaspa chain-dongusu ikisi de payliydi; mavi_boncuk saf ile ikisini de asar.
SONUC: dogrula_test PASSED, FUZZ_TUR=2000 FUZZ OK, 289 test yesil.

## KALAN: HIZ (sabah devam)
mavi_boncuk saf_atalik ile O(blue^2) (baslangic dongusu) -> uretim icin YAVAS (10k'da takildi).
COZUM YONU (NOTLAR_BILINEN_SINIRLAR.md satir 270-312): blues_anticone_sizes MIRAS mimarisi.
Baslangic O(blue^2) dongusunu kaldir; anticone_size'i sp'den KALICI+TUTARLI miras al.
DIKKAT: miras tutarli olmali - out'a sadece cand degil, komsu +1 guncellemeleri de yaz;
update_one anticone_sizes'a beslesin; sonraki vertex miras alsin. Basit miras denemesi
(sadece boyut sp'den) tur=9'da yeni overcount actı (tutarsizlik) -> tam miras zinciri gerekli.
