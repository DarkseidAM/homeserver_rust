// Unit tests for the alerting engine: pure metric extraction + the fire/resolve/cooldown
// state machine driven by an injected clock (no async, no real time).

use homeserver::alerting::{AlertEngine, AlertState, compare, extract_metric};
use homeserver::config::AlertRule;
use homeserver::models::*;
use std::time::{Duration, Instant};

fn snapshot(cpu_usage: f64) -> FullSystemSnapshot {
    FullSystemSnapshot {
        timestamp: 0,
        cpu: CpuStats {
            usage_percent: cpu_usage,
            temperature: 80.0,
            ..Default::default()
        },
        ram: RamStats {
            usage_percent: 50.0,
            swap_total: 100,
            swap_used: 25,
            ..Default::default()
        },
        containers: vec![],
        storage: StorageStats {
            partitions: vec![PartitionStat {
                mount: "/".into(),
                name: "sda1".into(),
                type_: "ext4".into(),
                total_space: 100,
                used_space: 95,
                available_space: 5,
                usage_percent: 95.0,
            }],
            disks: vec![],
        },
        network: NetworkStats::default(),
        system: SystemStatsDynamic {
            load_avg_1: 3.0,
            ..Default::default()
        },
        gpus: vec![GpuStats {
            temperature_c: 70.0,
            utilization_percent: 60.0,
            ..Default::default()
        }],
        smart: vec![],
    }
}

fn rule(
    name: &str,
    metric: &str,
    op: &str,
    threshold: f64,
    duration: u64,
    cooldown: u64,
) -> AlertRule {
    AlertRule {
        name: name.into(),
        metric: metric.into(),
        op: op.into(),
        threshold,
        duration_secs: duration,
        cooldown_secs: cooldown,
    }
}

#[test]
fn extract_metric_covers_all_sources() {
    let s = snapshot(90.0);
    assert_eq!(extract_metric("cpu_usage", &s), Some(90.0));
    assert_eq!(extract_metric("mem_usage_percent", &s), Some(50.0));
    assert_eq!(extract_metric("swap_usage_percent", &s), Some(25.0));
    assert_eq!(extract_metric("load_avg_1", &s), Some(3.0));
    assert_eq!(extract_metric("cpu_temperature", &s), Some(80.0));
    assert_eq!(extract_metric("disk_usage_percent", &s), Some(95.0));
    assert_eq!(extract_metric("gpu_temperature", &s), Some(70.0));
    assert_eq!(extract_metric("gpu_utilization", &s), Some(60.0));
    assert_eq!(extract_metric("unknown", &s), None);
}

#[test]
fn swap_metric_none_when_no_swap() {
    let mut s = snapshot(10.0);
    s.ram.swap_total = 0;
    assert_eq!(extract_metric("swap_usage_percent", &s), None);
}

#[test]
fn compare_operators() {
    assert!(compare(90.0, ">", 80.0));
    assert!(!compare(80.0, ">", 80.0));
    assert!(compare(80.0, ">=", 80.0));
    assert!(compare(5.0, "<", 10.0));
    assert!(compare(10.0, "<=", 10.0));
    assert!(!compare(1.0, "??", 0.0));
}

#[test]
fn duration_gates_firing() {
    let mut engine = AlertEngine::new(vec![rule("hot", "cpu_usage", ">", 80.0, 30, 300)]);
    let t0 = Instant::now();
    // Breached but not yet sustained for 30s.
    assert!(engine.evaluate(&snapshot(90.0), t0).is_empty());
    assert!(
        engine
            .evaluate(&snapshot(90.0), t0 + Duration::from_secs(10))
            .is_empty()
    );
    // Sustained past the duration → fires once.
    let events = engine.evaluate(&snapshot(90.0), t0 + Duration::from_secs(31));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].state, AlertState::Firing);
    assert_eq!(events[0].rule_name, "hot");
    // Still breached + already firing → no duplicate.
    assert!(
        engine
            .evaluate(&snapshot(90.0), t0 + Duration::from_secs(40))
            .is_empty()
    );
}

#[test]
fn resolves_then_cooldown_suppresses_refire() {
    // duration 0 → fire as soon as breached; cooldown 300s.
    let mut engine = AlertEngine::new(vec![rule("hot", "cpu_usage", ">", 80.0, 0, 300)]);
    let t0 = Instant::now();
    let fired = engine.evaluate(&snapshot(90.0), t0);
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].state, AlertState::Firing);

    // Recovery emits a Resolved event.
    let resolved = engine.evaluate(&snapshot(10.0), t0 + Duration::from_secs(20));
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].state, AlertState::Resolved);

    // Re-breach within the cooldown window does NOT re-fire.
    assert!(
        engine
            .evaluate(&snapshot(90.0), t0 + Duration::from_secs(30))
            .is_empty()
    );
    // After cooldown elapses, it fires again.
    let refired = engine.evaluate(&snapshot(90.0), t0 + Duration::from_secs(400));
    assert_eq!(refired.len(), 1);
    assert_eq!(refired[0].state, AlertState::Firing);
}

#[test]
fn empty_engine_emits_nothing() {
    let mut engine = AlertEngine::new(vec![]);
    assert!(engine.is_empty());
    assert!(engine.evaluate(&snapshot(99.0), Instant::now()).is_empty());
}
