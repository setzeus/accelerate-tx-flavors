#![allow(unused_doc_comments)]
use anyhow::Result;
use bitcoin::Amount;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use std::collections::HashMap;

pub async fn run_demo() -> Result<()> {
    println!("🚀 RBF Demo - REAL Replace-by-Fee\n");

    /////////////////////
    /// Initial Setup ///
    /////////////////////
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
    let target_addr = rpc.get_new_address(None, None)?.assume_checked();
    let funding_addr = rpc.get_new_address(None, None)?.assume_checked();
    
    // Fund wallet if needed
    let balance = rpc.get_balance(None, None)?;
    if balance.to_btc() < 10.0 {
        println!("⛏️  Mining blocks for funding...");
        rpc.generate_to_address(101, &funding_addr)?;
        let new_balance = rpc.get_balance(None, None)?;
        println!("   └─ Balance: {} BTC\n", new_balance);
    } else {
        println!("💰 Wallet balance: {} BTC\n", balance);
    }

    // Get a specific UTXO to spend (for true RBF)
    let unspent = rpc.list_unspent(None, None, None, None, None)?;
    if unspent.is_empty() || unspent[0].amount.to_btc() < 1.0 {
        println!("❌ Need larger UTXOs, mining more blocks...");
        rpc.generate_to_address(100, &funding_addr)?;
        return Ok(());
    }

    let utxo = &unspent[0];
    println!("🎯 Using UTXO: {}:{} ({} BTC)", utxo.txid, utxo.vout, utxo.amount);

    // Calculate reasonable amounts based on UTXO size
    let utxo_amount = utxo.amount.to_btc();
    let fee1 = 0.0001; // Low fee
    let fee2 = 0.001;  // High fee (10x higher)
    let send_amount1 = utxo_amount - fee1;
    let send_amount2 = utxo_amount - fee2;

    println!("💡 Will send {} BTC (fee: {}), then {} BTC (fee: {})\n", 
             send_amount1, fee1, send_amount2, fee2);

    /////////////////////////
    /// First Transaction ///
    /////////////////////////
    println!("📝 STEP 1: Creating original transaction");
    println!("   ├─ UTXO: {}:{}", utxo.txid, utxo.vout);
    println!("   ├─ Send: {} BTC", send_amount1);
    println!("   ├─ Fee: {} BTC (low)", fee1);
    println!("   └─ RBF: ENABLED\n");

    // Create inputs with RBF sequence
    let inputs = vec![bitcoincore_rpc::json::CreateRawTransactionInput {
        txid: utxo.txid,
        vout: utxo.vout,
        sequence: Some(0xfffffffd), // RBF enabled
    }];

    // Create outputs
    let mut outputs = HashMap::new();
    outputs.insert(target_addr.to_string(), Amount::from_btc(send_amount1)?);

    // Create raw transaction
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

    //////////////////////////
    /// Second Transaction ///
    //////////////////////////
    println!("📝 STEP 2: Creating REPLACEMENT transaction");
    println!("   ├─ SAME UTXO: {}:{}", utxo.txid, utxo.vout);
    println!("   ├─ Send: {} BTC", send_amount2);
    println!("   ├─ Fee: {} BTC (10x higher)", fee2);
    println!("   └─ RBF: ENABLED\n");

    // Create replacement with SAME inputs but higher fee
    let mut replacement_outputs = HashMap::new();
    replacement_outputs.insert(target_addr.to_string(), Amount::from_btc(send_amount2)?);

    let raw_tx2 = rpc.create_raw_transaction(&inputs, &replacement_outputs, None, Some(true))?;
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
    let blocks = rpc.generate_to_address(1, &funding_addr)?;
    
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