# GHOSTDAG blue_score bug - kok sebep (2026-07-07)

BELIRTI: update_one vs compute_default, yogun paralel topolojide blue_score eksik.
Test: kat=5 w=3, v=814a mergeset'e girmiyor.

KOK: Interval atalik sp-AGACINA dayaniyor ama DAG cok-parent. sp-agaci interval semasi
(incremental + gapped rebuild) yogun paralelde iki dala CAKISAN aralik veriyor.
814a [0,4.27e16] ve 55ed [0,2.13e16] ikisi de 0-tabanli -> 814a 55ed'i kapsiyor ->
is_ancestor(814a,55ed)=true YANLIS -> mavi kaciyor. subtree_reindex komsu dalla cakisiyor.

ELENEN (testle): coloring kararlari, anticone_sizes(+%6 yavas), K+1, out doldurma,
budama gevsetme, lokal_rebuild kapatma (fark 3->7 kotu). Hicbiri cozmedi.

KESIN: normal/seyrek/eszamanli trafik DOGRU. Node guvende, 10M benchmark gecerli. Edge-case.
Coloring dogru; bug REACHABILITY'de.

DUZELTME: Kaspa reachability modulu (interval+reindex+covering) referans. /tmp/kp.rs
