// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/// @title AIDAG Belge Damgasi (DocumentRegistry)
/// @notice Gercek dunya belgesinin hash'ini zincire degismez sekilde kaydeder.
///         Kurumsal kullanim: noter, diploma, sertifika, sozlesme dogrulama.
contract BelgeDamgasi {
    struct Kayit {
        uint64 zaman; // blok zaman damgasi
        bool var_mi; // kayit mevcut mu
    }

    // C2 (front-running onleme): (kaydeden, belge hash) -> kayit.
    // ESKI: `mapping(bytes32 => Kayit)` GLOBAL idi + ilk-gelen-kazanir. Hash public
    // oldugu icin saldirgan mempool'da hash'i gorup KENDI adina once kaydedip
    // gercek sahibin kaydini bloke edebiliyordu (sahiplik gaspi). Namespace ile her
    // adres KENDI alaninda kaydeder; baskasinin ayni hash'i kaydetmesi seni etkilemez.
    mapping(address => mapping(bytes32 => Kayit)) private kayitlar;
    uint256 public toplamKayit;

    event BelgeKaydedildi(address indexed kaydeden, bytes32 indexed belgeHash, uint64 zaman);

    /// @notice Bir belge hash'ini KENDI adin altinda kaydet.
    /// @dev Ayni (adres, hash) ikilisi iki kez kaydedilemez; farkli adresler ayni
    ///      hash'i BAGIMSIZ kaydedebilir (front-running sahiplik gaspi imkansiz).
    function kaydet(bytes32 belgeHash) external {
        require(!kayitlar[msg.sender][belgeHash].var_mi, "Bu adres bu belgeyi zaten kaydetti");
        kayitlar[msg.sender][belgeHash] = Kayit({zaman: uint64(block.timestamp), var_mi: true});
        toplamKayit += 1;
        emit BelgeKaydedildi(msg.sender, belgeHash, uint64(block.timestamp));
    }

    /// @notice Belirli bir adresin bu belge hash'ini kaydedip kaydetmedigini dogrula.
    /// @param kaydeden sahipligi sorgulanan adres
    /// @param belgeHash belge hash'i
    function dogrula(address kaydeden, bytes32 belgeHash)
        external
        view
        returns (bool varMi, uint64 zaman)
    {
        Kayit memory k = kayitlar[kaydeden][belgeHash];
        return (k.var_mi, k.zaman);
    }
}
