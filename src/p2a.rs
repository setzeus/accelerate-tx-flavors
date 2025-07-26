use anyhow::Result;
use bitcoin::{Amount, Transaction, TxOut, TxIn, OutPoint, Witness, Sequence};
use bitcoin::script::{Builder, PushBytesBuf, ScriptBuf};
use bitcoin::opcodes::all::OP_PUSHNUM_1;
use bitcoincore_rpc::{Auth, Client, RpcApi};

pub async fn run_demo() -> Result<()> {
    println!("🚀 P2A Demo - Ephemeral Anchors\n");

    // Connect to regtest bitcoind
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

    // Get addresses - FIXED: Remove .clone()
    let funding_addr = rpc.get_new_address(None, None)?.assume_checked();
    let target_addr = rpc.get_new_address(None, None)?.assume_checked();
    
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

    // Get a UTXO
    let unspent = rpc.list_unspent(None, None, None, None, None)?;
    if unspent.is_empty() || unspent[0].amount.to_btc() < 1.0 {
        println!("❌ Need larger UTXOs, mining more blocks...");
        rpc.generate_to_address(100, &funding_addr)?;
        return Ok(());
    }

    let utxo = &unspent[0];
    println!("🎯 Using UTXO: {}:{} ({} BTC)", utxo.txid, utxo.vout, utxo.amount);

    // === STEP 1: Create Transaction with P2A Anchor ===
    println!("\n📝 STEP 1: Creating transaction with P2A anchor");
    println!("   ├─ Regular transaction output");
    println!("   ├─ Plus: anchor output (0 value - true ephemeral!)");
    println!("   ├─ P2A script: OP_1 <0x4e73>");
    println!("   └─ Fee: VERY LOW (will get stuck)\n");

    // Create P2A (Pay-to-Anchor) script: OP_1 <0x4e73>
    let push_bytes = PushBytesBuf::try_from(&[0x4e, 0x73]).unwrap();
    let p2a_script = Builder::new()
        .push_opcode(OP_PUSHNUM_1)
        .push_slice(push_bytes)
        .into_script();

    println!("🔍 P2A Script Details:");
    println!("   ├─ Script hex: {}", hex::encode(p2a_script.as_bytes()));
    println!("   ├─ Script: OP_1 <4e73>");
    println!("   ├─ Length: {} bytes", p2a_script.len());
    println!("   └─ Anyone-can-spend: ✅\n");

    // Calculate amounts - SIMPLIFIED
    let utxo_amount = utxo.amount.to_btc();
    let fee_amount = 0.001; // Small fee for parent
    let send_amount = ((utxo_amount - fee_amount) * 100_000_000.0).round() / 100_000_000.0;
    let anchor_amount = 0.0; // TRUE ephemeral anchor - 0 value!

    println!("💡 Transaction breakdown:");
    println!("   ├─ Send: {} BTC to target", send_amount);
    println!("   ├─ Anchor: {} sats (TRUE ephemeral!)", (anchor_amount * 100_000_000.0) as u64);
    println!("   └─ Fee: {} BTC (low)", fee_amount);

    // Now manually build the transaction with the anchor
    let tx_input = TxIn {
        previous_output: OutPoint::new(utxo.txid, utxo.vout),
        script_sig: ScriptBuf::new(),
        sequence: Sequence(0xffffffff),
        witness: Witness::new(),
    };

    let mut tx_outputs_vec = vec![
        TxOut {
            value: Amount::from_btc(send_amount)?,
            script_pubkey: target_addr.script_pubkey(),
        }
    ];

    // Add the ephemeral anchor output (0 value for v3 transactions)
    let anchor_output = TxOut {
        value: Amount::from_sat(0), // ZERO value - true ephemeral anchor!
        script_pubkey: p2a_script.clone(),
    };
    tx_outputs_vec.push(anchor_output);

    // Build the complete transaction (version 3 for ephemeral anchors)
    let tx = Transaction {
        version: bitcoin::transaction::Version(3), // V3 for ephemeral anchors
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![tx_input],
        output: tx_outputs_vec,
    };

    // Sign and broadcast the transaction
    let tx_hex = hex::encode(bitcoin::consensus::encode::serialize(&tx));
    let signed_tx = rpc.sign_raw_transaction_with_wallet(tx_hex, None, None)?;
    let main_txid = rpc.send_raw_transaction(&signed_tx.hex)?;

    println!("✅ Transaction with P2A anchor broadcasted: {}", main_txid);
    println!("   ├─ Sends: {} BTC to target (main output)", send_amount);
    println!("   ├─ Fee: {} BTC (minimal - anchor will accelerate)", fee_amount);
    println!("   └─ Anchor: 0 sats (TRUE ephemeral anchor!)");

    // Check mempool
    let mempool = rpc.get_raw_mempool()?;
    println!("\n🔍 Mempool: {} transactions", mempool.len());
    println!("   └─ Contains main tx: {}", mempool.contains(&main_txid));

    // Pause for presentation
    println!("\n⏸️  [PRESENTATION MOMENT]");
    println!("💡 Transaction has very low fees and might get stuck!");
    println!("💡 But it has a 0-value ephemeral anchor output (v3 tx)");
    println!("💡 Anyone can spend this anchor to accelerate the transaction");
    println!("   Press Enter to spend the anchor and add fees...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    // === STEP 2: Create Anchor Spend Transaction ===
    println!("📝 STEP 2: Spending the P2A anchor to add fees");
    println!("   ├─ Spends the 0-value anchor output");
    println!("   ├─ Adds external UTXO for fees");
    println!("   ├─ High fee to accelerate main transaction");
    println!("   └─ Anyone can do this (no signature needed for anchor)\n");

    // Get another UTXO for fee payment
    if unspent.len() < 2 {
        println!("❌ Need more UTXOs, mining some...");
        rpc.generate_to_address(10, &funding_addr)?;
        return Ok(());
    }

    let fee_utxo = &unspent[1];
    let fee_utxo_amount = fee_utxo.amount.to_btc();
    let high_fee = 0.01; // High fee for acceleration
    let fee_change = fee_utxo_amount - high_fee;

    println!("💡 Anchor spend breakdown:");
    println!("   ├─ Anchor input: 0 sats (TRUE ephemeral anchor)");
    println!("   ├─ Fee UTXO input: {} BTC", fee_utxo_amount);
    println!("   ├─ Output: {} BTC", fee_change);
    println!("   └─ Fee: {} BTC (HIGH!)", high_fee);

    // Create anchor spend transaction manually (v3 required to spend from v3)
    let anchor_tx_input = TxIn {
        previous_output: OutPoint::new(main_txid, (tx.output.len() - 1) as u32),
        script_sig: ScriptBuf::new(),
        sequence: Sequence(0xfffffffe),
        witness: Witness::new(),
    };

    let fee_tx_input = TxIn {
        previous_output: OutPoint::new(fee_utxo.txid, fee_utxo.vout),
        script_sig: ScriptBuf::new(),
        sequence: Sequence(0xfffffffe),
        witness: Witness::new(),
    };

    let mut anchor_tx_outputs_vec = vec![];
    if fee_change > 0.001 {
        anchor_tx_outputs_vec.push(TxOut {
            value: Amount::from_btc(fee_change)?,
            script_pubkey: funding_addr.script_pubkey(),
        });
    }

    let anchor_spend_tx = Transaction {
        version: bitcoin::transaction::Version(3), // V3 required to spend from v3
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![anchor_tx_input, fee_tx_input],
        output: anchor_tx_outputs_vec,
    };

    // Sign and broadcast anchor spend transaction
    let anchor_tx_hex = hex::encode(bitcoin::consensus::encode::serialize(&anchor_spend_tx));
    let signed_anchor = rpc.sign_raw_transaction_with_wallet(anchor_tx_hex, None, None)?;
    let anchor_txid = rpc.send_raw_transaction(&signed_anchor.hex)?;

    println!("✅ Anchor spend transaction broadcasted: {}", anchor_txid);
    println!("   ├─ Spends: Ephemeral anchor (0 sats - TRUE ephemeral!)");
    println!("   ├─ Spends: Fee UTXO ({} BTC)", fee_utxo_amount);
    println!("   ├─ Fee: {} BTC (HIGH!)", high_fee);
    println!("   └─ Change: {} BTC", fee_change);

    // Check final mempool
    println!("\n🔍 Mempool Status (After Anchor Spend):");
    let final_mempool = rpc.get_raw_mempool()?;
    println!("   ├─ Total transactions: {}", final_mempool.len());
    println!("   ├─ Main TX present: {}", if final_mempool.contains(&main_txid) { "✅ YES" } else { "❌ NO" });
    println!("   └─ Anchor Spend present: {}", if final_mempool.contains(&anchor_txid) { "✅ YES" } else { "❌ NO" });

    // Show economics
    println!("\n💰 P2A Economics:");
    println!("   ├─ Main tx fee: {} BTC (low)", fee_amount);
    println!("   ├─ Anchor spend fee: {} BTC (high)", high_fee);
    println!("   ├─ Total package fee: {} BTC", fee_amount + high_fee);
    println!("   └─ Miners see: HIGH total fee for both transactions!");

    if final_mempool.contains(&main_txid) && final_mempool.contains(&anchor_txid) {
        println!("\n🎉 P2A SUCCESS!");
        println!("✅ Both main tx and anchor spend are in mempool!");
        println!("✅ High anchor fee accelerates the low-fee main transaction!");
        println!("✅ Anyone could have done this anchor spend!");
    }

    // Mine a block
    println!("\n⏸️  [FINAL DEMONSTRATION]");
    println!("🔗 Let's mine a block to see both transactions confirmed...");
    println!("   Press Enter to mine block...");
    input.clear();
    std::io::stdin().read_line(&mut input)?;

    println!("⛏️  Mining block...");
    let blocks = rpc.generate_to_address(1, &funding_addr)?;
    
    // Check confirmations
    let block = rpc.get_block(&blocks[0])?;
    println!("\n📦 Block {} mined!", blocks[0]);
    println!("   ├─ Transactions in block: {}", block.txdata.len());
    
    let main_confirmed = block.txdata.iter().any(|tx| tx.compute_txid().to_string() == main_txid.to_string());
    let anchor_confirmed = block.txdata.iter().any(|tx| tx.compute_txid().to_string() == anchor_txid.to_string());
    
    println!("   ├─ Main TX confirmed: {}", if main_confirmed { "✅ YES" } else { "❌ NO" });
    println!("   └─ Anchor Spend confirmed: {}", if anchor_confirmed { "✅ YES" } else { "❌ NO" });

    // Final verdict
    println!("\n🎉 P2A DEMO COMPLETE!");
    if main_confirmed && anchor_confirmed {
        println!("🏆 SUCCESS! Both transactions were mined together!");
        println!("💡 The anchor spend accelerated the main transaction!");
    }

    println!("\n📚 What we demonstrated:");
    println!("   ├─ Created v3 transaction with 0-value P2A anchor");
    println!("   ├─ Main transaction had low fees");
    println!("   ├─ Spent the anchor with high fees to accelerate");
    println!("   ├─ Both transactions mined together");
    println!("   └─ True ephemeral anchor demo!");

    println!("\n💡 Key P2A Benefits:");
    println!("   ├─ 0-value anchors enable fee acceleration");
    println!("   ├─ Anyone can accelerate stuck transactions");
    println!("   ├─ More efficient than traditional CPFP");
    println!("   ├─ True ephemeral anchors with v3 transactions");
    println!("   └─ Enables new transaction fee patterns");

    Ok(())
}