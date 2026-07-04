// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

/// Standart minimal ERC-20 token (AVM uyumluluk kaniti icin).
/// totalSupply, balanceOf, transfer, approve, transferFrom, allowance.
contract Token {
    string public name = "AIDAG Test Token";
    string public symbol = "ATT";
    uint8 public decimals = 18;
    uint256 public totalSupply;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);

    constructor(uint256 baslangicArz) {
        totalSupply = baslangicArz;
        balanceOf[msg.sender] = baslangicArz;
        emit Transfer(address(0), msg.sender, baslangicArz);
    }

    function transfer(address to, uint256 value) public returns (bool) {
        require(balanceOf[msg.sender] >= value, "yetersiz bakiye");
        balanceOf[msg.sender] -= value;
        balanceOf[to] += value;
        emit Transfer(msg.sender, to, value);
        return true;
    }

    function approve(address spender, uint256 value) public returns (bool) {
        allowance[msg.sender][spender] = value;
        emit Approval(msg.sender, spender, value);
        return true;
    }

    function transferFrom(address from, address to, uint256 value) public returns (bool) {
        require(balanceOf[from] >= value, "yetersiz bakiye");
        require(allowance[from][msg.sender] >= value, "yetersiz izin");
        balanceOf[from] -= value;
        balanceOf[to] += value;
        allowance[from][msg.sender] -= value;
        emit Transfer(from, to, value);
        return true;
    }
}
