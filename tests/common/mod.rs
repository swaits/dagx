// Common test utilities and helpers for the test suite

// Re-export task_fn for internal testing
// Note: task_fn is #[doc(hidden)] and not part of the documented public API
pub use dagx::task_fn;

// Re-export commonly used test structs
#[allow(dead_code)]
pub mod tasks {
    use dagx::task;

    // Simple seed task with mutable state
    pub struct Seed {
        pub value: i32,
    }

    impl Seed {
        pub fn new(v: i32) -> Self {
            Self { value: v }
        }
    }

    #[task]
    impl Seed {
        async fn run(&mut self) -> i32 {
            self.value
        }
    }

    // Stateless increment task
    pub struct Inc;

    impl Inc {
        pub fn new() -> Self {
            Self
        }
    }

    #[task]
    impl Inc {
        async fn run(x: &i32) -> i32 {
            x + 1
        }
    }

    // Stateless add task
    pub struct Add;

    impl Add {
        pub fn new() -> Self {
            Self
        }
    }

    #[task]
    impl Add {
        async fn run(a: &i32, b: &i32) -> i32 {
            a + b
        }
    }

    // Stateless multiply task
    pub struct Mul;

    impl Mul {
        pub fn new() -> Self {
            Self
        }
    }

    #[task]
    impl Mul {
        async fn run(a: &i32, b: &i32) -> i32 {
            a * b
        }
    }

    // Stateful counter task
    pub struct Counter {
        pub count: i32,
    }

    impl Counter {
        pub fn new(initial: i32) -> Self {
            Self { count: initial }
        }
    }

    #[task]
    impl Counter {
        async fn run(&mut self, input: &i32) -> i32 {
            self.count += input;
            self.count
        }
    }

    // Task that outputs multiple types combined as string
    pub struct OutputResult;

    impl OutputResult {
        pub fn new() -> Self {
            Self
        }
    }

    #[task]
    impl OutputResult {
        async fn run(s: &String, n: &usize, f: &bool) -> String {
            format!("{s}:{n}:{f}")
        }
    }
}
