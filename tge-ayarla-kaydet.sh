#!/bin/bash
# ============================================================================
# tge-ayarla-kaydet.sh — OWNER: ON-SATIS TGE'yi ayarla (tek komut, restart YOK).
#
# TGE POLITIKASI (kurucu karari, 2026-09-22):
#   TGE (kilit acilisi) ancak (1) 1.680.000 AIDAG on satis SATILDIKTAN ve
#   (2) LISTELEME (launchpad/ortaklik) KARARI alindiktan sonra ONE cekilebilir.
#   ERTELEME (TGE'yi daha ileri almak) her zaman serbesttir.
#
# ZINCIR KURALLARI (konsensus, node.rs tip=15) — bu arac once bunlari da kontrol eder:
#   - Yeni TGE >= simdi + 3 gun (TGE_MIN_BILDIRIM_SURESI; aksi halde zincir SESSIZCE reddeder)
#   - Mevcut TGE gunu geldiyse TGE KESINLESMISTIR, degistirilemez.
#
# Kullanim:
#   ./tge-ayarla-kaydet.sh <TGE>
#     TGE: "2027-03-01 14:00" gibi tarih+saat (UTC; saat verilmezse 00:00),
#          dogrudan unix saniye, VEYA "belirsiz" (tarihi ACIK birak = 2100-01-01 isareti)
#   One cekme (gercek TGE tarihi) icin ayrica: LISTELEME_KARARI=evet
#
# Ortam: RPC (varsayilan localhost:8645), NET (3474 mainnet), KEY (aidag-kurucu.key),
#        BIN (./target/release/tge-ayarla)
# GUVENLIK: imza offline binary'de; anahtar bu makinede kalir.
# SONUC DOGRULAMA: gonderimden sonra zincirdeki TGE okunur; "AYARLANDI" yalnizca
# zincir degeri gercekten degistiyse yazilir (sessiz red ASLA basari gibi gosterilmez).
# ============================================================================
set -euo pipefail
RPC="${RPC:-http://127.0.0.1:8645}"
NET="${NET:-3474}"
KEY="${KEY:-aidag-kurucu.key}"
BIN="${BIN:-./target/release/tge-ayarla}"
IN="${1:?TGE tarihi (YYYY-MM-DD) veya unix saniye gerekli}"

ON_SATIS_HEDEF_AIDAG=1680000       # politika: TGE'yi one cekmek icin satilmasi gereken
BILDIRIM_SN=$((3 * 86400))         # zincir kurali: mainnet::TGE_MIN_BILDIRIM_SURESI
TGE_BELIRSIZ=4102444800            # mainnet::TGE_BELIRSIZ (2100-01-01) = "belirlenmedi"
BOS_ADRES=0000000000000000000000000000000000000000

if [ ! -x "$BIN" ]; then echo "HATA: $BIN yok. cargo build --release --bin tge-ayarla" >&2; exit 1; fi
# tarih -> unix (rakamsa dogrudan unix say)
if [ "$IN" = "belirsiz" ]; then TGE=$TGE_BELIRSIZ
elif [[ "$IN" =~ ^[0-9]+$ ]]; then TGE="$IN"
elif [[ "$IN" =~ :[0-9]{2}$ ]]; then TGE=$(date -u -d "$IN" +%s)
else TGE=$(date -u -d "$IN 00:00:00" +%s); fi
tarih() { if [ "$1" -ge "$TGE_BELIRSIZ" ]; then echo "BELIRLENMEDI"; else date -u -d "@$1" '+%Y-%m-%d %H:%M UTC'; fi; }
SIMDI=$(date +%s)

mevcut_tge() {
  curl -s -m5 "$RPC/on-satis-tahsis/$BOS_ADRES" | python3 -c "import sys,json;print(json.load(sys.stdin)['tge'])"
}
MEVCUT=$(mevcut_tge)
echo "Mevcut zincir TGE : $MEVCUT ($(tarih "$MEVCUT"))"
echo "Istenen TGE       : $TGE ($(tarih "$TGE"))"

# --- Zincir kurallari (onceden kontrol; zincir ihlali sessizce reddeder) ---
if [ "$SIMDI" -ge "$MEVCUT" ]; then
  echo "RED: mevcut TGE gunu gecti -> TGE KESINLESTI, degistirilemez (zincir kurali)." >&2; exit 1
fi
if [ "$TGE" -lt $((SIMDI + BILDIRIM_SN)) ]; then
  echo "RED: TGE en az 3 gun sonrasi olmali (en erken: $(tarih $((SIMDI + BILDIRIM_SN)))). Zincir reddeder." >&2; exit 1
fi

# --- Politika: ONE CEKME yalniz 1.68M satis + listeleme karariyla ---
if [ "$TGE" -lt "$MEVCUT" ]; then
  SATILAN=$(curl -s -m5 "$RPC/on-satis-ozet" | python3 -c "import sys,json;print(int(json.load(sys.stdin)['toplam_satilan_aidag'])//10**18)")
  echo "Zincirde satilan (test haric): $SATILAN AIDAG / hedef $ON_SATIS_HEDEF_AIDAG"
  if [ "$SATILAN" -lt "$ON_SATIS_HEDEF_AIDAG" ]; then
    echo "RED (politika): TGE one cekilemez — on satis hedefi ($ON_SATIS_HEDEF_AIDAG AIDAG) dolmadi." >&2; exit 1
  fi
  if [ "${LISTELEME_KARARI:-}" != "evet" ]; then
    echo "RED (politika): listeleme/launchpad karari onaylanmadi. Karar alindiysa LISTELEME_KARARI=evet ile calistir." >&2; exit 1
  fi
else
  echo "Islem: ERTELEME (TGE ileri aliniyor) — politika kosulu gerekmez."
fi

TIPS=$(curl -s -m5 "$RPC/tips" | python3 -c "import sys,json;print(','.join(json.load(sys.stdin).get('tips',[])[:8]) or '-')")  # MAX_PARENTS=8
HEX=$("$BIN" "$KEY" "$NET" "$TGE" "$SIMDI" "$TIPS")
RESP=$(curl -s -m10 -X POST "$RPC/submit" -H 'Content-Type: application/json' -d "{\"hex\":\"$HEX\"}")
echo "Gonderim yaniti: $RESP"
if ! echo "$RESP" | grep -q '"ok":true'; then
  echo "HATA: gonderim reddedildi." >&2; exit 1
fi

# --- Sonucu ZINCIRDEN dogrula (gonderim kabulu != kural kabulu) ---
for _ in 1 2 3 4 5; do
  sleep 2
  YENI=$(mevcut_tge)
  if [ "$YENI" = "$TGE" ]; then
    echo "TGE AYARLANDI ✓ (zincirden dogrulandi): $TGE ($(tarih "$TGE"))."
    exit 0
  fi
done
echo "HATA: vertex kabul edildi ama zincirdeki TGE DEGISMEDI (hala $YENI). Kural reddi olabilir." >&2
exit 1
