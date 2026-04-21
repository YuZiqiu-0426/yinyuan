use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use crate::printer::cprintln;

use super::ConsoleState;

/// Debounce window for grouping `CommandResult` events that share one `request_id` (e.g. broadcast).
const CMD_RESULT_DEBOUNCE_MS: u64 = 250;

#[derive(Clone)]
struct ClientCommandOutcome {
    exit_code: i64,
    stdout: String,
    stderr: String,
    duration_ms: u64,
}

struct CommandBatch {
    group: String,
    by_client: BTreeMap<String, ClientCommandOutcome>,
}

#[derive(Default)]
pub(crate) struct CommandResultAggregator {
    batches: HashMap<String, CommandBatch>,
    debounce_generation: HashMap<String, u64>,
}

impl CommandResultAggregator {
    fn merge_and_bump_generation(
        &mut self,
        request_id: &str,
        group: &str,
        client: &str,
        exit_code: i64,
        stdout: String,
        stderr: String,
        duration_ms: u64,
    ) -> u64 {
        let gen = self.debounce_generation.entry(request_id.to_string()).or_insert(0);
        *gen += 1;
        let current = *gen;
        let batch = self.batches.entry(request_id.to_string()).or_insert_with(|| CommandBatch {
            group: String::new(),
            by_client: BTreeMap::new(),
        });
        if batch.group.is_empty() || batch.group == "unknown" {
            batch.group = group.to_string();
        }
        batch.by_client.insert(
            client.to_string(),
            ClientCommandOutcome {
                exit_code,
                stdout,
                stderr,
                duration_ms,
            },
        );
        current
    }

    fn take_if_current(&mut self, request_id: &str, expected_gen: u64) -> Option<CommandBatch> {
        if self.debounce_generation.get(request_id).copied() != Some(expected_gen) {
            return None;
        }
        self.debounce_generation.remove(request_id);
        self.batches.remove(request_id)
    }

    fn take_all_batches(&mut self) -> Vec<(String, CommandBatch)> {
        let keys: Vec<String> = self.batches.keys().cloned().collect();
        let mut out = Vec::new();
        for k in keys {
            self.debounce_generation.remove(&k);
            if let Some(b) = self.batches.remove(&k) {
                out.push((k, b));
            }
        }
        out
    }
}

impl ConsoleState {
    pub(crate) fn merge_command_result_snapshot(
        &self,
        request_id: &str,
        group: &str,
        client: &str,
        exit_code: i64,
        stdout: String,
        stderr: String,
        duration_ms: u64,
    ) -> u64 {
        self.command_results
            .lock()
            .expect("lock command result aggregation")
            .merge_and_bump_generation(request_id, group, client, exit_code, stdout, stderr, duration_ms)
    }

    pub(crate) fn flush_command_results_if_current(&self, request_id: &str, expected_gen: u64) {
        let batch = self
            .command_results
            .lock()
            .expect("lock command result aggregation")
            .take_if_current(request_id, expected_gen);
        if let Some(b) = batch {
            print_merged_command_results(request_id, &b.group, &b.by_client);
        }
    }

    /// Used when the CLI exits right after a command (e.g. `y2m send command`) so output is not lost to debounce.
    pub(crate) fn flush_all_pending_command_results(&self) {
        let taken = self.command_results.lock().expect("lock command result aggregation").take_all_batches();
        for (rid, batch) in taken {
            print_merged_command_results(&rid, &batch.group, &batch.by_client);
        }
    }

    pub(crate) fn reset_command_result_aggregation(&self) {
        *self.command_results.lock().expect("lock command result aggregation") = CommandResultAggregator::default();
    }
}

pub(crate) fn schedule_command_result_flush(state: Arc<ConsoleState>, request_id: String, generation: u64) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(CMD_RESULT_DEBOUNCE_MS)).await;
        state.flush_command_results_if_current(&request_id, generation);
    });
}

fn print_merged_command_results(
    request_id: &str,
    group: &str,
    by_client: &BTreeMap<String, ClientCommandOutcome>,
) {
    cprintln!(
        "[{group}] 命令结果汇总 (request_id={request_id}, {} 个客户端)",
        by_client.len()
    );
    for (client, r) in by_client {
        print_one_client_outcome(group, client, r);
    }
}

fn print_one_client_outcome(group: &str, client: &str, r: &ClientCommandOutcome) {
    cprintln!(
        "[{group}][{client}] 命令结果 (exit={}, {}ms)",
        r.exit_code, r.duration_ms
    );
    if !r.stdout.is_empty() {
        cprintln!("  {}", r.stdout.replace('\n', "\n  "));
    }
    if !r.stderr.is_empty() {
        cprintln!("  [stderr] {}", r.stderr.replace('\n', "\n  [stderr] "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregator_merge_bumps_generation_and_take_respects_stale() {
        let mut a = CommandResultAggregator::default();
        let g1 = a.merge_and_bump_generation("r1", "g", "alice", 0, "a".into(), "".into(), 1);
        let g2 = a.merge_and_bump_generation("r1", "g", "bob", 0, "b".into(), "".into(), 2);
        assert_eq!(g1, 1);
        assert_eq!(g2, 2);
        assert!(a.take_if_current("r1", 1).is_none());
        let batch = a.take_if_current("r1", 2).expect("batch");
        assert_eq!(batch.by_client.len(), 2);
        assert_eq!(batch.by_client.get("alice").unwrap().stdout, "a");
        assert_eq!(batch.by_client.get("bob").unwrap().stdout, "b");
    }
}
