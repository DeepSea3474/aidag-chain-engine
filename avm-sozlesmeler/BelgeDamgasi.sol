// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/// @title AIDAG Belge Damgasi (DocumentRegistry)
/// @notice Gercek dunya belgesinin hash'ini zincire degismez sekilde kaydeder.
///         Kurumsal kullanim: noter, diploma, sertifika, sozlesme dogrulama.
contract BelgeDamgasi {
    struct Kayit {
        address kaydeden;   // belgeyi kaydeden adres
        uint64  zaman;      // blok zaman damgasi
        bool    var_mi;     // kayit mevcut mu
    }

    // belge hash'i -> kayit
    mapping(bytes32 => Kayit) private kayitlar;
    uint256 public toplamKayit;

    event BelgeKaydedildi(bytes32 indexed belgeHash, address indexed kaydeden, uint64 zaman);

    /// @notice Bir belge hash'ini kaydet. Ayni hash iki kez kaydedilemez.
    function kaydet(bytes32 belgeHash) external {
        require(!kayitlar[belgeHash].var_mi, "Belge zaten kayitli");
        kayitlar[belgeHash] = Kayit({
            kaydeden: msg.sender,
            zaman: uint64(block.timestamp),
            var_mi: true
        });
        toplamKayit += 1;
        emit BelgeKaydedildi(belgeHash, msg.sender, uint64(block.timestamp));
    }

    /// @notice Bir belgenin kayitli olup olmadigini ve detayini dogrula.
    function dogrula(bytes32 belgeHash) external view returns (bool varMi, address kaydeden, uint64 zaman) {
        Kayit memory k = kayitlar[belgeHash];
        return (k.var_mi, k.kaydeden, k.zaman);
    }
}
