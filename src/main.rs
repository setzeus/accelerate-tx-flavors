mod rbf;
mod cpfp;
mod p2a;

use anyhow::Result;
use std::io;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Bitcoin Transaction Acceleration Demo\n");
    
    println!("Select a demonstration:");
    println!("1. RBF (Replace-by-Fee)");
    println!("2. CPFP (Child-Pays-for-Parent)");
    println!("3. P2A (Ephemeral Anchors)");
    println!("\nEnter your choice (1-3): ");

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    
    match input.trim() {
        "1" => {
            println!("🔄 Starting RBF Demo...\n");
            rbf::run_demo().await?;
        },
        "2" => {
            println!("🔄 Starting CPFP Demo...\n");
            cpfp::run_demo().await?;
        },
        "3" => {
            println!("🔄 Starting P2A Demo...\n");
            p2a::run_demo().await?;
        },
        _ => {
            println!("❌ Invalid choice. Please run again and select 1, 2, or 3.");
            return Ok(());
        }
    }
    Ok(())
}