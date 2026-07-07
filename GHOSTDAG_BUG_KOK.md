# GHOSTDAG blue_score Bug - COZULDU (2026-07-07)

## DURUM: COZULDU. Commit: bdd2e6f. 288 test yesil. TPS 3988->4286.

## BELIRTI (idi)
Artimli GHOSTDAG (update_one/update), referans (compute_default) ile YOGUN PARALEL
topolojide blue_score'u eksik hesapliyordu (bir mavi kaciyordu).
Test: dogrula_test kat=5 w=3. selected_parent dogruydu; fark mergeset_blues'ta.

## KOK SEBEP
Interval-tabanli atalik (is_ancestor_rec), selected-parent agacina dayaniyor. DAG'da bir
duguma birden cok yoldan ulasilir. sp-agaci interval semasi, yogun paralel topolojide iki
ayri dala CAKISAN aralik verebiliyor (v=814a [0,4.27e16], sp 55ed [0,2.13e16], ikisi de
0-tabanli -> 814a 55ed'i kapsiyor -> is_ancestor(814a,55ed)=YANLIS true). Interval
"kapsiyor->atasi" kisayolu bu yanlis-pozitife koru koru guveniyordu -> 814a "past(sp)'de"
sanilip mergeset'ten eleniyordu -> mavi kaciyordu.

## COZUM (AIDAG'a ozgu)
mergeset_of'un budama KAPISINDA: interval "atasi" dese bile DUR, saf recursive parent-
yuruyusuyle (saf_atalik_rec, iv KULLANMAZ) DOGRULA. Interval yanlis-pozitif verebilir,
saf recursive her zaman dogru. Sadece EVET'ler denetlenir (interval-HAYIR guvenli).
Fikir: "atalik kontrolunu kim yapiyorsa o dursun, gozuyle gormedigine evet demesin,
tek kapida dogrulasin." Kod: ReachIndex::saf_atalik_rec + mergeset_of kosulu.

## ONEMLI: KASPA COZUMLERI AIDAG'DA ISE YARAMADI
Bug reachability'de oldugu icin, Kaspa'nin COLORING cozumleri (K+1 siniri,
blues_anticone_sizes doldurma, add_blue peer +1) AIDAG'da bu buga DOKUNMADI - hepsi
test edildi, hicbiri cozmedi. Kaspa'nin interval'i cakismaz cunku AYRI reachability
modulu var (future covering). AIDAG torba/interval reachability kullanir -> farkli mimari
-> farkli cozum. Cozum Kaspa'dan degil, AIDAG'a ozgu kapi-dogrulamasindan geldi.

## SONUC (kanitla)
dogrula_test tum senaryolar (3/2,5/3,8/4,4/6,10/2,6/5) fark=0. 288 test yesil.
TPS 1M: 3988 -> 4286 (yukseldi). Edge-case (yogun paralel) artik dogru; normal
trafik zaten dogruydu, 10M benchmark gecerli.

## ELENEN YOLLAR (COZUM DEGILDI)
coloring kararlari (atla, cand_anticone), anticone_sizes doldurma (+%6 yavas), Kaspa K+1,
out doldurma, budama gevsetme, lokal_rebuild kapatma (fark 3->7 kotu). Cozum reachability
kapisindaki dogrulamaydi.
