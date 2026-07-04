# AIDAG-Chain — Yol Haritasi ve Proje Durumu / Roadmap & Project Status

> **Bu belge, repoyu inceleyenler, katkida bulunmak ya da isbirligi
> dusunenler icindir.** Projenin gercek durumunu ve kalan asamalari
> durustce gosterir. / For those reviewing the repo or considering
> contribution/collaboration. Honestly shows real status and remaining stages.

---

## SU AN NEREDEYIZ / CURRENT STATUS (2026)

### Tamamlanan ve kanitli / Completed & proven
- **Cekirdek DAG + GHOSTDAG** — 279 test yesil. O(n^2) darbogazi cozuldu
  (1M vertex'te ~3535 TPS benchmark ile dogrulandi).
- **Transfer, Kalkan (anti-fraud), belge dogrulama, kurum kimligi** — calisiyor.
- **AVM (Akilli Kontrat Motoru)** — Kopru 1-5 calisiyor: gas, nonce/replay
  korumasi, kontrat storage, node-seviye deploy/call. Standart ERC-20 (deploy+transfer) ve eth_call ile disaridan okuma KANITLANDI. ~25 test.
- **EVM uyumlulugu** — secp256k1, ecrecover, raw Ethereum tx cozme.
- **MetaMask entegrasyonu** — AIDAG cuzdanda goruntulenebiliyor (eth_ RPC,
  chain_id 3474); kisisel test ortaminda dogrulandi.
- **Iki-varlik altyapisi (u128)** — AIDAG + LSC buyuk-sayi (18 ondalik) gecisi.
- **On satis dagitim altyapisi** — owner-imzali dagitim kodlandi + test edildi.
- **Acik kaynak yapi** — Apache-2.0, dokumantasyon, katki rehberi.

### KALAN SON IKI ASAMA / FINAL TWO STAGES

Proje su an erken-asama prototiptir. Uretim/ciddi kullanim icin iki buyuk
asama kalmistir. Bunlar tamamlanmadan mainnet'e ALINMAZ, token satisi YAPILMAZ.
Early-stage prototype. Two major stages remain before production; the chain
will NOT go live and NO token sale will occur before these are complete.

**ASAMA 1 — BAGIMSIZ GUVENLIK DENETIMI (AUDIT)**
- Henuz yapilmadi. Repo bu inceleme icin hazir (temiz, dokumante, acik kaynak).
- Not done yet. The repo is ready for this review.

**ASAMA 2 — MAINNET**
- Henuz yok. Gerekli: gercek genesis (sabit arz, vesting), coklu-node aginin
  yuk altinda kanitlanmasi, halka acik kalici RPC, owner anahtar guvenligi.
- Not live yet. Requires real genesis, multi-node proven under load, public
  RPC, secure owner key management.

> Ancak audit + mainnet tamamlaninca: borsa listeleme, token satisi, kurumsal
> pilot gundeme gelebilir. Once guvenlik ve olgunluk. / Only after audit +
> mainnet can listing, token sale, or pilots be considered. Security first.

---

## DETAYLI YOL HARITASI (asagida) / DETAILED ROADMAP (below)

# AIDAG-Chain — Sıralı Yol Haritası (Vizyon + Gerçek Durum)

> Prensip: "gerçekle örtüşmeli" + "sıralı ve birbirini tamamlayıcı".
> Her adım, bir öncekinin üstüne kurulur. Sıra atlanmaz.
> Durum etiketleri: [ÇALIŞIYOR] = kanıtlı, canlı · [TASARIM] = planlandı, kodlanmadı · [HEDEF] = ileri vizyon

---

## TAMAMLANAN — Çalışan Çekirdek

1. **GHOSTDAG çekirdek** [ÇALIŞIYOR] — Rust, DAG Layer-1, çalışan testnet (binlerce vertex, canlı).
2. **Transfer / ödeme** [ÇALIŞIYOR] — çift-harcama + imza (ed25519) korumalı.
3. **Kalkan (anti-fraud)** [ÇALIŞIYOR] — stake-gated token kaydı + slashing (sahte token reddi).
4. **Belge doğrulama** [ÇALIŞIYOR] — hash + kim + ne zaman, değiştirilemez kayıt.
5. **Kurum kimliği** [ÇALIŞIYOR] — Devlet/Özel kategori altyapısı.
6. **Kalıcılık** [ÇALIŞIYOR] — reboot sonrası zincir geri yüklenir (kanıtlandı).
7. **Sıfır-kurulum web cüzdanı** [ÇALIŞIYOR] — tarayıcıda, ed25519, gerçek transfer.
8. **Site ↔ gerçek zincir köprüsü** [ÇALIŞIYOR] — /api/lsc/real, canlı veri.

## SIRADAKİ ADIMLAR (sıralı — atlanmaz)

9. **Token ekonomisi** [TASARIM→kodlanacak]
   - AIDAG (teminat/değer, 21M sabit) + LSC (yakıt/gas, 2.1B) iki ayrı native defter.
   - Genesis'te sabit arz tanımlı (üretim/madencilik yok).
   - DURUM: tasarım belgesi var (TOKENOMICS_TASARIM.md); kod HENÜZ YOK.

10. **AVM — Akıllı Kontrat Motoru** [HEDEF] — "zirve" adım.
    - Hazır motor entegre (revm / wasm — sıfırdan değil).
    - Nonce (replay koruma) + yakıt (gas) mekanizması BURADA bağlanır.
    - DEX, köprü gibi her şey AVM'nin üstüne kurulur — bu yüzden AVM önce gelir.

11. **Mainnet** [HEDEF] — testnet olgunlaşınca.

12. **Bağımsız güvenlik denetimi (audit)** [HEDEF] — mainnet öncesi, ciddi/pahalı (top-tier).

## İLERİ VİZYON (en son — AVM + mainnet + audit'e bağlı)

13. **Kendi DEX'i** [HEDEF]
    - AVM üstüne kurulur (akıllı kontrat gerektirir).
    - Kendi varlıkları (AIDAG/LSC) + Kalkan'lı tokenlar + (köprüyle) dış varlıklar takas edilir.
    - Kalkan entegrasyonu: sahte/taklit token DEX'e giremez.
    - AI bilgi/analiz katmanı: anomali tespiti → kullanıcıya UYARI sunar; şüpheli token işaretlenir.
      ÖNEMLİ SINIR: AI ÖNERİR/UYARIR, tek başına kontrol etmez (manipülasyon/merkezileşme riski).
    - Amaç: DEX'lerdeki güven sorununu (rug pull, sahte token) Kalkan + AI uyarısı ile azaltmak.

14. **Köprü (bridge)** [HEDEF] — EN SON, EN DİKKATLİ.
    - Dış zincirlerle varlık taşıma (dış değer/likidite getirir).
    - UYARI: köprüler kripto tarihinin en çok hacklenen parçaları (Ronin 600M$, Wormhole 320M$).
    - Denetimsiz ASLA canlıya alınmaz. Audit zorunlu.
    - Köprü-Kalkan: köprü işlemleri için ek koruma katmanı (fikir — köprüyle birlikte tasarlanır).

15. **SoulwareAI olgun katman** [HEDEF — en uzak]
    - 3 AI (OpenAI/Claude/Groq) + kendi modeli vizyonu.
    - DAO'ya öneri sunan, insan-onaylı ortak yönetişim katmanı.
    - SINIR: AI öneri/sunum yapar, bağlayıcı DEĞİL — DAO/insan oylar.
    - DURUM: şu an sadece AI-router (soru-cevap asistanı) çalışıyor. Otonom yönetim = uzak hedef.

## Borsa / değer notu (dürüst)
- CEX listeleme: pahalı (30-50K$+), mainnet + audit + hacim ister — çok ileri.
- DEX: AVM sonrası, kendi zincirinde.
- Token SATIŞI: EN SON — mainnet + audit + avukat + yasal yapı sonrası. Şu an YOK, planlanmıyor.
- Değer, spekülasyondan değil GERÇEK KULLANIMDAN gelir.

## Kritik prensip
Her adım bir öncekine bağlı. Sıra atlanırsa (örn. köprüyü AVM'siz, audit'siz yapmak) =
güvenlik felaketi + boşa emek. Doğru sıra = akıcılık + zaman + güvenlik.
