# Design Philosophy: Primitives as Scheduler

## How It Works

1. **Wire up tasks with primitives**: During DAG construction, tasks are organized into topological layers based on their dependencies
2. **Start everything simultaneously**: When you call `run()`, all tasks in the first layer spawn at once—there's no scheduler deciding when each task should start
3. **Let the primitives handle coordination**: Layers naturally enforce execution order. No custom orchestration needed.
4. **Runtime joins on completion**: The runtime simply spawns all tasks and waits for them to finish. That's it.

## The Implementation

Under the hood, dagx uses:

- **Ownership model**: Tasks take ownership (`self`) and are consumed during execution - no Mutex needed for task state
- **Direct data flow**: Arc-wrapped outputs flow from producer(s) to consumer(s) through a store in the runner
- **Fast-path optimization**: Single-task layers execute inline without spawning overhead

## Inline Execution Fast-Path

**Performance optimization for sequential workloads**: When a layer contains only a single task (common in deep chains and linear pipelines), dagx executes it inline rather than spawning it. This eliminates spawning overhead and context switching, resulting in a 10-100x performance improvements for sequential patterns.

**Panic handling guarantee**: To maintain behavioral consistency between runtimes, as well as inline vs. spawned tasks, panics in tasks are caught using `FutureExt::catch_unwind()` and converted to errors. This ensures your code behaves the same whether a task runs inline or spawned, making dagx's optimizations transparent and predictable.

## Benefits

**Simplicity**: The runtime is straightforward: spawn tasks, coordinate via compile-time dependency guarantees. No complex scheduler code to maintain, debug, or optimize.

**Reliability**: Built on battle-tested primitives from Rust's standard library and the futures crate. These have been used in production by thousands of projects and are orders of magnitude more reliable than custom scheduling logic.

**Bug resistance**: Fewer moving parts means fewer places for bugs to hide. The type system enforces correct wiring at compile time. The ownership model prevents data races. What's left to break?

**Performance**: Near zero-overhead. No locks during execution. Arc reference counting for efficient fanout (atomic operations, not locks). Tasks start as soon as their dependencies complete - maximum parallelism.

**Auditability**: Want to verify correctness? Check the dependency wiring, verify tasks await their inputs, done. No need to trace through complex state machine transitions or wake-up cascades.

## The Insight

The key insight is that **dependencies ARE the schedule**. If task B depends on task A's output, the topological sort naturally enforces that B waits for A. The dependency graph already encodes all the scheduling information—we just need to wire outputs to match it.

This is dagx's core philosophy: leverage the type system for correctness, use primitives for coordination, and let the compiler optimize everything else away.

## Measured Overhead

How much overhead does this approach actually add? Benchmarks on an Intel i9-13950HX show:

**DAG Construction**:

- Empty DAG creation: **~7 nanoseconds**
- Adding tasks: **~5 nanoseconds per task**
- Building a 10,000-task DAG: **~865 microseconds** (100 ns/task)

**Scaling**: Sub-microsecond per-task overhead across all workload patterns. Linear scaling verified to 10k+ tasks.

The primitives-as-scheduler approach with inline fast-path optimization delivers exceptional performance: coordination overhead is sub-microsecond per task, and for real-world workloads where tasks do meaningful work (I/O, computation, etc.), framework overhead is negligible—typically well under 1% of total execution time.
