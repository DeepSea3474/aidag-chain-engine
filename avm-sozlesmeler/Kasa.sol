// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

/// B1 KANIT KONTRATI (fon donmasi testi):
/// Native AIDAG'i kabul eder (payable) ve kontrat-tutulan bakiyeyi UCUNCU tarafa
/// gonderir (withdraw). Eski kodda kontrat-ici bu hareket gercek deftere yansimaz
/// -> fon donar. B1 fix'ten sonra `cek` sonrasi alicinin gercek bakiyesi artar.
contract Kasa {
    /// Native AIDAG yatir (payable). Kontrat bakiyesi artar.
    function depozito() external payable {}

    /// Duz transfer icin de kabul et.
    receive() external payable {}

    /// Kontrat-tutulan native AIDAG'i ucuncu tarafa gonder (withdraw).
    function cek(address payable alici, uint256 miktar) external {
        (bool ok, ) = alici.call{value: miktar}("");
        require(ok, "transfer basarisiz");
    }

    /// Kontratin native AIDAG bakiyesi.
    function bakiyem() external view returns (uint256) {
        return address(this).balance;
    }
}
