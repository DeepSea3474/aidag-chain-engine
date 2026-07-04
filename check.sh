#!/usr/bin/env bash
# AIDAG-Chain / LSC — yerel kalite kapisi (CI'in yerel, bedava esdegeri).
#
# Her commit/push ONCESI calistir: ./check.sh
# Uc kapi (CI ile birebir ayni): bicim -> lint -> test.
# Herhangi biri patlarsa script DURUR (set -e) ve hata kodu doner.

set -e  # ilk hatada dur

echo "=== 1/3: Bicim (cargo fmt --check) ==="
cargo fmt --all -- --check
echo "  OK: bicim temiz"

echo "=== 2/3: Lint (cargo clippy, uyari=hata) ==="
cargo clippy --all-targets --all-features -- -D warnings
echo "  OK: clippy temiz (sifir uyari)"

echo "=== 3/3: Testler (cargo test) ==="
cargo test --all
echo "  OK: tum testler gecti"

echo ""
echo "######################################"
echo "#  TUM KAPILAR YESIL - push'a hazir  #"
echo "######################################"
