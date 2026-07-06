# AIDAG-Chain

**A Rust Layer-1 with DAG consensus and an EVM-compatible execution layer — engineered as infrastructure for verifiable digital records.**



![Rust](https://img.shields.io/badge/Rust-stable-orange)

 

![Network](https://img.shields.io/badge/network-testnet-blue)

 

![Tests](https://img.shields.io/badge/tests-290%2B%20passing-brightgreen)

 

![EVM](https://img.shields.io/badge/EVM-compatible-8A2BE2)



## What it is

AIDAG-Chain is a DAG-based Layer-1 blockchain. Blocks are vertices in a directed acyclic graph, ordered by **GHOSTDAG** consensus for parallel, high-throughput block production without a single-chain bottleneck. The execution layer is **EVM-compatible** (built on `revm`), so MetaMask, ethers.js, and Solidity contracts work against it out of the box.

It is not a whitepaper — it runs. A live testnet where MetaMask sends real AIDAG transfers that settle on-chain, backed by **290+ passing tests** across consensus, execution, and state.

## Verified, today

- **Live testnet** — public JSON-RPC, faucet, and block explorer at [aidag-chain.com](https://aidag-chain.com).
- **Real wallet transfers** — MetaMask sends value on AIDAG-Chain; balances update on-chain and are visible in the explorer.
- **290+ tests, green** — consensus, EVM execution, gas accounting, and registries — enforced through `fmt` / `clippy -D warnings` / `test` gates.

## Core

- **GHOSTDAG DAG consensus** — mergeset selection, blue/red ordering, parallel vertices.
- **EVM execution (`revm`)** — Ethereum `eth_*` JSON-RPC, EIP-1559 transactions, Solidity / ERC-20 contracts.
- **Two-asset economy** — **AIDAG** (native value, 21M cap) for transfers; **LSC** (gas) charged per transaction and split 50% burn / 50% development pool — a self-sustaining, non-inflationary fee model.
- **On-chain registries** — balances, document/record verification, and institutional identity with public/private-sector separation.

## Where it's going

AIDAG-Chain is built as infrastructure for **verifiable digital records**: document authentication and institutional identity anchored on a public ledger, with a programmable EVM layer on top. The path runs from testnet to a multi-node network, an independent security audit, and mainnet — at which point the on-chain registries become production verification rails for institutions.

## Try it

1. **Add the network to MetaMask** — RPC `https://aidag-chain.com/rpc`, Chain ID `3474`, symbol `AIDAG`, 18 decimals.
2. **Claim AIDAG** from the faucet.
3. **Send a transaction** from MetaMask and view it at `aidag-chain.com/scan`.

## Architecture

| Component | Description |
|---|---|
| `lsc-engine/` | Rust core — GHOSTDAG consensus, AVM (`revm` execution), registries, genesis, transaction model |
| `lsc-net/` | Ethereum-compatible JSON-RPC server (`eth_*`) and networking |
| Front-end | Next.js block explorer, faucet, and wallet dApp |

## Stack

Rust · `revm` · GHOSTDAG · ed25519 / secp256k1 · Next.js / TypeScript / React · Python · Linux (systemd, pm2)

## Status

Testnet — mainnet follows an independent security audit.

## Links

- **Explorer, faucet & dApp:** https://aidag-chain.com
- **Repository:** https://github.com/DeepSea3474/aidag-chain-engine
