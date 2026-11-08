//! Test binary for the two-store sync harness.

use sync_test_harness::{TwoStoreHarness, HarnessConfig, TestScenario};

#[cfg(feature = "native")]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    println!("Starting two-store sync protocol test harness...");

    let config = HarnessConfig::default();
    println!("Configured {} test scenarios", config.test_scenarios.len());

    let harness = TwoStoreHarness::new(config);
    let results = harness.run_all().await;

    println!("\n=== Test Results ===");
    let mut passed = 0;
    let mut failed = 0;

    for result in &results {
        let status = if result.passed { "PASS" } else { "FAIL" };
        if result.passed {
            passed += 1;
        } else {
            failed += 1;
        }
        println!("  [{}] {:?}: {}", status, result.scenario, result.details);
        if !result.passed {
            println!("      Store1 records: {}", result.store1_final_records.len());
            println!("      Store2 records: {}", result.store2_final_records.len());
        }
    }

    println!("\n=== Summary ===");
    println!("  Passed: {}", passed);
    println!("  Failed: {}", failed);
    println!("  Total:  {}", results.len());

    if failed > 0 {
        std::process::exit(1);
    }
}
