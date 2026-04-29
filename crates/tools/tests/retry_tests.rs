//! Verify scrape retry behaviour:
//! - Connection refused → fail fast (under 5s).
//! - Successful endpoint → returns OK quickly.

use std::time::Instant;
use tools::scrape::scrape_trains;

#[tokio::test(flavor = "multi_thread")]
async fn connection_refused_fails_fast() {
    // Port 9 (discard) is virtually never open and refuses cleanly on macOS/Linux.
    let start = Instant::now();
    let result = scrape_trains("http://127.0.0.1:9", "北京", "上海", "2026-05-01").await;
    let elapsed = start.elapsed();

    assert!(result.is_err(), "expected error on closed port");
    assert!(
        elapsed.as_secs() < 5,
        "fail-fast violated: took {:?}",
        elapsed
    );
    println!("connection_refused_fails_fast: {:?} -> {:?}", elapsed, result.err());
}
