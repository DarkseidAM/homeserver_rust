// Dump recent system_history rows as JSON (deserializes wincode BLOBs).
// Merges stored SystemInfo + dynamic system per row for full display.
//
// Usage: cargo run --example dump_history -- [DB_PATH] [LIMIT]
//   DB_PATH  default: ./data/server.db
//   LIMIT    default: 5

use homeserver::history_repo::HistoryRepo;
use homeserver::models::{FullSystemSnapshotDisplay, merge_system_info};
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let path = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("./data/server.db");
    let limit: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);

    let repo = HistoryRepo::connect(path, 3).await?;
    let (stored_info, snapshots) = repo.get_recent_snapshots(limit).await?;

    let display: Vec<FullSystemSnapshotDisplay> = snapshots
        .into_iter()
        .map(|s| FullSystemSnapshotDisplay {
            timestamp: s.timestamp,
            cpu: s.cpu,
            ram: s.ram,
            containers: s.containers,
            storage: s.storage,
            network: s.network,
            system: merge_system_info(stored_info.as_ref(), &s.system),
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&display)?);
    Ok(())
}
