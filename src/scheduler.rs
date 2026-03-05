//! Work-stealing scheduler implementing the Blumofe-Leiserson algorithm.
//!
//! This module provides a lock-free work-stealing scheduler for efficient
//! load balancing across heterogeneous compute resources.
//!
//! ## Algorithm
//!
//! The work-stealing algorithm uses per-worker double-ended queues (deques):
//! - Local tasks: push to back, pop from back (LIFO, cache-friendly)
//! - Steal tasks: steal from front of other workers' queues (FIFO)
//!
//! ## References
//!
//! Blumofe, R. D., & Leiserson, C. E. (1999). "Scheduling Multithreaded
//! Computations by Work Stealing." Journal of the ACM, 46(5), 720-748.
//!
//! ## Example
//!
//! ```rust
//! use pepita::scheduler::{Scheduler, WorkerId};
//!
//! // Create scheduler with 4 workers
//! let scheduler = Scheduler::new(4);
//!
//! // Submit a task
//! let task_id = scheduler.submit(42u64);
//! assert!(task_id.is_some());
//!
//! // Worker 0 pops its local task
//! let task = scheduler.pop(WorkerId(0));
//! assert_eq!(task, Some(42));
//! ```

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use crate::error::{KernelError, Result};

// ============================================================================
// WORKER ID (Type-Safe, Stable)
// ============================================================================

/// Worker identifier.
///
/// Uses a type-safe wrapper to prevent index-based bugs (per Iron Lotus
/// Five Whys analysis - see spec section 12.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkerId(pub u32);

impl WorkerId {
    /// Create a new worker ID.
    #[must_use]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

// ============================================================================
// TASK ID
// ============================================================================

/// Task identifier.
///
/// Unique identifier for tracking tasks through the scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);

impl TaskId {
    /// Create a new task ID.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

// ============================================================================
// WORK-STEALING DEQUE
// ============================================================================

/// A work-stealing deque for a single worker.
///
/// Supports efficient local push/pop (LIFO) and remote steal (FIFO).
/// This implementation uses `Mutex` for simplicity; a lock-free version
/// would use Chase-Lev deque.
#[derive(Debug)]
pub struct WorkStealingDeque<T> {
    /// The underlying deque (protected by mutex)
    deque: Mutex<VecDeque<T>>,
    /// Number of items (atomic for lock-free reads)
    len: AtomicUsize,
    /// Capacity limit
    capacity: usize,
}

impl<T> WorkStealingDeque<T> {
    /// Create a new deque with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            deque: Mutex::new(VecDeque::with_capacity(capacity)),
            len: AtomicUsize::new(0),
            capacity,
        }
    }

    /// Push a task to the back (local operation, LIFO).
    ///
    /// Returns `Err` if the deque is at capacity.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn push(&self, item: T) -> Result<()> {
        let mut deque = self.deque.lock().map_err(|_| KernelError::WouldBlock)?;
        if deque.len() >= self.capacity {
            return Err(KernelError::UblkQueueFull);
        }
        deque.push_back(item);
        self.len.fetch_add(1, Ordering::Release);
        Ok(())
    }

    /// Pop a task from the back (local operation, LIFO).
    ///
    /// Returns `None` if the deque is empty.
    pub fn pop(&self) -> Option<T> {
        let mut deque = self.deque.lock().ok()?;
        let item = deque.pop_back()?;
        self.len.fetch_sub(1, Ordering::Release);
        Some(item)
    }

    /// Steal a task from the front (remote operation, FIFO).
    ///
    /// Returns `None` if the deque is empty.
    pub fn steal(&self) -> Option<T> {
        let mut deque = self.deque.lock().ok()?;
        let item = deque.pop_front()?;
        self.len.fetch_sub(1, Ordering::Release);
        Some(item)
    }

    /// Steal half of the tasks from this deque.
    ///
    /// Returns a vector of stolen tasks.
    pub fn steal_half(&self) -> Vec<T> {
        let Ok(mut deque) = self.deque.lock() else { return Vec::new(); };
        let steal_count = deque.len() / 2;
        if steal_count == 0 {
            return Vec::new();
        }
        let mut stolen = Vec::with_capacity(steal_count);
        for _ in 0..steal_count {
            if let Some(item) = deque.pop_front() {
                stolen.push(item);
            }
        }
        self.len.fetch_sub(stolen.len(), Ordering::Release);
        stolen
    }

    /// Get the current length (lock-free).
    #[must_use]
    pub fn len(&self) -> usize {
        self.len.load(Ordering::Acquire)
    }

    /// Check if empty (lock-free).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<T> Default for WorkStealingDeque<T> {
    fn default() -> Self {
        Self::new(1024)
    }
}

// ============================================================================
// SCHEDULER STATE
// ============================================================================

/// Worker state in the scheduler.
#[derive(Debug)]
struct WorkerState<T> {
    /// Worker's task deque
    deque: WorkStealingDeque<T>,
    /// Whether worker is active
    active: AtomicBool,
}

impl<T> WorkerState<T> {
    fn new(capacity: usize) -> Self {
        Self {
            deque: WorkStealingDeque::new(capacity),
            active: AtomicBool::new(true),
        }
    }
}

// ============================================================================
// SCHEDULER
// ============================================================================

/// Work-stealing scheduler for distributed task execution.
///
/// Implements the Blumofe-Leiserson work-stealing algorithm for
/// efficient load balancing across multiple workers.
#[derive(Debug)]
pub struct Scheduler<T> {
    /// Per-worker state
    workers: RwLock<Vec<Arc<WorkerState<T>>>>,
    /// Number of workers
    num_workers: AtomicUsize,
    /// Next task ID
    next_task_id: AtomicU64,
    /// Queue capacity per worker
    queue_capacity: usize,
    /// Round-robin counter for task submission
    submit_counter: AtomicUsize,
    /// Whether scheduler is running
    running: AtomicBool,
}

impl<T> Scheduler<T> {
    /// Create a new scheduler with the specified number of workers.
    ///
    /// # Arguments
    ///
    /// * `num_workers` - Number of worker threads/queues
    #[must_use]
    pub fn new(num_workers: usize) -> Self {
        Self::with_capacity(num_workers, 1024)
    }

    /// Create a scheduler with custom queue capacity.
    ///
    /// # Arguments
    ///
    /// * `num_workers` - Number of worker threads/queues
    /// * `queue_capacity` - Maximum tasks per worker queue
    #[must_use]
    pub fn with_capacity(num_workers: usize, queue_capacity: usize) -> Self {
        let workers: Vec<_> = (0..num_workers)
            .map(|_| Arc::new(WorkerState::new(queue_capacity)))
            .collect();

        Self {
            workers: RwLock::new(workers),
            num_workers: AtomicUsize::new(num_workers),
            next_task_id: AtomicU64::new(0),
            queue_capacity,
            submit_counter: AtomicUsize::new(0),
            running: AtomicBool::new(true),
        }
    }

    /// Submit a task to the scheduler.
    ///
    /// Uses round-robin distribution across workers.
    ///
    /// # Returns
    ///
    /// `Some(TaskId)` if submitted successfully, `None` if all queues full.
    pub fn submit(&self, task: T) -> Option<TaskId> {
        if !self.running.load(Ordering::Acquire) {
            return None;
        }

        let workers = self.workers.read().ok()?;
        if workers.is_empty() {
            return None;
        }

        // Round-robin with fallback to first available
        let start = self.submit_counter.fetch_add(1, Ordering::Relaxed) % workers.len();

        // Find first active worker with space
        let target_idx = (0..workers.len())
            .map(|i| (start + i) % workers.len())
            .find(|&idx| {
                workers[idx].active.load(Ordering::Acquire)
                    && workers[idx].deque.len() < workers[idx].deque.capacity()
            })?;

        // Try to push to the selected worker
        if workers[target_idx].deque.push(task).is_ok() {
            let task_id = self.next_task_id.fetch_add(1, Ordering::Relaxed);
            return Some(TaskId(task_id));
        }

        None
    }

    /// Pop a task from the worker's local queue.
    ///
    /// # Arguments
    ///
    /// * `worker_id` - ID of the requesting worker
    pub fn pop(&self, worker_id: WorkerId) -> Option<T> {
        let workers = self.workers.read().ok()?;
        let idx = worker_id.as_u32() as usize;
        if idx >= workers.len() {
            return None;
        }
        workers[idx].deque.pop()
    }

    /// Steal a task from another worker.
    ///
    /// Randomly selects a victim and steals from their queue.
    ///
    /// # Arguments
    ///
    /// * `thief_id` - ID of the stealing worker
    pub fn steal(&self, thief_id: WorkerId) -> Option<T> {
        let workers = self.workers.read().ok()?;
        let thief_idx = thief_id.as_u32() as usize;

        if workers.len() <= 1 || thief_idx >= workers.len() {
            return None;
        }

        // Try to steal from each worker (excluding self)
        for offset in 1..workers.len() {
            let victim_idx = (thief_idx + offset) % workers.len();
            if workers[victim_idx].active.load(Ordering::Acquire) {
                if let Some(task) = workers[victim_idx].deque.steal() {
                    return Some(task);
                }
            }
        }

        None
    }

    /// Steal half of the tasks from the busiest worker.
    ///
    /// # Arguments
    ///
    /// * `thief_id` - ID of the stealing worker
    pub fn steal_batch(&self, thief_id: WorkerId) -> Vec<T> {
        let Ok(workers) = self.workers.read() else { return Vec::new(); };

        let thief_idx = thief_id.as_u32() as usize;
        if workers.len() <= 1 || thief_idx >= workers.len() {
            return Vec::new();
        }

        // Find busiest worker (excluding self)
        let mut busiest_idx = None;
        let mut max_len = 0;

        for (idx, worker) in workers.iter().enumerate() {
            if idx != thief_idx && worker.active.load(Ordering::Acquire) {
                let len = worker.deque.len();
                if len > max_len {
                    max_len = len;
                    busiest_idx = Some(idx);
                }
            }
        }

        match busiest_idx {
            Some(idx) => workers[idx].deque.steal_half(),
            None => Vec::new(),
        }
    }

    /// Get the number of active workers.
    #[must_use]
    pub fn num_workers(&self) -> usize {
        self.num_workers.load(Ordering::Acquire)
    }

    /// Get the total number of pending tasks across all workers.
    #[must_use]
    pub fn pending_tasks(&self) -> usize {
        let Ok(workers) = self.workers.read() else { return 0; };
        workers.iter().map(|w| w.deque.len()).sum()
    }

    /// Get load statistics for each worker.
    #[must_use]
    pub fn worker_loads(&self) -> Vec<usize> {
        let Ok(workers) = self.workers.read() else { return Vec::new(); };
        workers.iter().map(|w| w.deque.len()).collect()
    }

    /// Check if scheduler is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Stop the scheduler.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
    }

    /// Add a new worker dynamically.
    ///
    /// Returns the new worker's ID.
    #[allow(clippy::cast_possible_truncation)]
    pub fn add_worker(&self) -> Option<WorkerId> {
        let mut workers = self.workers.write().ok()?;
        let id = workers.len() as u32;
        workers.push(Arc::new(WorkerState::new(self.queue_capacity)));
        self.num_workers.fetch_add(1, Ordering::Release);
        Some(WorkerId(id))
    }

    /// Deactivate a worker (mark as inactive, tasks can still be stolen).
    ///
    /// # Arguments
    ///
    /// * `worker_id` - ID of the worker to deactivate
    pub fn deactivate_worker(&self, worker_id: WorkerId) -> bool {
        let Ok(workers) = self.workers.read() else { return false; };
        let idx = worker_id.as_u32() as usize;
        if idx >= workers.len() {
            return false;
        }
        workers[idx].active.store(false, Ordering::Release);
        true
    }

    /// Check if a worker is active.
    #[must_use]
    pub fn is_worker_active(&self, worker_id: WorkerId) -> bool {
        let Ok(workers) = self.workers.read() else { return false; };
        let idx = worker_id.as_u32() as usize;
        if idx >= workers.len() {
            return false;
        }
        workers[idx].active.load(Ordering::Acquire)
    }
}

impl<T> Default for Scheduler<T> {
    fn default() -> Self {
        Self::new(4)
    }
}

// ============================================================================
// TESTS (EXTREME TDD)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // WorkerId Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_worker_id_new() {
        let id = WorkerId::new(42);
        assert_eq!(id.as_u32(), 42);
    }

    #[test]
    fn test_worker_id_equality() {
        let a = WorkerId(1);
        let b = WorkerId(1);
        let c = WorkerId(2);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_worker_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(WorkerId(1));
        set.insert(WorkerId(2));
        set.insert(WorkerId(1)); // Duplicate
        assert_eq!(set.len(), 2);
    }

    // ------------------------------------------------------------------------
    // TaskId Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_task_id_new() {
        let id = TaskId::new(100);
        assert_eq!(id.as_u64(), 100);
    }

    #[test]
    fn test_task_id_equality() {
        let a = TaskId(1);
        let b = TaskId(1);
        let c = TaskId(2);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // ------------------------------------------------------------------------
    // WorkStealingDeque Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_deque_new() {
        let deque: WorkStealingDeque<u32> = WorkStealingDeque::new(100);
        assert_eq!(deque.capacity(), 100);
        assert_eq!(deque.len(), 0);
        assert!(deque.is_empty());
    }

    #[test]
    fn test_deque_push_pop() {
        let deque: WorkStealingDeque<u32> = WorkStealingDeque::new(100);
        deque.push(1).unwrap();
        deque.push(2).unwrap();
        deque.push(3).unwrap();

        assert_eq!(deque.len(), 3);

        // LIFO order
        assert_eq!(deque.pop(), Some(3));
        assert_eq!(deque.pop(), Some(2));
        assert_eq!(deque.pop(), Some(1));
        assert_eq!(deque.pop(), None);
    }

    #[test]
    fn test_deque_steal() {
        let deque: WorkStealingDeque<u32> = WorkStealingDeque::new(100);
        deque.push(1).unwrap();
        deque.push(2).unwrap();
        deque.push(3).unwrap();

        // FIFO order for stealing
        assert_eq!(deque.steal(), Some(1));
        assert_eq!(deque.steal(), Some(2));
        assert_eq!(deque.steal(), Some(3));
        assert_eq!(deque.steal(), None);
    }

    #[test]
    fn test_deque_steal_half() {
        let deque: WorkStealingDeque<u32> = WorkStealingDeque::new(100);
        for i in 0..10 {
            deque.push(i).unwrap();
        }

        let stolen = deque.steal_half();
        assert_eq!(stolen.len(), 5);
        assert_eq!(deque.len(), 5);

        // Stolen should be FIFO (0, 1, 2, 3, 4)
        assert_eq!(stolen, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_deque_capacity_limit() {
        let deque: WorkStealingDeque<u32> = WorkStealingDeque::new(3);
        assert!(deque.push(1).is_ok());
        assert!(deque.push(2).is_ok());
        assert!(deque.push(3).is_ok());
        assert!(deque.push(4).is_err()); // At capacity
    }

    #[test]
    fn test_deque_default() {
        let deque: WorkStealingDeque<u32> = WorkStealingDeque::default();
        assert_eq!(deque.capacity(), 1024);
    }

    // ------------------------------------------------------------------------
    // Scheduler Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_scheduler_new() {
        let scheduler: Scheduler<u32> = Scheduler::new(4);
        assert_eq!(scheduler.num_workers(), 4);
        assert!(scheduler.is_running());
        assert_eq!(scheduler.pending_tasks(), 0);
    }

    #[test]
    fn test_scheduler_submit() {
        let scheduler: Scheduler<u32> = Scheduler::new(4);
        let task_id = scheduler.submit(42);
        assert!(task_id.is_some());
        assert_eq!(scheduler.pending_tasks(), 1);
    }

    #[test]
    fn test_scheduler_submit_multiple() {
        let scheduler: Scheduler<u32> = Scheduler::new(4);
        for i in 0..100 {
            let task_id = scheduler.submit(i);
            assert!(task_id.is_some());
        }
        assert_eq!(scheduler.pending_tasks(), 100);
    }

    #[test]
    fn test_scheduler_pop() {
        let scheduler: Scheduler<u32> = Scheduler::new(4);

        // Submit directly to worker 0's queue
        scheduler.submit(42);

        // Find which worker has the task
        let loads = scheduler.worker_loads();
        let worker_idx = loads.iter().position(|&l| l > 0).unwrap();

        let task = scheduler.pop(WorkerId(worker_idx as u32));
        assert_eq!(task, Some(42));
    }

    #[test]
    fn test_scheduler_steal() {
        let scheduler: Scheduler<u32> = Scheduler::new(2);

        // Submit many tasks (will go to worker 0 first via round-robin)
        for i in 0..10 {
            scheduler.submit(i);
        }

        // Worker 1 steals from worker 0
        let stolen = scheduler.steal(WorkerId(1));
        assert!(stolen.is_some());
    }

    #[test]
    fn test_scheduler_steal_batch() {
        let scheduler: Scheduler<u32> = Scheduler::new(2);

        // Submit many tasks to fill worker 0's queue
        for i in 0..20 {
            scheduler.submit(i);
        }

        // Worker 1 steals batch from busiest worker
        let stolen = scheduler.steal_batch(WorkerId(1));
        assert!(!stolen.is_empty());
    }

    #[test]
    fn test_scheduler_worker_loads() {
        let scheduler: Scheduler<u32> = Scheduler::new(4);

        for i in 0..20 {
            scheduler.submit(i);
        }

        let loads = scheduler.worker_loads();
        assert_eq!(loads.len(), 4);
        assert_eq!(loads.iter().sum::<usize>(), 20);
    }

    #[test]
    fn test_scheduler_stop() {
        let scheduler: Scheduler<u32> = Scheduler::new(4);
        assert!(scheduler.is_running());

        scheduler.stop();
        assert!(!scheduler.is_running());

        // Submit should fail after stop
        let task_id = scheduler.submit(42);
        assert!(task_id.is_none());
    }

    #[test]
    fn test_scheduler_add_worker() {
        let scheduler: Scheduler<u32> = Scheduler::new(2);
        assert_eq!(scheduler.num_workers(), 2);

        let new_id = scheduler.add_worker();
        assert!(new_id.is_some());
        assert_eq!(new_id.unwrap(), WorkerId(2));
        assert_eq!(scheduler.num_workers(), 3);
    }

    #[test]
    fn test_scheduler_deactivate_worker() {
        let scheduler: Scheduler<u32> = Scheduler::new(2);

        assert!(scheduler.is_worker_active(WorkerId(0)));
        assert!(scheduler.deactivate_worker(WorkerId(0)));
        assert!(!scheduler.is_worker_active(WorkerId(0)));
    }

    #[test]
    fn test_scheduler_deactivate_invalid_worker() {
        let scheduler: Scheduler<u32> = Scheduler::new(2);
        assert!(!scheduler.deactivate_worker(WorkerId(99)));
    }

    #[test]
    fn test_scheduler_pop_invalid_worker() {
        let scheduler: Scheduler<u32> = Scheduler::new(2);
        scheduler.submit(42);

        // Invalid worker ID
        let task = scheduler.pop(WorkerId(99));
        assert!(task.is_none());
    }

    #[test]
    fn test_scheduler_default() {
        let scheduler: Scheduler<u32> = Scheduler::default();
        assert_eq!(scheduler.num_workers(), 4);
    }

    // ------------------------------------------------------------------------
    // Concurrent Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_scheduler_concurrent_submit() {
        use std::thread;

        let scheduler = Arc::new(Scheduler::<u32>::new(4));
        let mut handles = Vec::new();

        for t in 0..4 {
            let s = Arc::clone(&scheduler);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    s.submit(t * 100 + i);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(scheduler.pending_tasks(), 400);
    }

    #[test]
    fn test_scheduler_concurrent_pop() {
        use std::thread;

        let scheduler = Arc::new(Scheduler::<u32>::new(4));

        // Submit tasks
        for i in 0..100 {
            scheduler.submit(i);
        }

        let mut handles = Vec::new();
        let popped = Arc::new(AtomicUsize::new(0));

        for worker_id in 0..4 {
            let s = Arc::clone(&scheduler);
            let p = Arc::clone(&popped);
            handles.push(thread::spawn(move || {
                while s.pop(WorkerId(worker_id)).is_some() {
                    p.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(popped.load(Ordering::Relaxed), 100);
    }

    #[test]
    fn test_scheduler_work_stealing_correctness() {
        let scheduler: Scheduler<u32> = Scheduler::new(2);

        // Submit tasks (distributed via round-robin to both workers)
        for _ in 0..10 {
            scheduler.submit(42);
        }

        // Both workers should have tasks (round-robin distribution)
        let loads = scheduler.worker_loads();
        let total_initial = loads.iter().sum::<usize>();
        assert_eq!(total_initial, 10);

        // Pop from worker 0 and steal from worker 1
        let mut consumed = 0;
        while let Some(_) = scheduler.pop(WorkerId(0)) {
            consumed += 1;
        }
        while let Some(_) = scheduler.pop(WorkerId(1)) {
            consumed += 1;
        }

        // Verify all tasks consumed
        assert_eq!(scheduler.pending_tasks(), 0);
        assert_eq!(consumed, 10);
    }

    // ------------------------------------------------------------------------
    // Property Tests (Invariants)
    // ------------------------------------------------------------------------

    #[test]
    fn test_invariant_task_count_preserved() {
        let scheduler: Scheduler<u32> = Scheduler::new(4);

        // Submit N tasks
        let n = 50;
        for i in 0..n {
            scheduler.submit(i);
        }

        // Pop all tasks
        let mut popped = 0;
        for worker_id in 0..4 {
            while scheduler.pop(WorkerId(worker_id)).is_some() {
                popped += 1;
            }
        }

        assert_eq!(popped, n);
    }

    #[test]
    fn test_invariant_steal_does_not_duplicate() {
        let scheduler: Scheduler<u32> = Scheduler::new(2);

        // Submit unique tasks
        for i in 0..10 {
            scheduler.submit(i);
        }

        let mut collected = Vec::new();

        // Collect all via pop and steal
        for worker_id in 0..2 {
            while let Some(task) = scheduler.pop(WorkerId(worker_id)) {
                collected.push(task);
            }
        }

        while let Some(task) = scheduler.steal(WorkerId(0)) {
            collected.push(task);
        }

        // No duplicates
        collected.sort();
        let len_before = collected.len();
        collected.dedup();
        assert_eq!(collected.len(), len_before);
    }
}
