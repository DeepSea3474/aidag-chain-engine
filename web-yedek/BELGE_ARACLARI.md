# Belge araçları: kayıt talebi + QR'lı çıktı

## 1) Kayıt talebi (KUBRA imzalamaz)

```
tarayıcı: dosya → blake3 (dosya sunucuya GİTMEZ)
   → POST /kubra-brain/v1/belge/hazirla {hash, imzalayan_pubkey?}   (gövde ≤ 1 KB)
   ← imzasız talep (aidag-belge-kayit-talebi v1) + kanıt izi
personel: kendi anahtarıyla imzalar
   - tarayıcıda: lib/belge-kayit.js (anahtar dosyası tarayıcıdan çıkmaz)
   - çevrimdışı: belge-imzala <anahtar> <talep.json>  → hex
   → POST /rpc/submit   (kaydeden = personelin adresi)
```

- KUBRA yalnız okur (`/belge`, `/kurum`, `/tips`); anahtar kullanmaz, `/submit` çağırmaz.
- İmzalayan taraf talebe körü körüne güvenmez: vertex'i talep alanlarından kendisi kurar.
  Payload yalnız `[tip=1][belge_hash]` olabilir. Başka ağ, başka belge, başka imzalayıcı veya
  zaten kayıtlı bir belge için hazırlanmış talep reddedilir.
- Kayıt zamanı, imza anıdır (`ts` imzada güncellenir).
- İmzalayıcı arayüzü `{tur, pubkey, imzala(id)}`. e-İmza ileride aynı arayüzle eklenir.
- **Sınır:** `main`'de kurum kaydı (tip=5) beyana dayanır. Araçlar kurum kaydı olmayan
  adresle imzalamaz, ama zincir kuralı bunu zorlamaz. Doğrulanmış kurum (M-of-N)
  `rwa-oracle-kyc` dalındadır. Çıktılarda kurum "beyan (doğrulanmamış)" olarak yazılır.

## 2) QR'lı çıktı (tamamen tarayıcıda)

- Şablonlar: belge kapağı (A4) ve parça etiketi (70×40 mm).
- QR ve DataMatrix içeriği: `https://aidag-chain.com/belge/<hash>`.
- Code128 içeriği: kısa referans, yani özetin ilk 16 hanesi. Yalnızca eşleştirme içindir,
  kriptografik doğrulama QR/DataMatrix'teki tam özetle yapılır.
- "Yazdır" tarayıcının `window.print()` penceresini açar; "PDF indir" jsPDF ile üretir.
  İkisi de aynı şemadan (`lib/belge-cikti.js`) çizer.
- Çıktı yalnızca zincirde kayıtlı bir özet için üretilir.
- Sunucunun yazıcı ucu yoktur.
- Kütüphaneler yerelden sunulur (`lib/`, lisanslar `lib/lisanslar/`): bwip-js 4.11.4 (MIT),
  jsPDF 4.2.1 (MIT), DejaVu Sans Türkçe alt kümesi.

## 3) blake3.js düzeltmesi (geriye uyumlu)

Eski `web/lib/blake3.js`, 1024 bayttan büyük girdilerde **standart dışı** özet üretiyordu
(tek parça sayıyordu). Vertex kimlikleri etkilenmedi (hep < 1024 bayt); etkilenen yalnızca
sayfada 1 KB'tan büyük dosya/metin özetleriydi.

- `blake3hash` artık standart BLAKE3'tür (Rust/Python ile aynı).
- `blake3hashEski` yalnız doğrulama içindir. Doğrulamada önce standart, bulunamazsa eski
  özet denenir ve sonuç "eski özet" diye etiketlenir. Yeni kayıtlar her zaman standarttır.

## Kurulum (canlıya alma) — onayla

1. Yedek: `/var/www/aidag/{belge-dogrulama.html,lib/}` ve `/etc/nginx`.
2. Kopyala: `web-yedek/aidag/belge-dogrulama.html` → `/var/www/aidag/`;
   `web-yedek/aidag/lib/*` ve `web/lib/blake3.js` → `/var/www/aidag/lib/`.
3. nginx: `web-yedek/nginx-belge-hash.patch` (`/belge/<64 hex>` yolu) → `nginx -t` → `reload`.
4. soulware-core (`/v1/belge/hazirla` + sohbet aracı): yeni ikili + kontrollü yeniden başlatma.

## Testler

- `cargo test -p lsc-engine belge_talep`, `-p soulware-core`, `-p lsc-net --bin belge-imzala`
- `web-yedek/aidag/test/belge-cikti.test.js`: gerçek PDF'i PDFium ile 600 dpi'ye çevirir,
  QR, DataMatrix ve Code128'i görüntüden okur. Ayrıca Türkçe metni ve taşmaları denetler.
- `web-yedek/aidag/test/belge_e2e.py`: izole ağ ad alanında devnet + KUBRA + aday nginx
  ile sayfa, CLI ve sohbet yollarını uçtan uca test eder.
