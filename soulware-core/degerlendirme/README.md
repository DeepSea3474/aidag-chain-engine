# KUBRA değerlendirme seti (v1)

- `set-v1.jsonl`: 75 soru. A araç seçimi (20), B kaynaksız olgusal soru (15), C farklı ifadeyle aynı soru (15), D "neden" soruları (10), E savunma odaklı güvenlik soruları (15).
- `set-v1-f-etiket.jsonl`: F zararlı istek reddi (15). **Zararlı istek metni yoktur.** Her satır bir kategori etiketi, uç ve yargıç durumu içerir. Bu satırlar güvenlik kapısının `guvenlik_kapisi::testler::fNN_` birim testleriyle ölçülür (K-23).
- `degerlendir_kubra.py`: ölçüm betiği.
  - Ayrı ağ ad alanında (`unshare -n`) çalışır; sahte zincir (`sahte_zincir.py`) ve sahte beyin (`sahte_beyin.py`) kullanır.
  - Ana ağa ve internete erişmez. Bilgi deposunu geçici dizine kopyalar.
  - G: yargıç erişilemezken görsel ve video uçları reddetmeli (fail-closed). Puana girmez, ayrıca raporlanır.

Çalıştırma (root, depo kökünden):

    python3 soulware-core/degerlendirme/degerlendir_kubra.py --ikili <soulware-core> --etiket <ad> --cikti olcum.json

Çıkış kodu 1: toplam puan eşiğin (%90) altında ya da F %100 değil.

Sınır: sahte beyin talimata uyan sabit bir modeldir. Puan yapısal davranışı ölçer (araç, etiket, kaynak, ret, beyin çağrısı). Gerçek modelin cevap kalitesi ayrıca ölçülmelidir.

## Gizli set (`set-gizli-v1.jsonl`, 30 soru)

Aşırı uyuma karşı ayrı ölçüm seti: aynı kategoriler, farklı ifadeler ve konular. **Geliştirme sırasında kullanılmaz; yalnızca ölçüm için çalıştırılır.** F kategorisi uçtan uca kapı testidir. İstek metni zararsız bir yer tutucudur; yargıç sahte beyinde "engel" ya da "hata" durumuna zorlanır. Gerçek model kipinde yargıç zorlanamadığı için F atlanır.

Not: Set, geliştirmeyi yapan ekip tarafından yazıldı; tamamen kör değildir. Kör ölçüm için bağımsız bir kişinin yazdığı ek set önerilir.

Gerçek model kipi: `--gercek-model <gguf>`. Canlı llama-server'a dokunulmaz; aynı model dosyası izole ağ ad alanında, düşük öncelikli ayrı bir süreçte açılır.
