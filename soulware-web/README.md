# KUBRA — Tarayıcı Katkı Sayfası (sıfır-kurulum)

`katil.html` — insanlar bir **web sayfası açar, "Katkı Ver" der**, tarayıcılarının
GPU/CPU gücü KUBRA'yı çalıştırır, **AIDAG LSC kazanır**. Kurulum YOK → güven maksimum.

## Güven-önce tasarım
- Kurulum yok · açık kaynak · dosyalara dokunmaz · non-custodial (kazanç senin cüzdanında) · izin-önce (başlamadan çalışmaz) · şeffaf canlı panel.

## Test/Deploy adımları
1. **Koordinatörü public yap** (tarayıcı ona ulaşmalı):
   - `SOULWARE_COORD_LISTEN=0.0.0.0:8647` + firewall'da 8647 aç.
   - CORS zaten açık (`access-control-allow-origin: *`).
   - GÜVENLİK: koordinatör settle-key'siz = mainnet-güvenli kuyruk modu. Üretimde rate-limit/reverse-proxy önerilir.
2. **HTTPS uyumu (önemli):** HTTPS sayfa → HTTP koordinatörü ÇAĞIRAMAZ (mixed-content). Seçenekler:
   - Koordinatörün önüne **nginx + TLS** koy (`https://api.aidag-chain.com` → 127.0.0.1:8647), VEYA
   - Sayfayı da HTTP'den servis et (test için).
3. **COORD URL'sini ayarla:** sayfada `window.KUBRA_COORD` ya da script'teki `COORD` sabiti:
   ```html
   <script>window.KUBRA_COORD = "https://api.aidag-chain.com";</script>
   ```
4. **Sayfayı yayınla:** `katil.html`'i siteye koy (örn. `aidag-chain.com/katil.html`).
5. **Tarayıcıda test:** WebGPU destekli tarayıcı (Chrome/Edge/Firefox güncel) + aç → adres gir → "Katkı Ver" → model iner (birkaç yüz MB, bir kez) → iş çeker → kazanır.

## Doğrulama notu (bilinen mühendislik düğümü)
Ağ, güvenliği "aynı işi ≥2 worker yapsın, çıktı eşleşsin" ile sağlar. İki tarayıcı-worker
aynı modeli (Qwen2.5-1.5B, greedy) çalıştırırsa eşleşir → doğrulanır → ödül. Farklı
model/donanım karışımında birebir eşleşme için ileride tolerans/hesap-izi doğrulaması gerekir.

## Sonraki (Faz 2)
Güçlü GPU sahipleri için **masaüstü uygulaması** → büyük model (70B) katkısı → yüksek tier.
