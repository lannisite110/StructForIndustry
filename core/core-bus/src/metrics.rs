//! Prometheus text metrics for observability (Phase 5).

use crate::bus::BusStatsSnapshot;
use crate::scheduler::SchedulerStatsSnapshot;

pub fn render_prometheus(
    bus: &BusStatsSnapshot,
    scheduler: Option<&SchedulerStatsSnapshot>,
) -> String {
    let mut out = String::new();
    macro_rules! metric {
        ($name:expr, $help:expr, $val:expr) => {
            out.push_str(&format!("# HELP {} {}\n", $name, $help));
            out.push_str(&format!("# TYPE {} counter\n", $name));
            out.push_str(&format!("{} {}\n", $name, $val));
        };
    }
    metric!(
        "sfi_frames_received_total",
        "HAL frames ingested",
        bus.frames_received
    );
    metric!(
        "sfi_task_done_published_total",
        "task.done events published",
        bus.task_done_published
    );
    metric!(
        "sfi_spc_metrics_published_total",
        "spc.metrics events published",
        bus.spc_metrics_published
    );
    metric!(
        "sfi_mes_reports_sent_total",
        "MES reports POSTed",
        bus.mes_reports_sent
    );
    metric!(
        "sfi_plugin_health_published_total",
        "plugin.health events",
        bus.plugin_health_published
    );
    out.push_str("# HELP sfi_last_frame_id Last processed frame id\n");
    out.push_str("# TYPE sfi_last_frame_id gauge\n");
    out.push_str(&format!("sfi_last_frame_id {}\n", bus.last_frame_id));

    if let Some(s) = scheduler {
        metric!(
            "sfi_scheduler_tasks_dispatched_total",
            "Vision tasks dispatched",
            s.tasks_dispatched
        );
        metric!(
            "sfi_scheduler_tasks_completed_total",
            "Vision tasks completed",
            s.tasks_completed
        );
        metric!(
            "sfi_scheduler_tasks_failed_total",
            "Vision tasks failed",
            s.tasks_failed
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::BusStatsSnapshot;
    use crate::scheduler::SchedulerStatsSnapshot;

    #[test]
    fn renders_prometheus_counters() {
        let bus = BusStatsSnapshot {
            frames_received: 3,
            last_frame_id: 9,
            last_timestamp_ns: 0,
            task_done_published: 2,
            plugin_health_published: 0,
            mes_reports_sent: 1,
            spc_metrics_published: 2,
        };
        let text = render_prometheus(
            &bus,
            Some(&SchedulerStatsSnapshot {
                tasks_dispatched: 2,
                tasks_completed: 2,
                tasks_failed: 0,
                last_task_id: 2,
            }),
        );
        assert!(text.contains("sfi_frames_received_total 3"));
        assert!(text.contains("sfi_task_done_published_total 2"));
    }
}
