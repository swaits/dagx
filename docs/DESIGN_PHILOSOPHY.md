# Design Philosophy: Primitives as Scheduler

dagx takes an unconventional approach to task orchestration: **there is no traditional scheduler**. Instead of complex scheduling logic managing when tasks run, the entire system is built on communication and synchronization primitives that do the scheduling themselves.

## How It Works

Traditional DAG executors contain substantial scheduling code—algorithms that track task states, manage dependencies, coordinate execution order, and handle synchronization. This code is complex, error-prone, and difficult to verify.

dagx eliminates this entirely:

1. **Wire up tasks with primitives**: During DAG construction, tasks are connected using channels and organized into topological layers based on their dependencies
2. **Start everything simultaneously**: When you call `run()`, all tasks spawn at once—there's no scheduler deciding when each task should start
3. **Let the primitives handle coordination**: Channels naturally enforce execution order—tasks wait on their input channels until upstream tasks send data. No custom orchestration needed.
4. **Runtime joins on completion**: The runtime simply spawns all tasks and waits for the completion channel to close. That's it.

## The Implementation

Under the hood, dagx uses:

- **Oneshot channels**: Each edge in the DAG gets a `futures::channel::oneshot` - producer sends once, consumer receives once
- **Ownership model**: Tasks take ownership (`self`) and are consumed during execution - no Mutex needed for task state
- **Direct data flow**: Outputs flow from producer to consumer via channels, never stored in shared memory during execution
- **Fast-path optimization**: Single-task layers execute inline without spawning overhead

The core execution logic in `run()` is remarkably simple:

```rust
// Create oneshot channel for each edge
for (producer, consumer) in edges {
    let (tx, rx) = oneshot::channel();
    // Wire tx to producer, rx to consumer
}

// Execute layer by layer
for layer in layers {
    if layer.len() == 1 {
        // Fast path: Execute inline, no spawning overhead
        // IMPORTANT: Panic handling is required here!
        let result = task.execute().catch_unwind().await;
        // Convert panic to error to match spawned task behavior
    } else {
        // Parallel path: Spawn all tasks in layer
        for task in layer {
            spawner(async {
                let input = await_on_input_channels();  // Blocks until deps complete
                let output = task.run(input);           // Consumes task
                send_to_output_channels(output);        // Sends to dependents
            });
        }
        // Wait for layer completion
    }
}

// Tasks coordinate themselves via channels - no scheduler needed
```

No state machines. No task queues. No wake-up logic. No Mutex locks. Just channels doing what channels do, with an inline fast-path for sequential execution.

## Inline Execution Fast-Path

**Performance optimization for sequential workloads**: When a layer contains only a single task (common in deep chains and linear pipelines), dagx executes it inline rather than spawning it. This eliminates spawning overhead, context switching, and channel creation, resulting in 10-100x performance improvements for sequential patterns.

**Panic handling guarantee**: To maintain behavioral consistency between inline and spawned execution, panics in inline tasks are caught using `FutureExt::catch_unwind()` and converted to errors. This matches the behavior of all major async runtimes (Tokio, async-std, smol, embassy-rs), which catch panics in spawned tasks and convert them to `JoinError` or equivalent.

**Why this matters**:

- **Spawned tasks** (layer.len() > 1): Runtime catches panics automatically → becomes error
- **Inline tasks** (layer.len() == 1): We catch panics manually → becomes error
- **Result**: Identical behavior regardless of execution path

This ensures your code behaves the same whether a task runs inline or spawned, making dagx's optimizations transparent and predictable.

## Benefits

**Simplicity**: The runtime is straightforward: create channels, spawn tasks, let them coordinate via awaiting. No complex scheduler code to maintain, debug, or optimize.

**Reliability**: Built on battle-tested primitives (oneshot channels, async/await) from Rust's standard library and the futures crate. These have been used in production by thousands of projects and are orders of magnitude more reliable than custom scheduling logic.

**Bug resistance**: Fewer moving parts means fewer places for bugs to hide. The type system enforces correct wiring at compile time. Channels handle synchronization. The ownership model prevents data races. What's left to break?

**Performance**: Near zero-overhead. No Mutex locks during execution. Arc reference counting for efficient fanout (atomic operations, not locks). Oneshot channels are often lock-free. Tasks start as soon as their dependencies complete - maximum parallelism.

**Auditability**: Want to verify correctness? Check the channel wiring, verify tasks await their inputs, done. No need to trace through complex state machine transitions or wake-up cascades.

## The Insight

The key insight is that **dependencies ARE the schedule**. If task B depends on task A's output, a channel naturally enforces that B waits for A. The dependency graph already encodes all the scheduling information—we just need to wire up channels to match it.

This is dagx's core philosophy: leverage the type system for correctness, use primitives for coordination, and let the compiler optimize everything else away.

## Measured Overhead

How much overhead does this approach actually add? Benchmarks on an AMD 7840U (Zen 4 laptop CPU) show:

**DAG Construction**:

- Empty DAG creation: **~20 nanoseconds**
- Adding tasks: **~100 nanoseconds per task**
- Building a 10,000-task DAG: **~1.0 milliseconds** (100 ns/task)

**Execution Overhead** (framework coordination, excluding actual task work):

- Sequential workloads: **~790 nanoseconds per task** (inline execution fast-path)
- Parallel workloads: **~1.3 microseconds per task**
- 100-task sequential chain: **~79 microseconds total**
- 100 independent tasks: **~122 microseconds total**
- 10,000 independent tasks: **~12.7 milliseconds total**

**Scaling**: Sub-microsecond per-task overhead across all workload patterns. Linear scaling verified to 10k+ tasks.

**Comparison**: dagx is **1.04-129x faster** than dagrs (v0.5) across all benchmark patterns (see [comparisons](COMPARISONS.md)).

The primitives-as-scheduler approach with inline fast-path optimization delivers exceptional performance: coordination overhead is sub-microsecond per task, and for real-world workloads where tasks do meaningful work (I/O, computation, etc.), framework overhead is negligible—typically well under 1% of total execution time.
