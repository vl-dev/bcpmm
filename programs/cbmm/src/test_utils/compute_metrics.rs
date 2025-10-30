use std::sync::Mutex;
use std::collections::HashMap;
use litesvm::LiteSVM;
use solana_sdk::transaction::Transaction;

// Global collector for compute unit metrics
struct ComputeMetrics {
    calls: HashMap<String, Vec<u64>>,
}

impl ComputeMetrics {
    fn new() -> Self {
        Self {
            calls: HashMap::new(),
        }
    }

    fn record(&mut self, instruction_name: &str, compute_units: u64) {
        self.calls
            .entry(instruction_name.to_string())
            .or_insert_with(Vec::new)
            .push(compute_units);
    }

    fn print_report(&self) {
        println!("\n{}", "=".repeat(80));
        println!("COMPUTE UNIT USAGE REPORT");
        println!("{}", "=".repeat(80));

        let mut sorted: Vec<_> = self.calls.iter().collect();
        sorted.sort_by_key(|(name, _)| *name);

        for (name, values) in sorted {
            let count = values.len();
            let sum: u64 = values.iter().sum();
            let avg = sum as f64 / count as f64;
            let min = *values.iter().min().unwrap();
            let max = *values.iter().max().unwrap();

            // ANSI color codes: \x1b[32;1m = green and bold, \x1b[0m = reset
            println!("\n\x1b[32;1m{}\x1b[0m", name);
            println!("  Calls:   {}", count);
            println!("  Average: {:.0} CU", avg);
            println!("  Min:     {} CU", min);
            println!("  Max:     {} CU", max);
        }

        println!("\n{}", "=".repeat(80));
    }
}

static METRICS: Mutex<Option<ComputeMetrics>> = Mutex::new(None);

// Initialize the metrics collector
pub fn init_metrics() {
    let mut metrics = METRICS.lock().unwrap();
    *metrics = Some(ComputeMetrics::new());
}

// Record a measurement
pub fn record_compute_units(instruction_name: &str, compute_units: u64) {
    let mut metrics = METRICS.lock().unwrap();
    if let Some(m) = metrics.as_mut() {
        m.record(instruction_name, compute_units);
    }
}

// Print the final report
pub fn print_metrics_report() {
    let metrics = METRICS.lock().unwrap();
    if let Some(m) = metrics.as_ref() {
        m.print_report();
    }
}

// Helper function to send transaction and auto-record metrics
pub fn send_and_record(
    svm: &mut LiteSVM,
    tx: Transaction,
    instruction_name: &str,
) -> litesvm::types::TransactionResult {
    match svm.send_transaction(tx) {
        Ok(result) => {
            // Only record compute units if transaction succeeded
            record_compute_units(instruction_name, result.compute_units_consumed);
            Ok(result)
        }
        Err(err) => {
            // Return the error without recording metrics
            Err(err)
        }
    }
}
