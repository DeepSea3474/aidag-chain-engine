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

    modifier onlyOwner() {
        require(msg.sender == owner, "sadece owner");
        _;
    }

    /// @param _custodian teminati tutan saklayici adresi
    /// @param _ilkArz baslangic arzi (token, 18 ondalikli)
    constructor(address _custodian, uint256 _ilkArz) {
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

    function transferFrom(address from, address to, uint256 value) external returns (bool) {
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
