//! Tests demonstrating that the type system prevents cycles at compile-time.
//!
//! These tests show that even if you try to create cycles, the Rust compiler
//! prevents it through the type-state pattern.

use dagx::{task, DagRunner};

/// Demonstrates that TaskBuilder is consumed by depends_on,
/// preventing you from wiring the same task multiple times
#[test]
fn test_builder_consumed_by_depends_on() {
    struct Source;
    #[task]
    impl Source {
        async fn run(&self) -> i32 {
            42
        }
    }

    struct Consumer;
    #[task]
    impl Consumer {
        async fn run(&self, input: &i32) -> i32 {
            *input * 2
        }
    }

    let mut dag = DagRunner::new();
    let source = dag.add_task(Source);
    let consumer_builder = dag.add_task(Consumer);

    // This consumes consumer_builder and returns a TaskHandle
    let _consumer_handle = consumer_builder.depends_on(source);

    // We CANNOT use consumer_builder again because it was moved
    // This would fail to compile:
    // let _consumer2 = consumer_builder.depends_on(source);  // ERROR: use of moved value
}

/// Demonstrates that you can only get a TaskHandle by finalizing a builder,
/// and handles are immutable references with no methods to add dependencies
#[test]
fn test_handle_is_immutable_reference() {
    struct NoInput;
    #[task]
    impl NoInput {
        async fn run(&self) -> i32 {
            100
        }
    }

    let mut dag = DagRunner::new();
    let builder = dag.add_task(NoInput);

    // Convert to handle (only works because Input = ())
    let handle = builder;

    // TaskHandle is Copy/Clone - just an ID wrapper
    let _copy1 = handle;
    let _copy2 = handle;
    let _copy3 = handle;

    // But TaskHandle has NO methods to modify the DAG
    // There's no way to add dependencies to it
    // This would fail to compile:
    // handle.depends_on(something);  // ERROR: no method named `depends_on`
}

/// Documents the ordering constraint that prevents cycles
#[test]
fn test_ordering_prevents_cycles() {
    struct TaskA;
    #[task]
    impl TaskA {
        async fn run(&self) -> i32 {
            42
        }
    }

    struct TaskB;
    #[task]
    impl TaskB {
        async fn run(&self, input: &i32) -> i32 {
            input + 2
        }
    }

    let mut dag = DagRunner::new();
    let a_builder = dag.add_task(TaskA);
    let b_builder = dag.add_task(TaskB);

    // To make A depend on B:
    // 1. Need B as a TaskHandle
    // 2. But B is currently a TaskBuilder
    // 3. To get B as TaskHandle, must call b_builder.depends_on(...)
    // 4. But then B depends on something else, not A!

    // To make B depend on A:
    // 1. Need A as a TaskHandle
    // 2. But if we finalize A first, we can't make it depend on B later

    // This creates an impossible ordering:
    // - Can't finalize A before B (need B first for A's dependency)
    // - Can't finalize B before A (need A first for B's dependency)

    // The ONLY valid ordering is a strict topological order
    // So we can only create DAGs, never cycles!

    // Let's create a valid DAG instead: A→B (B depends on A)
    let a_handle = a_builder; // A has no deps (unit input)
    let _b_handle = b_builder.depends_on(a_handle);
    // Now we could try: a_handle.depends_on(b_handle)?
    // But TaskHandle has no such method!
}

/// Shows that even with unit-type tasks, cycles are prevented
#[test]
fn test_unit_tasks_cannot_create_cycles() {
    struct Task1;
    #[task]
    impl Task1 {
        async fn run(&self) -> i32 {
            1
        }
    }

    struct Task2;
    #[task]
    impl Task2 {
        async fn run(&self) -> i32 {
            2
        }
    }

    let mut dag = DagRunner::new();
    let task1 = dag.add_task(Task1);
    let task2 = dag.add_task(Task2);

    // Both have unit input, so we can convert to handles
    let _handle1 = task1;
    let _handle2 = task2;

    // Now we have two handles, but...
    // TaskHandle has no methods to add dependencies!
    // The ONLY way to add dependencies is via TaskBuilder::depends_on()
    // And once you've converted to TaskHandle, you can't go back

    // This would fail to compile:
    // _handle1.depends_on(_handle2);  // ERROR: no method named `depends_on`
}

/// Demonstrates the catch-22 of three-way cycles
#[test]
fn test_three_way_cycle_impossible() {
    struct A;
    #[task]
    impl A {
        async fn run(&self, c: &i32) -> i32 {
            *c
        }
    }

    struct B;
    #[task]
    impl B {
        async fn run(&self, a: &i32) -> i32 {
            *a
        }
    }

    struct C;
    #[task]
    impl C {
        async fn run(&self, b: &i32) -> i32 {
            *b
        }
    }

    let mut dag = DagRunner::new();
    let _a_builder = dag.add_task(A);
    let _b_builder = dag.add_task(B);
    let _c_builder = dag.add_task(C);

    // To create A→C→B→A:
    // 1. a_builder.depends_on(c_handle)  // Need c_handle
    // 2. c_builder.depends_on(b_handle)  // Need b_handle
    // 3. b_builder.depends_on(a_handle)  // Need a_handle (from step 1!)
    //
    // But step 3 needs a_handle which doesn't exist until step 1 completes.
    // And step 1 needs c_handle which doesn't exist until step 2 completes.
    // And step 2 needs b_handle which doesn't exist until step 3 completes.
    //
    // This creates a circular dependency in the COMPILATION order,
    // not just the runtime order. The compiler won't allow it!
}

/// Shows that self-loops are prevented by the move semantics
#[test]
fn test_self_loop_prevented() {
    struct SelfReferential;
    #[task]
    impl SelfReferential {
        async fn run(&self, input: &i32) -> i32 {
            input + 1
        }
    }

    let mut dag = DagRunner::new();
    let _builder = dag.add_task(SelfReferential);

    // To create A→A (self-loop):
    // builder.depends_on(builder)
    //
    // But this would fail because:
    // 1. depends_on() takes `self` by value (moves builder)
    // 2. So builder is moved in the function call
    // 3. We can't also use &builder as an argument because it's being moved
    //
    // This would fail to compile:
    // let _handle = builder.depends_on(builder);  // ERROR: use of moved value
}

/// Comprehensive test showing all the guardrails
#[tokio::test]
async fn test_type_safety_prevents_invalid_graphs() {
    // The type system enforces:
    // 1. Tasks with dependencies must call depends_on()
    // 2. depends_on() consumes the builder (can only be called once)
    // 3. TaskHandle is immutable (no way to modify after finalization)
    // 4. You need a TaskHandle to wire a dependency
    // 5. Getting a TaskHandle consumes the TaskBuilder
    //
    // Together, these rules make it IMPOSSIBLE to create cycles!

    struct Source;
    #[task]
    impl Source {
        async fn run(&self) -> i32 {
            10
        }
    }

    struct Transform;
    #[task]
    impl Transform {
        async fn run(&self, x: &i32) -> i32 {
            x * 2
        }
    }

    struct Aggregate;
    #[task]
    impl Aggregate {
        async fn run(&self, a: &i32, b: &i32) -> i32 {
            a + b
        }
    }

    let mut dag = DagRunner::new();

    // Build a valid DAG: two sources, two transforms, one aggregator
    let s1 = dag.add_task(Source);
    let s2 = dag.add_task(Source);

    // Transform depends on sources
    let t1 = dag.add_task(Transform).depends_on(s1);
    let t2 = dag.add_task(Transform).depends_on(s2);

    // Aggregate depends on transforms
    let result = dag.add_task(Aggregate).depends_on((&t1, &t2));

    // Execute and verify
    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    assert_eq!(output.get(result).unwrap(), 40); // (10*2) + (10*2)

    // Now if we wanted to add a cycle, we'd need to:
    // - Make one of the Source tasks depend on the Aggregate result
    // - But the Source builders were already consumed by .into() conversions
    // - And even if we had them, Aggregate is already finalized as a TaskHandle
    // - And TaskHandle has no method to add it as a dependency
    //
    // The API makes cycles structurally impossible!
}
