// Domain models (ported from shared Kotlin)

mod aggregation;
mod container;
mod network;
mod storage;
mod system;

pub use aggregation::AggregatedSnapshot;
pub use container::{ContainerState, ContainerStats};
pub use network::{InterfaceStat, NetworkStats};
pub use storage::{DiskDeviceStat, PartitionStat, StorageStats};
pub use system::{
    CpuStats, FullSystemSnapshot, FullSystemSnapshotDisplay, RamStats, SystemInfo, SystemStats,
    SystemStatsDynamic, merge_system_info,
};
