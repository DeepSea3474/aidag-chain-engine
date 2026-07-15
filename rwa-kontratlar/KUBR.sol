// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.26;

/// @title KUBR - Degerli Maden RWA Token (Altin)
/// @notice 1 KUBR = 1 gram altin temsil eder. ERC20 + RWA maden meta-verisi.
/// @dev AIDAG-Chain AVM uzerinde calisir. Standart ERC20 (borsa/cuzdan uyumlu)
///      + degerli maden teminat bilgisi.
contract KUBR {
    // --- ERC20 standart ---
    string public name = "Kubra Irem Gold";
    string public symbol = "KUBR";
    uint8  public decimals = 18;
    uint256 public totalSupply;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    // --- RWA maden katmani (KUBR'i ozel yapan) ---
    string  public metal = "GOLD";        // maden turu
    uint16  public purity = 9999;         // saflik: 9999 = %99.99 (24 ayar)
    uint256 public mgPerToken = 1000;     // 1 token = 1 gram = 1000 miligram
    address public custodian;             // teminati tutan saklayici
    uint256 public collateralMg;          // toplam teminat (miligram altin)

    address public owner;

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);
    event CollateralUpdated(uint256 newCollateralMg);
    event OwnershipTransferred(address indexed oncekiOwner, address indexed yeniOwner);

    modifier onlyOwner() {
        require(msg.sender == owner, "sadece owner");
        _;
    }

    /// @param _custodian teminati tutan saklayici adresi
    /// @param _ilkArz baslangic arzi (token, 18 ondalikli)
    constructor(address _custodian, uint256 _ilkArz) {
        // C4: custodian sifir adres olamaz (RWA teminat sahibi tanimsiz kalmasin).
        require(_custodian != address(0), "custodian sifir adres olamaz");
        owner = msg.sender;
        custodian = _custodian;
        totalSupply = _ilkArz;
        balanceOf[msg.sender] = _ilkArz;
        // teminat: her token 1 gram = 1000 mg. Ilk arz kadar teminat beklenir.
        // C3: ONCE carp sonra bol (truncation'i onle). Baslangic teminatini FAZLA
        // gostermemek icin floor (deposit edilenden fazla altin iddia edilmez).
        collateralMg = (_ilkArz * mgPerToken) / (10 ** 18);
        emit Transfer(address(0), msg.sender, _ilkArz);
    }

    // --- ERC20 fonksiyonlari ---
    function transfer(address to, uint256 value) external returns (bool) {
        require(to != address(0), "sifir adrese transfer olmaz"); // C4
        require(balanceOf[msg.sender] >= value, "yetersiz bakiye");
        balanceOf[msg.sender] -= value;
        balanceOf[to] += value;
        emit Transfer(msg.sender, to, value);
        return true;
    }

    function approve(address spender, uint256 value) external returns (bool) {
        allowance[msg.sender][spender] = value;
        emit Approval(msg.sender, spender, value);
        return true;
    }

    /// @notice C4 (approve-race onleme): izni MUTLAK set etmek yerine ARTIR.
    /// @dev Klasik approve-race (eski izni harcayip yeni izni de kapmak) icin
    ///      OpenZeppelin-tarzi guvenli yol. Overflow 0.8'de otomatik revert.
    function increaseAllowance(address spender, uint256 ekle) external returns (bool) {
        allowance[msg.sender][spender] += ekle;
        emit Approval(msg.sender, spender, allowance[msg.sender][spender]);
        return true;
    }

    /// @notice C4: izni AZALT (taban 0'a saturasyon; underflow yok).
    function decreaseAllowance(address spender, uint256 azalt) external returns (bool) {
        uint256 mevcut = allowance[msg.sender][spender];
        allowance[msg.sender][spender] = azalt >= mevcut ? 0 : mevcut - azalt;
        emit Approval(msg.sender, spender, allowance[msg.sender][spender]);
        return true;
    }

    function transferFrom(address from, address to, uint256 value) external returns (bool) {
        require(to != address(0), "sifir adrese transfer olmaz"); // C4
        require(balanceOf[from] >= value, "yetersiz bakiye");
        require(allowance[from][msg.sender] >= value, "yetersiz izin");
        allowance[from][msg.sender] -= value;
        balanceOf[from] -= value;
        balanceOf[to] += value;
        emit Transfer(from, to, value);
        return true;
    }

    // --- RWA yonetim (teminat) ---
    /// @notice Saklayici teminati gunceller (gercek altin girisi/cikisi).
    function setCollateral(uint256 _collateralMg) external onlyOwner {
        collateralMg = _collateralMg;
        emit CollateralUpdated(_collateralMg);
    }

    /// @notice C4: sahipligi devret (zero-address korumali). Yeni owner yonetim
    /// yetkilerini (setCollateral vb.) devralir. Owner kaybi/yanlis devir onlenir.
    function transferOwnership(address yeniOwner) external onlyOwner {
        require(yeniOwner != address(0), "yeni owner sifir adres olamaz");
        emit OwnershipTransferred(owner, yeniOwner);
        owner = yeniOwner;
    }

    /// @notice Teminat orani: teminat (mg) >= arz (token) * mgPerToken olmali.
    /// @return teminatli mi (her token'in arkasinda altin var mi)
    function tamTeminatliMi() external view returns (bool) {
        // C3: ONCE carp sonra bol (truncation'i onle) + TAVANA yuvarla. Kesirli
        // token'lari yutan floor, az-teminati "tam teminatli" gosterirdi. Ceil ->
        // gereksinim muhafazakar (fazla teminat ister), az-teminat maskelenmez.
        uint256 gerekenMg = (totalSupply * mgPerToken + (10 ** 18 - 1)) / (10 ** 18);
        return collateralMg >= gerekenMg;
    }
}
