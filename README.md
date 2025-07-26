# Bitcoin Acceleration Favors

🚀 Interactive demonstrations of Bitcoin transaction acceleration techniques: RBF, CPFP, and P2A (Ephemeral Anchors).

## Overview

This demo showcases three different methods for accelerating Bitcoin transactions when they get stuck with low fees:

1. **RBF (Replace-by-Fee)** - Replace your own transaction with a higher fee version
2. **CPFP (Child-Pays-for-Parent)** - Create a high-fee child transaction to accelerate the parent
3. **P2A (Pay-to-Anchor/Ephemeral Anchors)** - Use anyone-can-spend anchors for fee acceleration

## Prerequisites

- **Rust** (latest stable version)
- **Bitcoin Core** (v26.0+ recommended for P2A support)
  - Download from [bitcoin.org](https://bitcoin.org/en/download) 
  - Includes `bitcoind` (the daemon) and `bitcoin-cli`
  - Alternative: `brew install bitcoin` (macOS) or `sudo apt install bitcoind` (Ubuntu)
- **Cargo** for building and running

## Setup

### 1. Start Bitcoin Core in Regtest Mode

```bash
bitcoind -regtest -daemon -rpcuser=user -rpcpassword=pass -fallbackfee=0.0001 -acceptnonstdtxn=1
```

**Flag explanations:**
- `-regtest` - Use regression test network (local testing)
- `-daemon` - Run in background
- `-rpcuser=user -rpcpassword=pass` - RPC credentials
- `-fallbackfee=0.0001` - Default fee rate
- `-acceptnonstdtxn=1` - Accept non-standard transactions (enables v3 for P2A)
- `-mempoolfullrbf=1` - Enable full RBF support

### 2. Clone and Build

```bash
git clone
cd accelerate-txs-demo
cargo build
```

### 3. Run the Demo

```bash
cargo run
```

Select your demonstration:
- `1` - RBF Demo
- `2` - CPFP Demo  
- `3` - P2A Demo

## What Each Demo Shows

### 🔄 RBF (Replace-by-Fee)
- Creates a transaction with **low fees** and **RBF enabled** (sequence < 0xfffffffe)
- Shows the transaction getting stuck in mempool
- Creates a **replacement transaction** spending the same UTXO with **higher fees**
- Demonstrates the original transaction being **evicted** from mempool
- **Key insight**: Same inputs, higher fee wins

### 👨‍👩‍👧‍👦 CPFP (Child-Pays-for-Parent)
- Creates a **parent transaction** with very low fees (gets stuck)
- Parent has **RBF disabled** (sequence = 0xffffffff)
- Creates a **child transaction** spending from the parent with very high fees
- Shows both transactions being mined together
- **Key insight**: High child fee incentivizes miners to include low-fee parent

### ⚓ P2A (Pay-to-Anchor/Ephemeral Anchors)
- Creates a **v3 transaction** with ephemeral anchor output (0 satoshis)
- Uses the **P2A script pattern**: `OP_1 <0x4e73>`
- Shows **anyone-can-spend** anchor acceleration
- Demonstrates **TRUC topology restrictions** (v3 → v3 spending rules)
- **Key insight**: More efficient than CPFP, anyone can accelerate

## Technical Details

### Transaction Versions
- **RBF & CPFP**: Use standard v2 transactions
- **P2A**: Uses v3 transactions (TRUC - Topologically Restricted Until Confirmation)

### P2A Script Pattern
```
Script: OP_1 <0x4e73>
Hex: 0x01514e73
Mainnet Address: bc1pfeessrawgf... (deterministic)
```

### Sequence Numbers for RBF
```rust
0xfffffffd  // RBF enabled
0xfffffffe  // RBF enabled  
0xffffffff  // RBF disabled (final)
```

## Auto-Setup Features

The demos automatically handle:
- ✅ **Wallet creation** - Creates `rbf_demo_wallet` if it doesn't exist
- ✅ **Funding** - Mines blocks if wallet balance < 10 BTC
- ✅ **UTXO management** - Uses available UTXOs intelligently
- ✅ **Address generation** - Creates fresh addresses for each demo

## Dependencies

```toml
[dependencies]
bitcoin = { version = "0.32.4", features = ["rand-std"] }
bitcoincore-rpc = "0.19"
anyhow = "1.0"
tokio = { version = "1.0", features = ["full"] }
hex = "0.4"
```

## Running Multiple Demos

You can run the demos in any order:
- First run will auto-fund the wallet
- Subsequent runs use the same funded wallet
- Each demo uses different UTXOs to avoid conflicts

## Troubleshooting

### "Connection refused" error
Make sure bitcoind is running:
```bash
bitcoin-cli -regtest -rpcuser=user -rpcpassword=pass getblockchaininfo
```

### "min relay fee not met" error
This can happen if:
- UTXOs are too small for the transaction amounts
- Try running other demos first to create larger UTXOs
- Restart with a fresh regtest: `bitcoin-cli -regtest stop && bitcoind [flags]`

### P2A demo fails
- Ensure you're using Bitcoin Core v26.0+ 
- Check that `-acceptnonstdtxn=1` flag is set
- P2A requires v3 transaction support

## Educational Value

This demo teaches:
- **Real Bitcoin transaction mechanics** (not testnet simulation)
- **Fee market dynamics** and miner incentives  
- **Mempool behavior** and transaction replacement
- **Modern Bitcoin features** like v3 transactions and ephemeral anchors
- **Practical fee acceleration** techniques used in production

## Production vs Demo

**Demo differences:**
- Uses regtest (instant mining)
- Higher fees for demonstration purposes
- Interactive pauses for explanation

**Production similarities:**
- Real Bitcoin Core RPC calls
- Actual transaction construction
- Authentic script patterns and signatures
- Same economic incentives

## Contributing

Feel free to submit issues or PRs to improve the demos or add new acceleration techniques!

## License

MIT License - See LICENSE file for details

---

**⚠️ Educational Purpose Only**: This demo is for learning Bitcoin transaction mechanics. Always test thoroughly before using similar techniques on mainnet.