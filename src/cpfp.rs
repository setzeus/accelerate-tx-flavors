use anyhow::Result;
use bitcoin::Amount;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use std::collections::HashMap;

pub async fn run_demo() -> Result<()> {
    println!("🚀 CPFP Demo - Child-Pays-for-Parent\n");

    // Connect to regtest bitcoind
    let rpc_base = Client::new("http://127.0.0.1:18443", Auth::UserPass("user".to_string(), "pass".to_string()))?;
    
    // Check regtest is running
    let blockchain_info = rpc_base.get_blockchain_info()?;
    println!("✅ Connected to Bitcoin Core (regtest)");
    println!("   └─ Chain: {}, Blocks: {}\n", blockchain_info.chain, blockchain_info.blocks);

    // Try to load existing wallet or create new one
    let wallet_name = "rbf_demo_wallet"; // Use same wallet as RBF demo
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
    let funding_addr = rpc.get_new_address(None, None)?;
    let intermediate_addr = rpc.get_new_address(None, None)?; // For parent tx output
    let final_addr = rpc.get_new_address(None, None)?; // For child tx output
    
    // Fund wallet if needed
    let balance = rpc.get_balance(None, None)?;
    if balance.to_btc() < 10.0 {
        println!("⛏️  Mining blocks for funding...");
        rpc.generate_to_address(101, &funding_addr.clone().assume_checked())?;
        let new_balance = rpc.get_balance(None, None)?;
        println!("   └─ Balance: {} BTC\n", new_balance);
    } else {
        println!("💰 Wallet balance: {} BTC\n", balance);
    }

    // Get a UTXO to create our parent transaction
    let unspent = rpc.list_unspent(None, None, None, None, None)?;
    if unspent.is_empty() {
        println!("❌ No UTXOs available, mining more blocks...");
        rpc.generate_to_address(100, &funding_addr.clone().assume_checked())?;
        return Ok(());
    }

    let utxo = &unspent[0];
    println!("🎯 Using UTXO: {}:{} ({} BTC)", utxo.txid, utxo.vout, utxo.amount);

    // === STEP 1: Create Parent Transaction (Low Fee) ===
    println!("\n📝 STEP 1: Creating PARENT transaction with LOW fee");
    
    // Calculate amounts based on actual UTXO
    let utxo_amount = utxo.amount.to_btc();
    let parent_send_amount = 1.0; // Send exactly 1 BTC
    let parent_fee = 0.0001; // Very small fee
    let parent_change = ((utxo_amount - parent_send_amount - parent_fee) * 100_000_000.0).round() / 100_000_000.0; // Round to 8 decimal places

    println!("   ├─ Input: {}:{} ({} BTC)", utxo.txid, utxo.vout, utxo_amount);
    println!("   ├─ Output: {} BTC to intermediate address", parent_send_amount);
    println!("   ├─ Change: {} BTC back to wallet", parent_change);
    println!("   ├─ Fee: {} BTC (VERY LOW)", parent_fee);
    println!("   └─ RBF: DISABLED (can't be replaced)\n");

    // Create parent transaction
    let parent_inputs = vec![bitcoincore_rpc::json::CreateRawTransactionInput {
        txid: utxo.txid,
        vout: utxo.vout,
        sequence: Some(0xffffffff), // NO RBF - final sequence
    }];

    let mut parent_outputs = HashMap::new();
    parent_outputs.insert(intermediate_addr.clone().assume_checked().to_string(), Amount::from_btc(parent_send_amount)?);
    if parent_change > 0.0 {
        parent_outputs.insert(funding_addr.clone().assume_checked().to_string(), Amount::from_btc(parent_change)?);
    }

    // Create and sign parent transaction
    let parent_raw = rpc.create_raw_transaction(&parent_inputs, &parent_outputs, None, Some(false))?;
    let parent_signed = rpc.sign_raw_transaction_with_wallet(&parent_raw, None, None)?;

    // Broadcast parent transaction
    let parent_txid = rpc.send_raw_transaction(&parent_signed.hex)?;
    println!("✅ Parent TX broadcasted: {}", parent_txid);
    println!("   ├─ Creates: {} BTC output for child to spend", parent_send_amount);
    println!("   ├─ Fee: {} BTC (very low)", parent_fee);
    println!("   └─ RBF: DISABLED");

    // Check mempool
    let mempool = rpc.get_raw_mempool()?;
    println!("\n🔍 Mempool: {} transactions", mempool.len());
    println!("   └─ Contains parent: {}", mempool.contains(&parent_txid));

    // Pause for presentation
    println!("\n⏸️  [PRESENTATION MOMENT]");
    println!("💡 Parent transaction is stuck with very low fee!");
    println!("💡 It cannot use RBF (sequence = 0xffffffff)");
    println!("💡 But we can use CPFP to accelerate it!");
    println!("   Press Enter to create CHILD transaction...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    // === STEP 2: Create Child Transaction (High Fee) ===
    println!("📝 STEP 2: Creating CHILD transaction with HIGH fee");
    
    // Child spends the 1 BTC output from parent
    let child_send_amount = 0.99; // Send 0.99 BTC
    let child_fee = 0.01; // 0.01 BTC fee (100x higher than parent!)
    
    println!("   ├─ Input: Parent's {} BTC output ({}:0)", parent_send_amount, parent_txid);
    println!("   ├─ Output: {} BTC to final address", child_send_amount);
    println!("   ├─ Fee: {} BTC (100x HIGHER than parent!)", child_fee);
    println!("   └─ Effect: Accelerates BOTH parent and child\n");

    // Create child transaction
    let child_inputs = vec![bitcoincore_rpc::json::CreateRawTransactionInput {
        txid: parent_txid,
        vout: 0, // First output of parent (the 1 BTC to intermediate_addr)
        sequence: Some(0xfffffffe),
    }];

    let mut child_outputs = HashMap::new();
    child_outputs.insert(final_addr.clone().assume_checked().to_string(), Amount::from_btc(child_send_amount)?);

    // Create and sign child transaction
    let child_raw = rpc.create_raw_transaction(&child_inputs, &child_outputs, None, None)?;
    let child_signed = rpc.sign_raw_transaction_with_wallet(&child_raw, None, None)?;

    // Broadcast child transaction
    let child_txid = rpc.send_raw_transaction(&child_signed.hex)?;
    println!("✅ Child TX broadcasted: {}", child_txid);
    println!("   ├─ Spends: Parent output ({}:0)", parent_txid);
    println!("   ├─ Output: {} BTC to final address", child_send_amount);
    println!("   └─ Fee: {} BTC (HIGH!)", child_fee);

    // Check mempool after child
    println!("\n🔍 Mempool Status (After CPFP):");
    let final_mempool = rpc.get_raw_mempool()?;
    println!("   ├─ Total transactions: {}", final_mempool.len());
    println!("   ├─ Parent TX present: {}", if final_mempool.contains(&parent_txid) { "✅ YES" } else { "❌ NO" });
    println!("   └─ Child TX present: {}", if final_mempool.contains(&child_txid) { "✅ YES" } else { "❌ NO" });

    // Show CPFP economics
    println!("\n💰 CPFP Economics:");
    println!("   ├─ Parent fee: {} BTC", parent_fee);
    println!("   ├─ Child fee: {} BTC", child_fee);
    println!("   ├─ Combined fee: {} BTC", parent_fee + child_fee);
    println!("   └─ Miners see: HIGH total fee for transaction package!");

    if final_mempool.contains(&parent_txid) && final_mempool.contains(&child_txid) {
        println!("\n🎉 CPFP SUCCESS!");
        println!("✅ Both parent and child are in mempool!");
        println!("✅ High child fee incentivizes miners to include both!");
        println!("✅ Parent gets 'pulled along' by profitable child!");
    }

    // Mine a block to see final result
    println!("\n⏸️  [FINAL DEMONSTRATION]");
    println!("🔗 Let's mine a block to see both transactions get confirmed...");
    println!("   Press Enter to mine block...");
    input.clear();
    std::io::stdin().read_line(&mut input)?;

    println!("⛏️  Mining block...");
    let blocks = rpc.generate_to_address(1, &funding_addr.clone().assume_checked())?;
    
    // Check what actually got confirmed
    let block = rpc.get_block(&blocks[0])?;
    println!("\n📦 Block {} mined!", blocks[0]);
    println!("   ├─ Transactions in block: {}", block.txdata.len());
    
    let parent_confirmed = block.txdata.iter().any(|tx| tx.compute_txid().to_string() == parent_txid.to_string());
    let child_confirmed = block.txdata.iter().any(|tx| tx.compute_txid().to_string() == child_txid.to_string());
    
    println!("   ├─ Parent confirmed: {}", if parent_confirmed { "✅ YES" } else { "❌ NO" });
    println!("   └─ Child confirmed: {}", if child_confirmed { "✅ YES" } else { "❌ NO" });

    // Final verdict
    println!("\n🎉 CPFP DEMO COMPLETE!");
    if parent_confirmed && child_confirmed {
        println!("🏆 PERFECT! Both parent and child were mined together!");
        println!("💡 The high-fee child pulled the low-fee parent along!");
        println!("💡 This is how CPFP accelerates stuck transactions!");
    } else if child_confirmed && !parent_confirmed {
        println!("🤔 Only child was mined - this shouldn't happen!");
        println!("   (Child can't be valid without parent)");
    } else {
        println!("🤷 Neither transaction was mined - check the implementation");
    }

    println!("\n📚 What we demonstrated:");
    println!("   ├─ Created parent transaction with very low fee");
    println!("   ├─ Parent got stuck (no RBF available)");
    println!("   ├─ Created child spending from parent with very high fee");
    println!("   ├─ Miners included both transactions for the combined fee");
    println!("   └─ Child 'paid for' parent's confirmation");

    println!("\n💡 Key CPFP Insights:");
    println!("   ├─ Child transaction MUST spend parent's output");
    println!("   ├─ Miners consider package fee rate (total fees / total size)");
    println!("   ├─ High child fee can make low parent fee profitable");
    println!("   ├─ Both transactions are mined together (atomic)");
    println!("   └─ Useful when RBF is not available or desired");

    Ok(())
}