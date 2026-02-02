// Dump recent system_history rows as JSON (deserializes wincode BLOBs).
//
// Usage: cargo run --example dump_history -- [DB_PATH] [LIMIT]
//   DB_PATH  default: ./data/server.db
//   LIMIT    default: 5

use homeserver::history_repo::HistoryRepo;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).map(String::as_str).unwrap_or("./data/server.db");
    let limit: u32 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    let repo = HistoryRepo::connect(path).await?;
    let snapshots = repo.get_recent_snapshots(limit).await?;

    println!("{}", serde_json::to_string_pretty(&snapshots)?);
    Ok(())
}
