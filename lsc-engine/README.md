# lsc-engine

LSC (Light Soulware Chain) - AIDAG Chain'in DAG tabanli L1 motoru.

Durum: Cekirdek konsensus katmanlari tamamlandi (101 test gecer).
Ag katmani (lsc-net) Asama 1: P2P iskelet (peer identity + ping).

## Calistir

  cd ~/aidag-lsc
  cargo build
  cargo test

## Dogrulama Politikasi (Sahte Yok)

Her katman, ureten ve denetleyenin AYRI oldugu bir surecten gecer:
1. Ureten: bir AI (Claude / Grok / Gemini)
2. Denetleyen: farkli bir AI + insan (capraz kontrol)
3. Test: sunucuda cargo test ile dogrulama

Denetimden ve testten gecmeden (GO denmeden) sonraki katmana gecilmez.
Mainnet oncesi profesyonel (insan) audit zorunludur.

## Wire Format Kilidi (KAT)

genesis_id( network_id=0, seed=[1;32], parents=[], ts=0, payload=[] )
= c692f9dd55a0a57b9246679a2820091d0b3b6af27382cb1718bafb4f01fbfe9c

Bu deger known_answer_genesis_id_seed_one testinde sabit. Hash formulu
veya preimage duzeni degisirse test patlar - mainnet sonrasi YASAKTIR.

## Tamamlanan Katmanlar

- dag::vertex - vertex + blake3 + ed25519_strict + KAT
- dag::graph - vertex deposu + ebeveyn linkleri + dongu kontrolu
- consensus::ghostdag - GHOSTDAG tip secimi + mavi skor
- consensus::finality - finality + spine + pruning
- consensus incremental - artimli hesaplama (full ile bit-ayni)
- lsc-net - libp2p P2P, Asama 1: identity + ping

Toplam: 101 (engine) + 2 (net) test gecer.

## Yol Haritasi (Siradaki)

- Kalici dugum kimligi (lsc-net Asama 2)
- Gossip / mesaj yayilimi
- Vertex propagasyonu + engine entegrasyonu
- State / execution, PoS ekonomisi (ileri asamalar)
- Testnet, profesyonel audit, mainnet

## Lisans
Apache-2.0
