# Comparison with Similar Projects

The Rust ecosystem offers several DAG execution libraries, each optimized for different use cases. This comparison helps you choose the right tool for your needs.

## Developer Experience: API Comparison

The same task (compute 2 + 3) implemented across different libraries shows how API complexity varies:

**dagx** (Simple: macro + builder):

```rust
#[task]
impl Value { async fn run(&self) -> i32 { self.0 } }

#[task]
impl Add { async fn run(a: &i32, b: &i32) -> i32 { a + b } }

let dag = DagRunner::new();
let x = dag.add_task(Value(2));
let y = dag.add_task(Value(3));
let sum = dag.add_task(Add).depends_on((x, y));
dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;
```

**dagrs** (Complex: traits + channels + IDs + manual wiring):

```rust
#[async_trait]
impl Action for Value {
    async fn run(&self, _: &mut InChannels, out: &mut OutChannels, _: Arc<EnvVar>) -> Output {
        out.broadcast(Content::new(self.0)).await;
        Output::Out(Some(Content::new(self.0)))
    }
}

let mut table = NodeTable::new();
let node1 = DefaultNode::with_action("x".into(), Value(2), &mut table);
let id1 = node1.id();
let node2 = DefaultNode::with_action("y".into(), Value(3), &mut table);
let id2 = node2.id();
let node3 = DefaultNode::with_action("add".into(), Add, &mut table);
let id3 = node3.id();
let mut graph = Graph::new();
graph.add_node(node1); graph.add_node(node2); graph.add_node(node3);
graph.add_edge(id1, vec![id3]); graph.add_edge(id2, vec![id3]);
graph.set_env(EnvVar::new(table)); graph.start().unwrap();
```

**async_dag** (Medium: slots + indices):

```rust
let mut graph = Graph::new();
let x = graph.add_task(|| async { 2 });
let y = graph.add_task(|| async { 3 });
let sum = graph.add_child_task(x, |a: i32| async move { a }, 0)?;
graph.update_dependency(y, sum, 1)?;  // Must specify slot index
let sum = graph.add_child_task(sum, |a: i32, b: i32| async move { a + b }, 0)?;
graph.update_dependency(y, sum, 1)?;
```

**Key differences**:

- **dagx**: Type-safe dependencies, automatic wiring, no manual ID tracking, minimal boilerplate
- **dagrs**: Manual channel management, node ID tracking, Content wrapping, Action trait boilerplate
- **async_dag**: Slot indices must be tracked manually, dependencies updated separately

## Quick Comparison

| Project                                                       | License        | Runtime             | Type Safety           | API Complexity | Performance vs dagx          | Key Features                                                                         |
| ------------------------------------------------------------- | -------------- | ------------------- | --------------------- | -------------- | ---------------------------- | ------------------------------------------------------------------------------------ |
| **dagx**                                                      | MIT            | Any async runtime   | Compile-time          | Simple         | Baseline (see benchmarks)    | Primitives-as-scheduler, inline fast-path, automatic Arc wrapping, up to 8 deps/task |
| [**dagrs**](https://github.com/dagrs-dev/dagrs) (v0.5)        | MIT/Apache-2.0 | Creates own runtime | Runtime (async_trait) | Complex        | 1.04-129x slower (see below) | Flow-based Programming, cyclic graphs, loops, conditional nodes, YAML config         |
| [**async_dag**](https://github.com/chubei-oppen/async_dag)    | MIT            | Any async runtime   | Compile-time          | Medium         | No benchmarks                | Slot-based dependencies, Graph/TryGraph modes, maximum parallelism                   |
| [**dag-flow**](https://github.com/makisevon/dag-flow)         | MIT/Apache-2.0 | Any async runtime   | Runtime (HashMap)     | Complex        | No benchmarks                | Experimental, all tasks run simultaneously, weak dependencies                        |
| [**RenovZ/dag-runner**](https://github.com/RenovZ/dag-runner) | MIT            | Tokio only          | Unclear               | Simple         | No benchmarks                | Edge-based API, cycle detection, stops on first error                                |
| [**tasksitter**](https://github.com/lionkor/tasksitter)       | Unspecified    | Unclear             | Unclear               | Medium         | No benchmarks                | Cyclic graphs, dynamic runtime modification, pause/resume                            |

_GitHub stars (as of 2025): dagrs (449), async_dag (25), dag-flow (2), others (0-2)_

## Performance Benchmarks vs dagrs

Direct comparison benchmarks (lower is better):

| Workload          | Tasks  | dagx    | dagrs   | Speedup            |
| ----------------- | ------ | ------- | ------- | ------------------ |
| Sequential chain  | 5      | 3.0 µs  | 385 µs  | **129x faster** 🚀 |
| Sequential chain  | 100    | 79 µs   | 703 µs  | **8.9x faster**    |
| Diamond pattern   | 4      | 11 µs   | 387 µs  | **34x faster**     |
| Fan-out (1→100)   | 101    | 155 µs  | 595 µs  | **3.85x faster**   |
| Independent tasks | 10,000 | 12.7 ms | 13.3 ms | **1.04x faster**   |

**Summary**: dagx is **1.04-129x faster** than dagrs across all benchmark patterns. The inline fast-path optimization eliminates spawning overhead for sequential workloads while maintaining excellent parallel performance.

**How does dagx achieve this?** The inline fast-path detects single-task layers (common in sequential chains) and executes them directly without spawning overhead. For parallel workloads, tasks still spawn to maximize concurrency. This adaptive execution strategy combines the best of both worlds: sub-microsecond overhead for sequential work, true parallelism for concurrent work.

**Key insight**: Most real-world DAGs mix sequential and parallel patterns. dagx automatically optimizes for both, delivering 1.04-129x better performance than dagrs regardless of workload shape.

_Benchmarks run on AMD Ryzen 7 7840U (Zen 4) @ 3.3GHz. Run `cargo bench` to test on your hardware._

## Detailed Comparison

### dagrs (Most Mature)

**Best for**: Complex workflows requiring advanced flow control, Tokio-based applications, machine learning pipelines.

**Strengths**:

- Most mature (449 GitHub stars, active community)
- Rich feature set: Flow-based Programming, cyclic graphs, loops, conditional nodes
- YAML configuration support for declarative workflows
- Designed for complex orchestration patterns

**Trade-offs**:

- Creates own Tokio runtime internally (not runtime-agnostic, cannot be nested)
- More complex API: `Action` trait, `InChannels`/`OutChannels`, `NodeTable`, `Content` wrappers, manual node ID tracking
- Uses `async_trait` for type erasure (runtime overhead)
- **1.04-129x slower than dagx** across all benchmark patterns (see comparison benchmarks)

**API Style** (v0.5):

```rust
// Define action with channels for communication
#[async_trait]
impl Action for MyTask {
    async fn run(&self, input: &mut InChannels,
                 output: &mut OutChannels, env: Arc<EnvVar>) -> Output {
        let inputs: Vec<T> = input.map(|c| ...).await;  // Manually extract
        output.broadcast(Content::new(result)).await;   // Manually wrap
        Output::Out(Some(Content::new(result)))
    }
}

let mut table = NodeTable::new();
let node = DefaultNode::with_action("name".into(), MyTask, &mut table);
let node_id = node.id();  // Must capture ID before moving
graph.add_node(node);
graph.add_edge(source_id, vec![target_id]);
graph.set_env(EnvVar::new(table)); graph.start().unwrap();
```

### async_dag (Clean Type Safety)

**Best for**: Runtime flexibility with compile-time type safety, fail-fast workflows.

**Strengths**:

- Runtime-agnostic (works with any async runtime)
- Compile-time type checking on task connections
- Both standard (`Graph`) and fail-fast (`TryGraph`) modes
- Designed for maximum parallelism

**Trade-offs**:

- Medium API complexity: slot-based dependency management
- Must manually specify slot indices (0, 1, etc.) when connecting tasks
- Less mature (25 stars)
- No performance benchmarks

**API Style**:

```rust
let mut graph = Graph::new();
let _1 = graph.add_task(|| async { 1 });
let _2 = graph.add_task(|| async { 2 });

// add_child_task with slot index
let _3 = graph.add_child_task(_1, sum, 0).unwrap();
graph.update_dependency(_2, _3, 1).unwrap();  // Specify slot 1

graph.run().await;
```

### dag-flow (Experimental)

**Best for**: Experimental projects, flexible dependency awaiting patterns.

**Strengths**:

- Runtime-agnostic
- All tasks run simultaneously (not in dependency layers)
- Weak dependencies support
- Flexible input awaiting at any point in task execution

**Trade-offs**:

- Explicitly experimental
- Runtime type safety via `HashMap<String, Input>`
- Complex API: implement `Task` trait with `id()`, `dependencies()`, `run()`
- Named dependencies (string-based lookup)
- Very early stage (2 stars)

**API Style**:

```rust
impl Task<String, Bytes> for MyTask {
    fn id(&self) -> String { "task_name".into() }
    fn dependencies(&self) -> Option<Vec<String>> { Some(vec!["dep1".into()]) }

    async fn run(&self, inputs: HashMap<String, Input<'_, Bytes>>) -> Option<Bytes> {
        let dep_value = inputs.get("dep1").unwrap().await;
        // Process
    }
}
```

### RenovZ/dag-runner (Simple Edge-Based)

**Best for**: Simple DAGs with Tokio, straightforward edge-based dependencies.

**Strengths**:

- Simple API: `add_vertex()`, `add_edge()`
- Cycle detection
- Stops on first error

**Trade-offs**:

- Requires Tokio runtime
- Manual channel setup for task communication
- Type safety model unclear
- Very early stage (0 stars)

**API Style**:

```rust
let mut dag = Dag::default();
dag.add_vertex("one", || async move { /* task */ });
dag.add_edge("one", "two");
dag.run().await?;
```

### tasksitter (Dynamic Workflows)

**Best for**: Dynamic workflow modification, cyclic graphs, runtime introspection.

**Strengths**:

- Supports cyclic graphs (not just DAGs)
- Dynamic graph modification at runtime
- Pause/resume capabilities
- Graph introspection

**Trade-offs**:

- Limited documentation
- Runtime and type safety model unclear
- Very early stage (0 stars)

## When to Choose dagx

Choose dagx when you value:

- **Performance**: 1.04-129x faster than dagrs across all workload patterns (see benchmarks)
- **Runtime flexibility**: Works with Tokio, async-std, smol, or any async runtime
- **Compile-time safety**: Full type safety with no runtime type errors in the public API
- **Minimal overhead**: ~790 ns per task (sequential), ~1.3 µs per task (parallel)
- **Simple, ergonomic API**: `#[task]` macro, `add_task()`, `depends_on()` - that's it
- **Automatic optimizations**: Arc wrapping, inline execution, adaptive spawning - all transparent
- **Predictable performance**: Linear scaling, no hidden complexity, consistent sub-µs overhead

dagx is **not** the right choice if you need:

- Cyclic graphs or dynamic flow control (loops, conditions) → Consider **dagrs** or **tasksitter**
- More than 8 dependencies per task → Consider **dagrs** or **async_dag**
- YAML-based configuration → Consider **dagrs**

## When to Consider Alternatives

- **Choose dagrs** if you need advanced flow control (loops, conditionals, cyclic graphs), YAML configuration, or are already committed to Tokio and want a mature, feature-rich solution
- **Choose async_dag** if you want compile-time type safety with runtime flexibility and the slot-based API appeals to you
- **Choose dag-flow** if you're building experimental projects and the all-tasks-run-simultaneously model fits your use case
- **Choose RenovZ/dag-runner** if you need the simplest possible edge-based API and are already using Tokio
- **Choose tasksitter** if you need dynamic graph modification at runtime or cyclic workflow support
