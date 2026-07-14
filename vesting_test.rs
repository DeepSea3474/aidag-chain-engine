// SoulwareAI/AIDAG — VESTING mantik testi (izole, entegrasyondan once)
// Cliff + dogrusal acilim. Blok/zaman bazli.

type Tutar = u128;

#[derive(Debug)]
struct VestingPlani {
    toplam: Tutar,        // kilitli toplam miktar
    baslangic: u64,       // vesting baslangic zamani (unix saniye)
    cliff_sure: u64,      // cliff suresi (saniye) - bu sureden once HIC acilmaz
    toplam_sure: u64,     // toplam vesting suresi (saniye)
}

impl VestingPlani {
    fn yeni(toplam: Tutar, baslangic: u64, cliff_gun: u64, toplam_gun: u64) -> Self {
        VestingPlani {
            toplam,
            baslangic,
            cliff_sure: cliff_gun * 86400,
            toplam_sure: toplam_gun * 86400,
        }
    }

    // Belli bir zamanda ACILMIS (kullanilabilir) miktari hesapla
    fn acilmis(&self, simdi: u64) -> Tutar {
        if simdi < self.baslangic + self.cliff_sure {
            return 0; // cliff dolmadi -> hic acilmadi
        }
        let gecen = simdi - self.baslangic;
        if gecen >= self.toplam_sure {
            return self.toplam; // sure doldu -> hepsi acildi
        }
        // dogrusal: gecen sure / toplam sure orani
        self.toplam * (gecen as u128) / (self.toplam_sure as u128)
    }

    // Kilitli (henuz acilmamis) miktar
    fn kilitli(&self, simdi: u64) -> Tutar {
        self.toplam - self.acilmis(simdi)
    }
}

fn main() {
    println!("=== VESTING MANTIK TESTI (Cliff + Dogrusal) ===\n");

    // Kurucu: 2.730.000 AIDAG, 6 ay cliff, 24 ay toplam
    let baslangic = 1_000_000_000u64; // ornek baslangic
    let kurucu = VestingPlani::yeni(2_730_000, baslangic, 180, 730); // 6ay cliff, 2yil

    let gun = 86400u64;
    let senaryolar = [
        ("Baslangic (gun 0)", baslangic),
        ("3. ay (cliff icinde)", baslangic + 90*gun),
        ("6. ay (cliff biter)", baslangic + 180*gun),
        ("12. ay (yari yol)", baslangic + 365*gun),
        ("24. ay (biter)", baslangic + 730*gun),
        ("25. ay (sonra)", baslangic + 760*gun),
    ];

    for (ad, zaman) in senaryolar {
        let acik = kurucu.acilmis(zaman);
        let kilit = kurucu.kilitli(zaman);
        println!("{:22} -> acik: {:>9} | kilitli: {:>9}", ad, acik, kilit);
    }

    println!("\n=== KONTROL ===");
    // Cliff icinde hic acilmamali
    assert_eq!(kurucu.acilmis(baslangic + 90*gun), 0, "cliff icinde acilmamali");
    // Cliff sonunda acilmaya baslamali
    assert!(kurucu.acilmis(baslangic + 200*gun) > 0, "cliff sonrasi acilmali");
    // Sure sonunda hepsi acilmali
    assert_eq!(kurucu.acilmis(baslangic + 730*gun), 2_730_000, "sure sonunda hepsi");
    // Arz korunumu: acik + kilitli = toplam
    let t = baslangic + 365*gun;
    assert_eq!(kurucu.acilmis(t) + kurucu.kilitli(t), 2_730_000, "acik+kilit=toplam");
    println!("TUM KONTROLLER GECTI - vesting mantigi dogru");
}
