use anyhow::Result;
use bitcoin::Amount;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use std::collections::HashMap;

pub async fn run_demo() -> Result<()> {
    println!("🚀 RBF Demo - REAL Replace-by-Fee\n");

    // Connect to regtest bitcoind (without wallet first)
    let rpc_base = Client::new("http://127.0.0.1:18443", Auth::UserPass("user".to_string(), "pass".to_string()))?;
    
    // Check regtest is running
    let blockchain_info = rpc_base.get_blockchain_info()?;
    println!("✅ Connected to Bitcoin Core (regtest)");
    println!("   └─ Chain: {}, Blocks: {}\n", blockchain_info.chain, blockchain_info.blocks);

    // Try to load existing wallet or create new one
    let wallet_name = "rbf_demo_wallet";
    match rpc_base.load_wallet(wallet_name) {
        Ok(_) => println!("💼 Loaded existing wallet"),
        Err(_) => {
            match rpc_base.create_wallet(wallet_name, None, None, None, None) {
                Ok(_) => println!("💼 Created new wallet"),
                Err(_) => println!("💼 Using existing wallet"),
            }
        }
    }

    // Connect to the specific wallet
    let rpc = Client::new(&format!("http://127.0.0.1:18443/wallet/{}", wallet_name), Auth::UserPass("user".to_string(), "pass".to_string()))?;

    // Get addresses
    let target_addr = rpc.get_new_address(None, None)?;
    let funding_addr = rpc.get_new_address(None, None)?;
    
    // Fund wallet if needed
    let balance = rpc.get_balance(None, None)?;
    if balance.to_btc() < 10.0 {
        println!("⛏️  Mining blocks for funding...");
        rpc.generate_to_address(10, &funding_addr.clone().assume_checked())?;
        let new_balance = rpc.get_balance(None, None)?;
        println!("   └─ Balance: {} BTC\n", new_balance);
    } else {
        println!("💰 Wallet balance: {} BTC\n", balance);
    }

    // Get a specific UTXO to spend (for true RBF)
    let unspent = rpc.list_unspent(None, None, None, None, None)?;
    if unspent.is_empty() {
        println!("❌ No UTXOs available, mining more blocks...");
        rpc.generate_to_address(5, &funding_addr.clone().assume_checked())?;
        return Ok(());
    }

    let utxo = &unspent[0];
    println!("🎯 Using UTXO: {}:{} ({} BTC)", utxo.txid, utxo.vout, utxo.amount);

    // Calculate reasonable amounts based on UTXO size
    let utxo_amount = utxo.amount.to_btc();
    let send_amount = utxo_amount - 0.001; // Leave 0.001 BTC fee
    let replacement_amount = utxo_amount - 0.01; // Leave 0.01 BTC fee (10x higher)

    println!("💡 Will send {} BTC (0.001 fee), then {} BTC (0.01 fee)\n", send_amount, replacement_amount);

    // === STEP 1: Create Original Transaction (Low Fee, RBF Enabled) ===
    println!("📝 STEP 1: Creating original transaction");
    println!("   ├─ UTXO: {}:{}", utxo.txid, utxo.vout);
    println!("   ├─ Send: {} BTC (small fee)", send_amount);
    println!("   ├─ Fee: LOW");
    println!("   └─ RBF: ENABLED\n");

    // Create raw transaction spending specific UTXO
    let inputs = vec![bitcoincore_rpc::json::CreateRawTransactionInput {
        txid: utxo.txid,
        vout: utxo.vout,
        sequence: Some(0xfffffffd), // RBF enabled!
    }];

    let mut outputs = HashMap::new();
    outputs.insert(target_addr.clone().assume_checked().to_string(), Amount::from_btc(send_amount)?);

    // Create raw transaction (don't use fund_raw_transaction to avoid auto-fee)
    let raw_tx1 = rpc.create_raw_transaction(&inputs, &outputs, None, Some(true))?;
    let signed_tx1 = rpc.sign_raw_transaction_with_wallet(&raw_tx1, None, None)?;

    // Broadcast original transaction
    let original_txid = rpc.send_raw_transaction(&signed_tx1.hex)?;
    println!("✅ Original TX broadcasted: {}", original_txid);

    // Check mempool
    let mempool = rpc.get_raw_mempool()?;
    println!("🔍 Mempool: {} transactions", mempool.len());
    println!("   └─ Contains original: {}\n", mempool.contains(&original_txid));

    // Pause for presentation
    println!("⏸️  [PRESENTATION MOMENT]");
    println!("💡 Original transaction is in mempool with LOW fee");
    println!("💡 It spends UTXO: {}:{}", utxo.txid, utxo.vout);
    println!("   Press Enter to create REPLACEMENT transaction...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    // === STEP 2: Create REPLACEMENT Transaction (SAME UTXO, Higher Fee) ===
    println!("📝 STEP 2: Creating REPLACEMENT transaction");
    println!("   ├─ SAME UTXO: {}:{}", utxo.txid, utxo.vout);
    println!("   ├─ Send: {} BTC (higher fee)", replacement_amount);
    println!("   ├─ Fee: HIGH");
    println!("   └─ RBF: ENABLED\n");

    // Create replacement with SAME inputs but MUCH higher fee (much less output)
    let mut replacement_outputs = HashMap::new();
    replacement_outputs.insert(target_addr.clone().assume_checked().to_string(), Amount::from_btc(replacement_amount)?);

    let raw_tx2 = rpc.create_raw_transaction(&inputs, &replacement_outputs, None, Some(true))?; // SAME inputs!
    let signed_tx2 = rpc.sign_raw_transaction_with_wallet(&raw_tx2, None, None)?;

    // Broadcast replacement transaction
    let replacement_txid = rpc.send_raw_transaction(&signed_tx2.hex)?;
    println!("✅ Replacement TX broadcasted: {}", replacement_txid);

    // Check mempool after replacement
    println!("\n🔍 Mempool Status (After RBF):");
    let final_mempool = rpc.get_raw_mempool()?;
    println!("   ├─ Total transactions: {}", final_mempool.len());
    println!("   ├─ Original TX present: {}", if final_mempool.contains(&original_txid) { "❌ STILL THERE" } else { "✅ EVICTED!" });
    println!("   └─ Replacement TX present: {}", if final_mempool.contains(&replacement_txid) { "✅ YES" } else { "❌ NO" });

    // Show the magic of RBF!
    if !final_mempool.contains(&original_txid) && final_mempool.contains(&replacement_txid) {
        println!("\n🎉 RBF SUCCESS!");
        println!("✅ Original transaction was REPLACED!");
        println!("✅ Same UTXO, higher fee wins!");
        println!("✅ Miners will prefer the replacement!");
    } else {
        println!("\n⚠️  RBF may not have worked as expected");
        println!("   (Both transactions might be in mempool)");
    }

    // Mine a block to see final result
    println!("\n⏸️  [FINAL DEMONSTRATION]");
    println!("🔗 Let's mine a block to see which transaction gets confirmed...");
    println!("   Press Enter to mine block...");
    input.clear();
    std::io::stdin().read_line(&mut input)?;

    println!("⛏️  Mining block...");
    let blocks = rpc.generate_to_address(1, &funding_addr.clone().assume_checked())?;
    
    // Check what actually got confirmed
    let block = rpc.get_block(&blocks[0])?;
    println!("\n📦 Block {} mined!", blocks[0]);
    println!("   ├─ Transactions in block: {}", block.txdata.len());
    
    let orig_confirmed = block.txdata.iter().any(|tx| tx.compute_txid().to_string() == original_txid.to_string());
    let replacement_confirmed = block.txdata.iter().any(|tx| tx.compute_txid().to_string() == replacement_txid.to_string());
    
    println!("   ├─ Original confirmed: {}", if orig_confirmed { "✅ YES" } else { "❌ NO" });
    println!("   └─ Replacement confirmed: {}", if replacement_confirmed { "✅ YES" } else { "❌ NO" });

    // Final verdict
    println!("\n🎉 RBF DEMO COMPLETE!");
    if replacement_confirmed && !orig_confirmed {
        println!("🏆 PERFECT! Only the replacement transaction was mined!");
        println!("💡 The original was completely replaced - this is TRUE RBF!");
    } else if orig_confirmed && !replacement_confirmed {
        println!("🤔 Original was mined instead - RBF didn't work as expected");
    } else {
        println!("🤷 Unexpected result - check the implementation");
    }

    println!("\n📚 What we demonstrated:");
    println!("   ├─ Created transaction spending specific UTXO");
    println!("   ├─ Enabled RBF with sequence < 0xfffffffe");
    println!("   ├─ Created replacement spending SAME UTXO with higher fee");
    println!("   ├─ Showed original was evicted from mempool");
    println!("   └─ Confirmed only replacement was mined");
    println!("\n💡 This is REAL Replace-by-Fee in action!");

    Ok(())
}