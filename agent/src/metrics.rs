use std::collections::HashMap;

use chrono::Utc;
use shared::{DiskInfo, MemoryInfo, MetricPayload, NetworkInfo};
use sysinfo::{Disks, Networks, System};

/// Cumulative per-interface byte counters from the previous sample.
/// Maps interface name → (total_bytes_received, total_bytes_transmitted).
pub type NetworkBaseline = HashMap<String, (u64, u64)>;

/// Collect a full metric snapshot for `agent_id`.
///
/// `net_baseline` carries the previous network byte counters so the function
/// can compute a delta.  Pass `&mut None` on the first call; the caller must
/// persist the value across ticks so subsequent calls produce real deltas.
///
/// CPU measurement requires two samples separated by a short sleep
/// (`sysinfo::MINIMUM_CPU_UPDATE_INTERVAL`), so this function is async.
pub async fn collect_metrics(
    agent_id: &str,
    net_baseline: &mut Option<NetworkBaseline>,
) -> MetricPayload {
    let timestamp = Utc::now();

    // ── CPU ──────────────────────────────────────────────────────────────────
    // sysinfo requires two refreshes with a delay to produce meaningful usage %.
    let mut sys = System::new_all();
    sys.refresh_cpu_usage();
    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    sys.refresh_cpu_usage();

    let cpus = sys.cpus();
    let cpu_percent = if cpus.is_empty() {
        0.0_f64
    } else {
        cpus.iter().map(|c| c.cpu_usage() as f64).sum::<f64>() / cpus.len() as f64
    };

    // ── Memory ───────────────────────────────────────────────────────────────
    // new_all() already refreshed memory; no extra call needed.
    let used_mem = sys.used_memory();
    let total_mem = sys.total_memory();
    let memory = MemoryInfo {
        used_bytes: used_mem,
        total_bytes: total_mem,
        percent: if total_mem > 0 {
            used_mem as f64 / total_mem as f64 * 100.0
        } else {
            0.0
        },
    };

    // ── Disks ────────────────────────────────────────────────────────────────
    let disk_list = Disks::new_with_refreshed_list();
    let disks: Vec<DiskInfo> = disk_list
        .iter()
        .filter_map(|d| {
            let total = d.total_space();
            if total == 0 {
                return None; // skip zero-total volumes (avoids divide-by-zero)
            }
            let used = total.saturating_sub(d.available_space());
            Some(DiskInfo {
                mount_point: d.mount_point().to_string_lossy().into_owned(),
                used_bytes: used,
                total_bytes: total,
                percent: used as f64 / total as f64 * 100.0,
            })
        })
        .collect();

    // ── Network delta ────────────────────────────────────────────────────────
    // Read cumulative OS counters and subtract the previous sample to get a
    // per-interval delta.  The first call always returns (0, 0).
    let nets = Networks::new_with_refreshed_list();
    let current_totals: NetworkBaseline = nets
        .iter()
        .map(|(name, data)| {
            (
                name.clone(),
                (data.total_received(), data.total_transmitted()),
            )
        })
        .collect();

    let network = match net_baseline.as_ref() {
        None => NetworkInfo {
            bytes_in: 0,
            bytes_out: 0,
        },
        Some(prev) => {
            let (in_delta, out_delta) = current_totals.iter().fold(
                (0u64, 0u64),
                |acc, (iface, &(cur_in, cur_out))| {
                    if let Some(&(prev_in, prev_out)) = prev.get(iface) {
                        (
                            acc.0 + cur_in.saturating_sub(prev_in),
                            acc.1 + cur_out.saturating_sub(prev_out),
                        )
                    } else {
                        acc
                    }
                },
            );
            NetworkInfo {
                bytes_in: in_delta,
                bytes_out: out_delta,
            }
        }
    };

    *net_baseline = Some(current_totals);

    // ── Uptime ───────────────────────────────────────────────────────────────
    MetricPayload {
        agent_id: agent_id.to_owned(),
        timestamp,
        cpu_percent,
        memory,
        disks,
        network,
        uptime_seconds: System::uptime(),
    }
}
