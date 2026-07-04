# Katkida Bulunma / Contributing

> Turkce + English

## Turkce

AIDAG-Chain'e katkida bulunmak istediginiz icin tesekkurler.

### Nasil katki yapilir
1. Repoyu fork'layin (kendi hesabiniza kopyalayin).
2. Bir dal (branch) acin: `git checkout -b ozellik/aciklama`
3. Degisikliginizi yapin.
4. Kalite kapilarini gecin (ZORUNLU):
   - `cargo fmt` (bicimlendirme)
   - `cargo clippy` (kod kalitesi, uyari birakmadan)
   - `cargo test --lib` (tum testler yesil olmali)
5. Commit + push edin, sonra bir Pull Request (PR) acin.
6. PR'da NE degistirdiginizi ve NEDEN oldugunu acikca yazin.

### Ilkeler (bu proje icin onemli)
- **Once kanit:** Her degisiklik test + calisan kanit ile gelir. Iddia yeter degil.
- **Kucuk ve net:** Buyuk, karisik PR yerine kucuk, tek amacli PR'lar.
- **Guvenlik:** Anahtar, sifre, kisisel veri ASLA commit edilmez.
- **Dil:** Kod yorumlari Turkce (mevcut kod boyle). PR aciklamasi Turkce veya Ingilizce olabilir.

### Ne tur katkilar degerli
- Hata duzeltmeleri (test ile birlikte)
- Guvenlik iyilestirmeleri
- Dokumantasyon (README, kod yorumlari, Ingilizce ceviri)
- Test kapsamini artirmak
- Performans iyilestirmeleri (benchmark kaniti ile)

### Onemli not
Bu erken asama bir projedir ve proje sahibi (Aydin Akyuz) tum PR'lari
inceler. Buyuk mimari degisiklikler once bir Issue acilarak tartisilmalidir.

## English

Thanks for your interest in contributing to AIDAG-Chain.

### How to contribute
1. Fork the repo.
2. Create a branch: `git checkout -b feature/description`
3. Make your change.
4. Pass the quality gates (REQUIRED):
   - `cargo fmt`
   - `cargo clippy` (no warnings)
   - `cargo test --lib` (all tests green)
5. Commit, push, and open a Pull Request.
6. Clearly describe WHAT you changed and WHY.

### Principles
- **Proof first:** every change comes with tests / working proof.
- **Small and focused:** small, single-purpose PRs.
- **Security:** never commit keys, secrets, or personal data.
- **Language:** code comments are in Turkish (existing code style); PR
  descriptions may be Turkish or English.

### Note
This is an early-stage project. The project owner (Aydin Akyuz) reviews all
PRs. For major architectural changes, please open an Issue to discuss first.
