//! Demonstrates the work-stealing scheduler with a simple task pool.
//!
//! This example shows how to create a scheduler, submit tasks, and use
//! work-stealing to balance load across workers — all in userspace with
//! no kernel access required.
//!
//! Run with: `cargo run --example scheduler_demo`

use pepita::scheduler::{Scheduler, WorkerId};

fn main() {
    println!("=== Pepita Work-Stealing Scheduler Demo ===\n");

    // Create a scheduler with 4 workers, each with a queue capacity of 64.
    let scheduler = Scheduler::<u64>::with_capacity(4, 64);
    println!("Created scheduler with 4 workers (queue capacity: 64)\n");

    // Submit 10 tasks (plain u64 values representing workload IDs).
    println!("Submitting 10 tasks...");
    for i in 0..10 {
        if let Some(task_id) = scheduler.submit(i * 100) {
            println!("  Submitted workload {} -> TaskId({})", i * 100, task_id.as_u64());
        }
    }

    // Worker 0 pops tasks from its local queue (LIFO order).
    println!("\nWorker 0 popping local tasks:");
    while let Some(value) = scheduler.pop(WorkerId::new(0)) {
        println!("  Worker 0 got: {value}");
    }

    // Submit more tasks so other workers have work to steal.
    println!("\nSubmitting 8 more tasks...");
    for i in 10..18 {
        scheduler.submit(i * 100);
    }

    // Worker 3 steals from other workers' queues (FIFO order).
    println!("\nWorker 3 stealing from other workers:");
    for _ in 0..4 {
        if let Some(value) = scheduler.steal(WorkerId::new(3)) {
            println!("  Worker 3 stole: {value}");
        }
    }

    // Worker 2 batch-steals half from the busiest worker.
    let stolen = scheduler.steal_batch(WorkerId::new(2));
    println!("\nWorker 2 batch-stole {} tasks: {:?}", stolen.len(), stolen);

    // Show scheduler state.
    println!("\n--- Scheduler State ---");
    println!("  Workers:        {}", scheduler.num_workers());
    println!("  Pending tasks:  {}", scheduler.pending_tasks());
    println!("  Worker loads:   {:?}", scheduler.worker_loads());
    println!("  Running:        {}", scheduler.is_running());

    println!("\nDone.");
}
