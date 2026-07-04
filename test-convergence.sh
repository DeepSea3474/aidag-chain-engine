#!/usr/bin/env bash
# AIDAG-Chain / LSC — Cok-node YAKINSAMA testi (ag davranisi).
#
# check.sh (fmt+clippy+unit test) MOTOR mantigini dogrular.
# Bu script AG davranisini dogrular: gercek node'lar baslatilir, yakinsama
# olcum ile kontrol edilir. Timing'e bagli oldugu icin cargo test'ten AYRI.
#
# Kullanim: ./test-convergence.sh   (once cargo build --bin lsc-node gerekir)
#
# Iki senaryo:
#   1) 3-node zincir (A<-B<-C): 1 uretici + 2 dinleyici, hepsi yakinsamali
#   2) 2-uretici (A + B uretir, C dinler): paralel DAG, tek genesis, orphan=0

set -u
BIN=./target/debug/lsc-node
PASS=0
FAIL=0

cleanup() { pkill -f lsc-node 2>/dev/null; sleep 1; rm -f aidag-data-*.log aidag-key-*.bin; }

check() { # $1=aciklama $2=beklenen $3=gercek
  if [ "$2" = "$3" ]; then
    echo "  GECTI: $1 (=$3)"; PASS=$((PASS+1))
  else
    echo "  KALDI: $1 (beklenen=$2, gercek=$3)"; FAIL=$((FAIL+1))
  fi
}

echo "=== Binary kontrol ==="
if [ ! -x "$BIN" ]; then echo "HATA: $BIN yok. Once: cargo build --bin lsc-node"; exit 1; fi

# ---------- SENARYO 1: 3-node zincir ----------
echo ""
echo "=== SENARYO 1: 3-node zincir (A<-B<-C), 1 uretici + 2 dinleyici ==="
cleanup
$BIN /ip4/127.0.0.1/tcp/40001 > /tmp/cA.log 2>&1 & sleep 3
$BIN /ip4/127.0.0.1/tcp/40002 /ip4/127.0.0.1/tcp/40001 listen > /tmp/cB.log 2>&1 & sleep 3
$BIN /ip4/127.0.0.1/tcp/40003 /ip4/127.0.0.1/tcp/40002 listen > /tmp/cC.log 2>&1 & sleep 25
pkill -f lsc-node 2>/dev/null; sleep 1

B1=$(grep -oE "toplam_vertex=[0-9]+" /tmp/cB.log | tail -1 | grep -oE "[0-9]+")
C1=$(grep -oE "toplam_vertex=[0-9]+" /tmp/cC.log | tail -1 | grep -oE "[0-9]+")
B1=${B1:-0}; C1=${C1:-0}
echo "  B=$B1, C=$C1"
check "B ve C yakinsadi" "$B1" "$C1"
[ "${C1:-0}" -ge 3 ] && { echo "  GECTI: C anlamli sayida vertex aldi (=$C1)"; PASS=$((PASS+1)); } || { echo "  KALDI: C cok az vertex (=$C1)"; FAIL=$((FAIL+1)); }

# ---------- SENARYO 2: 2-uretici paralel ----------
echo ""
echo "=== SENARYO 2: 2-uretici (A + B uretir, C dinler), paralel DAG ==="
cleanup
$BIN /ip4/127.0.0.1/tcp/40001 > /tmp/cA.log 2>&1 & sleep 4
$BIN /ip4/127.0.0.1/tcp/40002 /ip4/127.0.0.1/tcp/40001 produce > /tmp/cB.log 2>&1 & sleep 4
$BIN /ip4/127.0.0.1/tcp/40003 /ip4/127.0.0.1/tcp/40002 listen > /tmp/cC.log 2>&1 & sleep 35
pkill -f lsc-node 2>/dev/null; sleep 1

GEN=$(grep -c "Genesis uretildi" /tmp/cA.log /tmp/cB.log /tmp/cC.log | grep -oE ":[0-9]+" | grep -oE "[0-9]+" | paste -sd+ | bc)
REJ=$(grep -c "second genesis" /tmp/cA.log /tmp/cB.log /tmp/cC.log | grep -oE ":[0-9]+" | grep -oE "[0-9]+" | paste -sd+ | bc)
ORP=$(grep -c "orphan'a alindi" /tmp/cA.log /tmp/cB.log /tmp/cC.log | grep -oE ":[0-9]+" | grep -oE "[0-9]+" | paste -sd+ | bc)
B2=$(grep -oE "toplam_vertex=[0-9]+" /tmp/cB.log | tail -1 | grep -oE "[0-9]+")
C2=$(grep -oE "toplam_vertex=[0-9]+" /tmp/cC.log | tail -1 | grep -oE "[0-9]+")
B2=${B2:-0}; C2=${C2:-0}
echo "  toplam_genesis=$GEN, ikinci_genesis_reddi=$REJ, kalan_orphan=$ORP, B=$B2, C=$C2"
check "Tek genesis uretildi" "1" "$GEN"
check "Ikinci genesis reddi yok" "0" "$REJ"
check "Kalan orphan yok" "0" "$ORP"
check "B ve C yakinsadi" "$B2" "$C2"

# ---------- SENARYO 3: mDNS otomatik kesif ----------
echo ""
echo "=== SENARYO 3: mDNS otomatik kesif (dial_addr YOK), paralel + yakinsama ==="
cleanup
# Dikkat: dial_addr verilMEZ. Node'lar birbirini mDNS ile bulmali.
$BIN /ip4/0.0.0.0/tcp/40001 > /tmp/cA.log 2>&1 & sleep 5
$BIN /ip4/0.0.0.0/tcp/40002 produce > /tmp/cB.log 2>&1 & sleep 30
pkill -f lsc-node 2>/dev/null; sleep 1

MDNS=$(grep -c "mDNS kesfetti" /tmp/cA.log)
GEN3=$(grep -c "Genesis uretildi" /tmp/cA.log /tmp/cB.log | grep -oE ":[0-9]+" | grep -oE "[0-9]+" | paste -sd+ | bc)
REJ3=$(grep -c "second genesis" /tmp/cA.log /tmp/cB.log | grep -oE ":[0-9]+" | grep -oE "[0-9]+" | paste -sd+ | bc)
A3=$(grep -oE "toplam_vertex=[0-9]+" /tmp/cA.log | tail -1 | grep -oE "[0-9]+")
B3=$(grep -oE "toplam_vertex=[0-9]+" /tmp/cB.log | tail -1 | grep -oE "[0-9]+")
A3=${A3:-0}; B3=${B3:-0}
echo "  mDNS_kesif=$MDNS, toplam_genesis=$GEN3, ikinci_genesis_reddi=$REJ3, A=$A3, B=$B3"
[ "${MDNS:-0}" -ge 1 ] && { echo "  GECTI: mDNS otomatik kesif calisti (=$MDNS)"; PASS=$((PASS+1)); } || { echo "  KALDI: mDNS kesif yok"; FAIL=$((FAIL+1)); }
check "Tek genesis (mDNS senaryo)" "1" "$GEN3"
check "Ikinci genesis reddi yok (mDNS)" "0" "$REJ3"
# A ve B farki en fazla 1 olmali (son vertex yayilma aninda olabilir)
DIFF=$((A3 - B3)); DIFF=${DIFF#-}
[ "$DIFF" -le 1 ] && { echo "  GECTI: A ve B yakinsadi (A=$A3, B=$B3, fark<=1)"; PASS=$((PASS+1)); } || { echo "  KALDI: A=$A3 B=$B3 fark cok"; FAIL=$((FAIL+1)); }

# ---------- SONUC ----------
cleanup
echo ""
echo "######################################"
if [ "$FAIL" -eq 0 ]; then
  echo "#  YAKINSAMA TESTLERI YESIL ($PASS gecti)  #"
  echo "######################################"
  exit 0
else
  echo "#  YAKINSAMA HATASI ($FAIL kaldi, $PASS gecti)  #"
  echo "######################################"
  exit 1
fi
