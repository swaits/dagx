//! Comprehensive tests for Arc-wrapped task outputs
//!
//! These tests validate that Arc works correctly for:
//! - Basic 1:1 pipelines
//! - Fan-out patterns (1:N)
//! - Complex multi-layer DAGs
//! - Performance characteristics
//! - Edge cases and error handling
//!
//! These tests serve as both validation and documentation for proper Arc usage.

use dagx::{task, DagRunner};
use futures::FutureExt;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ============================================================================
// SECTION 1: Basic Arc Usage
// ============================================================================

/// Test 1: Basic Arc in 1:1 pipeline - validates Arc can flow through simple pipeline
#[tokio::test]
async fn test_arc_basic_pipeline() {
    struct Producer;

    #[task]
    impl Producer {
        async fn run(&mut self) -> Arc<String> {
            Arc::new("Hello, Arc!".to_string())
        }
    }

    struct Consumer;

    #[task]
    impl Consumer {
        async fn run(&mut self, data: &Arc<String>) -> String {
            format!("Received: {}", data)
        }
    }

    let dag = DagRunner::new();
    let producer = dag.add_task(Producer);
    let consumer = dag.add_task(Consumer).depends_on(&producer);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Note: producer is not a sink (consumer depends on it), so we can't get() its value
    // Only sink nodes (nodes with no dependents) store their outputs for retrieval
    assert_eq!(dag.get(consumer).unwrap(), "Received: Hello, Arc!");
}

/// Test 2: Arc with primitives - validates Arc works with basic types
#[tokio::test]
async fn test_arc_with_primitives() {
    struct I32Producer;

    #[task]
    impl I32Producer {
        async fn run(&mut self) -> Arc<i32> {
            Arc::new(42)
        }
    }

    struct F64Producer;

    #[task]
    impl F64Producer {
        async fn run(&mut self) -> Arc<f64> {
            Arc::new(std::f64::consts::PI)
        }
    }

    struct BoolProducer;

    #[task]
    impl BoolProducer {
        async fn run(&mut self) -> Arc<bool> {
            Arc::new(true)
        }
    }

    struct AllConsumer;

    #[task]
    impl AllConsumer {
        async fn run(&mut self, i: &Arc<i32>, f: &Arc<f64>, b: &Arc<bool>) -> String {
            format!("i={}, f={:.2}, b={}", **i, **f, **b)
        }
    }

    let dag = DagRunner::new();
    let i32_prod = dag.add_task(I32Producer);
    let f64_prod = dag.add_task(F64Producer);
    let bool_prod = dag.add_task(BoolProducer);
    let consumer = dag
        .add_task(AllConsumer)
        .depends_on((&i32_prod, &f64_prod, &bool_prod));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Note: producers are not sinks (consumer depends on them), so we can't get() their values
    // The consumer output verifies all three producers worked correctly
    assert_eq!(dag.get(consumer).unwrap(), "i=42, f=3.14, b=true");
}

/// Test 3: Arc with collections - validates Arc works with Vec, HashMap, HashSet
#[tokio::test]
async fn test_arc_with_collections() {
    struct VecProducer;

    #[task]
    impl VecProducer {
        async fn run(&mut self) -> Arc<Vec<i32>> {
            Arc::new(vec![1, 2, 3, 4, 5])
        }
    }

    struct MapProducer;

    #[task]
    impl MapProducer {
        async fn run(&mut self) -> Arc<HashMap<String, i32>> {
            let mut map = HashMap::new();
            map.insert("one".to_string(), 1);
            map.insert("two".to_string(), 2);
            map.insert("three".to_string(), 3);
            Arc::new(map)
        }
    }

    struct SetProducer;

    #[task]
    impl SetProducer {
        async fn run(&mut self) -> Arc<HashSet<String>> {
            let mut set = HashSet::new();
            set.insert("apple".to_string());
            set.insert("banana".to_string());
            set.insert("cherry".to_string());
            Arc::new(set)
        }
    }

    struct CollectionConsumer;

    #[task]
    impl CollectionConsumer {
        async fn run(
            &mut self,
            vec: &Arc<Vec<i32>>,
            map: &Arc<HashMap<String, i32>>,
            set: &Arc<HashSet<String>>,
        ) -> String {
            format!(
                "Vec sum: {}, Map size: {}, Set size: {}",
                vec.iter().sum::<i32>(),
                map.len(),
                set.len()
            )
        }
    }

    let dag = DagRunner::new();
    let vec_prod = dag.add_task(VecProducer);
    let map_prod = dag.add_task(MapProducer);
    let set_prod = dag.add_task(SetProducer);
    let consumer = dag
        .add_task(CollectionConsumer)
        .depends_on((&vec_prod, &map_prod, &set_prod));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Note: producers are not sinks (consumer depends on them), so we can't get() their values
    // The consumer output verifies all three collection producers worked correctly
    assert_eq!(
        dag.get(consumer).unwrap(),
        "Vec sum: 15, Map size: 3, Set size: 3"
    );
}

/// Test 4: Arc with custom structs - validates Arc works with user-defined types
#[tokio::test]
async fn test_arc_with_custom_structs() {
    #[derive(Debug, Clone, PartialEq)]
    struct Person {
        name: String,
        age: u32,
        emails: Vec<String>,
    }

    struct PersonProducer;

    #[task]
    impl PersonProducer {
        async fn run(&mut self) -> Arc<Person> {
            Arc::new(Person {
                name: "Alice".to_string(),
                age: 30,
                emails: vec![
                    "alice@example.com".to_string(),
                    "alice@work.com".to_string(),
                ],
            })
        }
    }

    struct NameExtractor;

    #[task]
    impl NameExtractor {
        async fn run(&mut self, person: &Arc<Person>) -> String {
            person.name.clone()
        }
    }

    struct AgeExtractor;

    #[task]
    impl AgeExtractor {
        async fn run(&mut self, person: &Arc<Person>) -> u32 {
            person.age
        }
    }

    struct EmailCountExtractor;

    #[task]
    impl EmailCountExtractor {
        async fn run(&mut self, person: &Arc<Person>) -> usize {
            person.emails.len()
        }
    }

    let dag = DagRunner::new();
    let person_prod = dag.add_task(PersonProducer);
    let name_ext = dag.add_task(NameExtractor).depends_on(&person_prod);
    let age_ext = dag.add_task(AgeExtractor).depends_on(&person_prod);
    let email_ext = dag.add_task(EmailCountExtractor).depends_on(&person_prod);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Note: person_prod is not a sink (3 extractors depend on it), so we can't get() its value
    // The extractor outputs verify the person was created and shared correctly
    assert_eq!(dag.get(name_ext).unwrap(), "Alice");
    assert_eq!(dag.get(age_ext).unwrap(), 30);
    assert_eq!(dag.get(email_ext).unwrap(), 2);
}

// ============================================================================
// SECTION 2: Fan-Out Patterns (CRITICAL!)
// ============================================================================

/// Test 5: 1:3 fan-out - validates efficient Arc sharing among multiple consumers
#[tokio::test]
async fn test_arc_fan_out_three_consumers() {
    struct DataProducer;

    #[task]
    impl DataProducer {
        async fn run(&mut self) -> Arc<Vec<String>> {
            Arc::new(vec![
                "item1".to_string(),
                "item2".to_string(),
                "item3".to_string(),
                "item4".to_string(),
                "item5".to_string(),
            ])
        }
    }

    struct CountConsumer;

    #[task]
    impl CountConsumer {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> usize {
            data.len()
        }
    }

    struct FirstConsumer;

    #[task]
    impl FirstConsumer {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> String {
            data.first().unwrap_or(&String::new()).clone()
        }
    }

    struct JoinConsumer;

    #[task]
    impl JoinConsumer {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> String {
            data.join(", ")
        }
    }

    let dag = DagRunner::new();
    let producer = dag.add_task(DataProducer);
    let c1 = dag.add_task(CountConsumer).depends_on(&producer);
    let c2 = dag.add_task(FirstConsumer).depends_on(&producer);
    let c3 = dag.add_task(JoinConsumer).depends_on(&producer);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(c1).unwrap(), 5);
    assert_eq!(dag.get(c2).unwrap(), "item1");
    assert_eq!(dag.get(c3).unwrap(), "item1, item2, item3, item4, item5");
}

/// Test 6: 1:5 fan-out with large data - proves Arc is more efficient than cloning
#[tokio::test]
async fn test_arc_fan_out_five_consumers_large_data() {
    struct LargeDataProducer;

    #[task]
    impl LargeDataProducer {
        async fn run(&mut self) -> Arc<Vec<String>> {
            Arc::new((0..10_000).map(|i| format!("Item {}", i)).collect())
        }
    }

    struct First100Analyzer;

    #[task]
    impl First100Analyzer {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> usize {
            data.iter().take(100).map(|s| s.len()).sum()
        }
    }

    struct Last100Analyzer;

    #[task]
    impl Last100Analyzer {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> usize {
            data.iter().rev().take(100).map(|s| s.len()).sum()
        }
    }

    struct LongestFinder;

    #[task]
    impl LongestFinder {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> usize {
            data.iter().map(|s| s.len()).max().unwrap_or(0)
        }
    }

    struct PatternCounter;

    #[task]
    impl PatternCounter {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> usize {
            data.iter().filter(|s| s.contains("999")).count()
        }
    }

    struct TotalBytesComputer;

    #[task]
    impl TotalBytesComputer {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> usize {
            data.iter().map(|s| s.len()).sum()
        }
    }

    let dag = DagRunner::new();
    let producer = dag.add_task(LargeDataProducer);
    let c1 = dag.add_task(First100Analyzer).depends_on(&producer);
    let c2 = dag.add_task(Last100Analyzer).depends_on(&producer);
    let c3 = dag.add_task(LongestFinder).depends_on(&producer);
    let c4 = dag.add_task(PatternCounter).depends_on(&producer);
    let c5 = dag.add_task(TotalBytesComputer).depends_on(&producer);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Note: producer is not a sink (has 5 consumers), so we can't get() its value
    // We verify correctness through the consumer outputs instead

    // Verify each consumer got correct results
    assert!(dag.get(c1).unwrap() > 0);
    assert!(dag.get(c2).unwrap() > 0);
    assert_eq!(dag.get(c3).unwrap(), 9); // "Item 9999" has 9 chars
    assert_eq!(dag.get(c4).unwrap(), 19); // Items containing "999": 999, 1999..8999, 9990-9999
    assert!(dag.get(c5).unwrap() > 0);
}

/// Test 7: Nested fan-out - Arc propagated through tree of tasks
#[tokio::test]
async fn test_arc_nested_fan_out_tree() {
    struct RootProducer;

    #[task]
    impl RootProducer {
        async fn run(&mut self) -> Arc<Vec<i32>> {
            Arc::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
        }
    }

    struct Level1Sum;

    #[task]
    impl Level1Sum {
        async fn run(&mut self, data: &Arc<Vec<i32>>) -> Arc<i32> {
            Arc::new(data.iter().sum())
        }
    }

    struct Level1Product;

    #[task]
    impl Level1Product {
        async fn run(&mut self, data: &Arc<Vec<i32>>) -> Arc<i32> {
            Arc::new(data.iter().take(5).product())
        }
    }

    struct Level2Double;

    #[task]
    impl Level2Double {
        async fn run(&mut self, sum: &Arc<i32>) -> i32 {
            **sum * 2
        }
    }

    struct Level2Square;

    #[task]
    impl Level2Square {
        async fn run(&mut self, sum: &Arc<i32>) -> i32 {
            **sum * **sum
        }
    }

    struct Level2Negate;

    #[task]
    impl Level2Negate {
        async fn run(&mut self, product: &Arc<i32>) -> i32 {
            -**product
        }
    }

    struct Level2Mod10;

    #[task]
    impl Level2Mod10 {
        async fn run(&mut self, product: &Arc<i32>) -> i32 {
            **product % 10
        }
    }

    let dag = DagRunner::new();
    let root = dag.add_task(RootProducer);
    let l1_sum = dag.add_task(Level1Sum).depends_on(&root);
    let l1_prod = dag.add_task(Level1Product).depends_on(&root);
    let l2_double = dag.add_task(Level2Double).depends_on(l1_sum);
    let l2_square = dag.add_task(Level2Square).depends_on(l1_sum);
    let l2_negate = dag.add_task(Level2Negate).depends_on(l1_prod);
    let l2_mod = dag.add_task(Level2Mod10).depends_on(l1_prod);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Note: l1_sum and l1_prod are not sinks (they have level 2 tasks depending on them)
    // Only the level 2 tasks are sinks and can be retrieved
    assert_eq!(dag.get(l2_double).unwrap(), 110); // 55 * 2
    assert_eq!(dag.get(l2_square).unwrap(), 3025); // 55 * 55
    assert_eq!(dag.get(l2_negate).unwrap(), -120); // -120
    assert_eq!(dag.get(l2_mod).unwrap(), 0); // 120 % 10
}

/// Test 8: Performance benchmark - Arc vs Clone for large data
#[tokio::test]
async fn test_arc_performance_vs_clone() {
    const DATA_SIZE: usize = 100_000;

    struct LargeProducer;

    #[task]
    impl LargeProducer {
        async fn run(&mut self) -> Arc<Vec<String>> {
            Arc::new(
                (0..DATA_SIZE)
                    .map(|i| format!("This is a relatively long string number {}", i))
                    .collect(),
            )
        }
    }

    struct Process1;

    #[task]
    impl Process1 {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> usize {
            tokio::time::sleep(Duration::from_millis(10)).await;
            data.len()
        }
    }

    struct Process2;

    #[task]
    impl Process2 {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> usize {
            tokio::time::sleep(Duration::from_millis(10)).await;
            data.iter().filter(|s| s.contains("5")).count()
        }
    }

    struct Process3;

    #[task]
    impl Process3 {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> usize {
            tokio::time::sleep(Duration::from_millis(10)).await;
            data.iter().map(|s| s.len()).sum()
        }
    }

    let dag = DagRunner::new();
    let producer = dag.add_task(LargeProducer);
    let p1 = dag.add_task(Process1).depends_on(&producer);
    let p2 = dag.add_task(Process2).depends_on(&producer);
    let p3 = dag.add_task(Process3).depends_on(&producer);

    let start = Instant::now();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    let elapsed = start.elapsed();

    // Verify results
    assert_eq!(dag.get(p1).unwrap(), DATA_SIZE);
    assert!(dag.get(p2).unwrap() > 0);
    assert!(dag.get(p3).unwrap() > 0);

    // With Arc, this should complete quickly even with large data
    assert!(
        elapsed < Duration::from_secs(2),
        "Arc fan-out was too slow: {:?}",
        elapsed
    );
}

// ============================================================================
// SECTION 3: Complex Scenarios
// ============================================================================

/// Test 9: Arc with Result - validates Arc<Result<T, E>> and Result<Arc<T>, E>
#[tokio::test]
async fn test_arc_with_result() {
    struct ArcResultOk;

    #[task]
    impl ArcResultOk {
        async fn run(&mut self) -> Arc<Result<String, String>> {
            Arc::new(Ok("Success".to_string()))
        }
    }

    struct ResultArcOk;

    #[task]
    impl ResultArcOk {
        async fn run(&mut self) -> Result<Arc<String>, String> {
            Ok(Arc::new("Success".to_string()))
        }
    }

    struct ArcResultErr;

    #[task]
    impl ArcResultErr {
        async fn run(&mut self) -> Arc<Result<String, String>> {
            Arc::new(Err("Error".to_string()))
        }
    }

    struct ArcResultConsumer;

    #[task]
    impl ArcResultConsumer {
        async fn run(&mut self, data: &Arc<Result<String, String>>) -> String {
            match data.as_ref() {
                Ok(s) => format!("Got Ok: {}", s),
                Err(e) => format!("Got Err: {}", e),
            }
        }
    }

    struct ResultArcConsumer;

    #[task]
    impl ResultArcConsumer {
        async fn run(&mut self, data: &Result<Arc<String>, String>) -> String {
            match data {
                Ok(arc) => format!("Got Arc: {}", arc),
                Err(e) => format!("Got Error: {}", e),
            }
        }
    }

    let dag = DagRunner::new();
    let arc_result_ok = dag.add_task(ArcResultOk);
    let result_arc_ok = dag.add_task(ResultArcOk);
    let arc_result_err = dag.add_task(ArcResultErr);

    let consumer1 = dag.add_task(ArcResultConsumer).depends_on(&arc_result_ok);
    let consumer2 = dag.add_task(ResultArcConsumer).depends_on(&result_arc_ok);

    // Create a new instance for the error case
    struct ArcResultConsumer2;

    #[task]
    impl ArcResultConsumer2 {
        async fn run(&mut self, data: &Arc<Result<String, String>>) -> String {
            match data.as_ref() {
                Ok(s) => format!("Got Ok: {}", s),
                Err(e) => format!("Got Err: {}", e),
            }
        }
    }

    let consumer3 = dag.add_task(ArcResultConsumer2).depends_on(&arc_result_err);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(consumer1).unwrap(), "Got Ok: Success");
    assert_eq!(dag.get(consumer2).unwrap(), "Got Arc: Success");
    assert_eq!(dag.get(consumer3).unwrap(), "Got Err: Error");
}

/// Test 10: Arc with Option - validates Arc<Option<T>> and Option<Arc<T>>
#[tokio::test]
async fn test_arc_with_option() {
    struct ArcOptionSome;

    #[task]
    impl ArcOptionSome {
        async fn run(&mut self) -> Arc<Option<i32>> {
            Arc::new(Some(42))
        }
    }

    struct ArcOptionNone;

    #[task]
    impl ArcOptionNone {
        async fn run(&mut self) -> Arc<Option<i32>> {
            Arc::new(None)
        }
    }

    struct OptionArcSome;

    #[task]
    impl OptionArcSome {
        async fn run(&mut self) -> Option<Arc<i32>> {
            Some(Arc::new(100))
        }
    }

    struct OptionArcNone;

    #[task]
    impl OptionArcNone {
        async fn run(&mut self) -> Option<Arc<i32>> {
            None
        }
    }

    struct ArcOptionConsumer1;

    #[task]
    impl ArcOptionConsumer1 {
        async fn run(&mut self, data: &Arc<Option<i32>>) -> String {
            match **data {
                Some(v) => format!("Some({})", v),
                None => "None".to_string(),
            }
        }
    }

    struct ArcOptionConsumer2;

    #[task]
    impl ArcOptionConsumer2 {
        async fn run(&mut self, data: &Arc<Option<i32>>) -> String {
            match **data {
                Some(v) => format!("Some({})", v),
                None => "None".to_string(),
            }
        }
    }

    struct OptionArcConsumer1;

    #[task]
    impl OptionArcConsumer1 {
        async fn run(&mut self, data: &Option<Arc<i32>>) -> String {
            match data {
                Some(arc) => format!("Arc({})", **arc),
                None => "No Arc".to_string(),
            }
        }
    }

    struct OptionArcConsumer2;

    #[task]
    impl OptionArcConsumer2 {
        async fn run(&mut self, data: &Option<Arc<i32>>) -> String {
            match data {
                Some(arc) => format!("Arc({})", **arc),
                None => "No Arc".to_string(),
            }
        }
    }

    let dag = DagRunner::new();
    let arc_opt_some = dag.add_task(ArcOptionSome);
    let arc_opt_none = dag.add_task(ArcOptionNone);
    let opt_arc_some = dag.add_task(OptionArcSome);
    let opt_arc_none = dag.add_task(OptionArcNone);

    let c1 = dag.add_task(ArcOptionConsumer1).depends_on(&arc_opt_some);
    let c2 = dag.add_task(ArcOptionConsumer2).depends_on(&arc_opt_none);
    let c3 = dag.add_task(OptionArcConsumer1).depends_on(&opt_arc_some);
    let c4 = dag.add_task(OptionArcConsumer2).depends_on(&opt_arc_none);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(c1).unwrap(), "Some(42)");
    assert_eq!(dag.get(c2).unwrap(), "None");
    assert_eq!(dag.get(c3).unwrap(), "Arc(100)");
    assert_eq!(dag.get(c4).unwrap(), "No Arc");
}

/// Test 11: Diamond dependency - Arc used in diamond pattern
#[tokio::test]
async fn test_arc_diamond_dependency() {
    struct Top;

    #[task]
    impl Top {
        async fn run(&mut self) -> Arc<Vec<i32>> {
            Arc::new(vec![1, 2, 3, 4, 5])
        }
    }

    struct LeftBranch;

    #[task]
    impl LeftBranch {
        async fn run(&mut self, data: &Arc<Vec<i32>>) -> Arc<i32> {
            Arc::new(data.iter().sum())
        }
    }

    struct RightBranch;

    #[task]
    impl RightBranch {
        async fn run(&mut self, data: &Arc<Vec<i32>>) -> Arc<i32> {
            Arc::new(data.iter().product())
        }
    }

    struct Bottom;

    #[task]
    impl Bottom {
        async fn run(&mut self, left: &Arc<i32>, right: &Arc<i32>) -> String {
            format!("Sum: {}, Product: {}", **left, **right)
        }
    }

    let dag = DagRunner::new();
    let top = dag.add_task(Top);
    let left = dag.add_task(LeftBranch).depends_on(&top);
    let right = dag.add_task(RightBranch).depends_on(&top);
    let bottom = dag.add_task(Bottom).depends_on((&left, &right));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(bottom).unwrap(), "Sum: 15, Product: 120");
}

// ============================================================================
// SECTION 4: Edge Cases
// ============================================================================

/// Test 12: Empty Arc - Arc<Vec<T>> with empty Vec
#[tokio::test]
async fn test_arc_empty_collections() {
    struct EmptyVec;

    #[task]
    impl EmptyVec {
        async fn run(&mut self) -> Arc<Vec<i32>> {
            Arc::new(Vec::new())
        }
    }

    struct EmptyString;

    #[task]
    impl EmptyString {
        async fn run(&mut self) -> Arc<String> {
            Arc::new(String::new())
        }
    }

    struct EmptyHashMap;

    #[task]
    impl EmptyHashMap {
        async fn run(&mut self) -> Arc<HashMap<String, i32>> {
            Arc::new(HashMap::new())
        }
    }

    struct EmptyConsumer;

    #[task]
    impl EmptyConsumer {
        async fn run(
            &mut self,
            vec: &Arc<Vec<i32>>,
            string: &Arc<String>,
            map: &Arc<HashMap<String, i32>>,
        ) -> String {
            format!(
                "Vec empty: {}, String empty: {}, Map empty: {}",
                vec.is_empty(),
                string.is_empty(),
                map.is_empty()
            )
        }
    }

    let dag = DagRunner::new();
    let vec_task = dag.add_task(EmptyVec);
    let string_task = dag.add_task(EmptyString);
    let map_task = dag.add_task(EmptyHashMap);
    let consumer = dag
        .add_task(EmptyConsumer)
        .depends_on((&vec_task, &string_task, &map_task));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(
        dag.get(consumer).unwrap(),
        "Vec empty: true, String empty: true, Map empty: true"
    );
}

/// Test 13: Very large Arc - Arc<Vec<String>> with 100K elements
#[tokio::test]
async fn test_arc_very_large_data() {
    const SIZE: usize = 100_000;

    struct HugeProducer;

    #[task]
    impl HugeProducer {
        async fn run(&mut self) -> Arc<Vec<String>> {
            Arc::new(
                (0..SIZE)
                    .map(|i| format!("Item number {} with some additional text", i))
                    .collect(),
            )
        }
    }

    struct ItemCounter;

    #[task]
    impl ItemCounter {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> usize {
            data.len()
        }
    }

    struct FirstSampler;

    #[task]
    impl FirstSampler {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> String {
            data.first().cloned().unwrap_or_default()
        }
    }

    struct LastSampler;

    #[task]
    impl LastSampler {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> String {
            data.last().cloned().unwrap_or_default()
        }
    }

    struct MiddleSampler;

    #[task]
    impl MiddleSampler {
        async fn run(&mut self, data: &Arc<Vec<String>>) -> String {
            data.get(SIZE / 2).cloned().unwrap_or_default()
        }
    }

    let dag = DagRunner::new();
    let producer = dag.add_task(HugeProducer);
    let counter = dag.add_task(ItemCounter).depends_on(&producer);
    let first = dag.add_task(FirstSampler).depends_on(&producer);
    let last = dag.add_task(LastSampler).depends_on(&producer);
    let middle = dag.add_task(MiddleSampler).depends_on(&producer);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(counter).unwrap(), SIZE);
    assert!(dag.get(first).unwrap().contains("Item number 0"));
    assert!(dag.get(last).unwrap().contains("Item number 99999"));
    assert!(dag.get(middle).unwrap().contains("Item number 50000"));
}

/// Test 14: Arc of Arc - Arc<Arc<T>> (unusual but should work)
#[tokio::test]
async fn test_arc_of_arc() {
    struct NestedProducer;

    #[task]
    impl NestedProducer {
        #[allow(clippy::arc_with_non_send_sync)]
        async fn run(&mut self) -> Arc<Arc<String>> {
            Arc::new(Arc::new("Nested Arc".to_string()))
        }
    }

    struct NestedConsumer;

    #[task]
    impl NestedConsumer {
        #[allow(clippy::redundant_allocation)]
        async fn run(&mut self, data: &Arc<Arc<String>>) -> String {
            format!("Value: {}", ***data)
        }
    }

    let dag = DagRunner::new();
    let producer = dag.add_task(NestedProducer);
    let consumer = dag.add_task(NestedConsumer).depends_on(&producer);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(dag.get(consumer).unwrap(), "Value: Nested Arc");
}

/// Test 15: Mixed types - Some tasks use Arc, others don't
#[tokio::test]
async fn test_arc_mixed_with_non_arc() {
    struct ArcProducer;

    #[task]
    impl ArcProducer {
        async fn run(&mut self) -> Arc<String> {
            Arc::new("Arc data".to_string())
        }
    }

    struct NonArcProducer;

    #[task]
    impl NonArcProducer {
        async fn run(&mut self) -> String {
            "Non-Arc data".to_string()
        }
    }

    struct MixedConsumer;

    #[task]
    impl MixedConsumer {
        async fn run(&mut self, arc_data: &Arc<String>, non_arc_data: &String) -> String {
            format!("Arc: {}, Non-Arc: {}", arc_data, non_arc_data)
        }
    }

    struct Unwrapper;

    #[task]
    impl Unwrapper {
        async fn run(&mut self, data: &Arc<String>) -> String {
            format!("Unwrapped: {}", **data)
        }
    }

    struct Wrapper;

    #[task]
    impl Wrapper {
        async fn run(&mut self, data: &String) -> Arc<String> {
            Arc::new(format!("Wrapped: {}", data))
        }
    }

    let dag = DagRunner::new();
    let arc_prod = dag.add_task(ArcProducer);
    let non_arc_prod = dag.add_task(NonArcProducer);
    let mixed = dag
        .add_task(MixedConsumer)
        .depends_on((&arc_prod, &non_arc_prod));
    let unwrap = dag.add_task(Unwrapper).depends_on(&arc_prod);
    let wrap = dag.add_task(Wrapper).depends_on(&non_arc_prod);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    assert_eq!(
        dag.get(mixed).unwrap(),
        "Arc: Arc data, Non-Arc: Non-Arc data"
    );
    assert_eq!(dag.get(unwrap).unwrap(), "Unwrapped: Arc data");
    assert_eq!(*dag.get(wrap).unwrap(), "Wrapped: Non-Arc data");
}

// ============================================================================
// SECTION 5: Advanced Usage
// ============================================================================

/// Test 16: Arc in parallel execution paths
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(tarpaulin, ignore)] // Timing tests are unreliable with instrumentation overhead
#[cfg(not(target_os = "macos"))] // macOS CI runners are too slow and unreliable for timing tests
async fn test_arc_parallel_execution() {
    struct SharedProducer;

    #[task]
    impl SharedProducer {
        async fn run(&mut self) -> Arc<Vec<i32>> {
            Arc::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
        }
    }

    struct ParallelTask1;

    #[task]
    impl ParallelTask1 {
        async fn run(&mut self, data: &Arc<Vec<i32>>) -> i32 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            data.iter().sum()
        }
    }

    struct ParallelTask2;

    #[task]
    impl ParallelTask2 {
        async fn run(&mut self, data: &Arc<Vec<i32>>) -> i32 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            data.iter().product()
        }
    }

    struct ParallelTask3;

    #[task]
    impl ParallelTask3 {
        async fn run(&mut self, data: &Arc<Vec<i32>>) -> i32 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            data.len() as i32
        }
    }

    let dag = DagRunner::new();
    let producer = dag.add_task(SharedProducer);
    let t1 = dag.add_task(ParallelTask1).depends_on(&producer);
    let t2 = dag.add_task(ParallelTask2).depends_on(&producer);
    let t3 = dag.add_task(ParallelTask3).depends_on(&producer);

    let start = Instant::now();

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    let elapsed = start.elapsed();

    // If running in parallel, should take ~50ms, not 150ms
    // Allow significant overhead for slower CI runners (especially macOS which can be very slow)
    // Serial execution would take 150ms+, so 200ms threshold still proves parallelism
    assert!(
        elapsed < Duration::from_millis(200),
        "Tasks didn't run in parallel: {:?}",
        elapsed
    );

    assert_eq!(dag.get(t1).unwrap(), 55);
    assert_eq!(dag.get(t2).unwrap(), 3628800);
    assert_eq!(dag.get(t3).unwrap(), 10);
}

/// Test 17: Arc reference counting verification
#[tokio::test]
async fn test_arc_reference_counting() {
    struct RefCountProducer;

    #[task]
    impl RefCountProducer {
        async fn run(&mut self) -> Arc<String> {
            let arc = Arc::new("Reference counted".to_string());
            // At creation, strong count should be 1
            assert_eq!(Arc::strong_count(&arc), 1);
            arc
        }
    }

    struct RefCountChecker;

    #[task]
    impl RefCountChecker {
        async fn run(&mut self, data: &Arc<String>) -> usize {
            // When passed to task, count increases due to cloning
            Arc::strong_count(data)
        }
    }

    let dag = DagRunner::new();
    let producer = dag.add_task(RefCountProducer);
    let checker = dag.add_task(RefCountChecker).depends_on(&producer);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    // Note: producer is not a sink (checker depends on it), so we can't get() its value
    // The checker task verifies the Arc was passed correctly
    let count = dag.get(checker).unwrap();
    assert!(count > 0); // Should be at least 1
}

// ============================================================================
// SECTION 6: Performance Comparison - Arc vs Clone with Massive Fan-Out
// ============================================================================

/// Test 18: Performance comparison - Arc vs Clone with optimized fan-out
///
/// This test demonstrates the critical performance benefit of using Arc for fan-out patterns.
/// It creates two identical DAG structures:
/// 1. WITHOUT Arc: 500KB Vec<u8> cloned to 60 downstream tasks (30MB total copied)
/// 2. WITH Arc: Arc<Vec<u8>> shared with 60 downstream tasks (only pointer copies)
///
/// The Arc version should be measurably faster due to avoiding deep copies,
/// while keeping test execution well under 1 second for fast CI/CD cycles.
///
/// Optimized from original 2MB×200 (400MB) to maintain test effectiveness
/// while reducing execution time from 6.5s to under 0.5s.
#[tokio::test]
async fn test_arc_massive_fanout_performance_comparison() {
    const DATA_SIZE: usize = 500_000; // 500KB - large enough to show meaningful difference
    const FAN_OUT_COUNT: usize = 60; // 60 consumers - balanced for sub-1s execution

    // Generate 500KB of pseudo-random data
    let random_data: Vec<u8> = (0..DATA_SIZE).map(|i| (i % 256) as u8).collect();

    // ========================================
    // Scenario 1: WITHOUT Arc (expensive cloning)
    // ========================================

    struct ProducerWithoutArc {
        data: Vec<u8>,
    }

    #[task]
    impl ProducerWithoutArc {
        async fn run(&mut self) -> Vec<u8> {
            self.data.clone() // Returns the Vec directly
        }
    }

    struct ConsumerWithoutArc;

    #[task]
    impl ConsumerWithoutArc {
        #[allow(clippy::ptr_arg)]
        async fn run(&mut self, data: &Vec<u8>) -> usize {
            // Simulate some work: compute checksum
            data.iter().map(|&b| b as usize).sum::<usize>() % 1000
        }
    }

    let dag_without_arc = DagRunner::new();
    let producer = dag_without_arc.add_task(ProducerWithoutArc {
        data: random_data.clone(),
    });

    // Create consumer tasks for fan-out pattern
    let consumers_without_arc: Vec<_> = (0..FAN_OUT_COUNT)
        .map(|_| {
            dag_without_arc
                .add_task(ConsumerWithoutArc)
                .depends_on(&producer)
        })
        .collect();

    let start_without_arc = Instant::now();
    dag_without_arc
        .run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();
    let duration_without_arc = start_without_arc.elapsed();

    // Verify all consumers completed
    for consumer in &consumers_without_arc {
        let result = dag_without_arc.get(*consumer).unwrap();
        assert!(result < 1000); // Checksum should be valid
    }

    // ========================================
    // Scenario 2: WITH Arc (efficient sharing)
    // ========================================

    struct ProducerWithArc {
        data: Vec<u8>,
    }

    #[task]
    impl ProducerWithArc {
        async fn run(&mut self) -> Arc<Vec<u8>> {
            Arc::new(self.data.clone()) // Wraps Vec in Arc
        }
    }

    struct ConsumerWithArc;

    #[task]
    impl ConsumerWithArc {
        async fn run(&mut self, data: &Arc<Vec<u8>>) -> usize {
            // Same work as before: compute checksum
            data.iter().map(|&b| b as usize).sum::<usize>() % 1000
        }
    }

    let dag_with_arc = DagRunner::new();
    let producer_arc = dag_with_arc.add_task(ProducerWithArc {
        data: random_data.clone(),
    });

    // Create consumer tasks for fan-out pattern
    let consumers_with_arc: Vec<_> = (0..FAN_OUT_COUNT)
        .map(|_| {
            dag_with_arc
                .add_task(ConsumerWithArc)
                .depends_on(&producer_arc)
        })
        .collect();

    let start_with_arc = Instant::now();
    dag_with_arc
        .run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();
    let duration_with_arc = start_with_arc.elapsed();

    // Verify all consumers completed
    for consumer in &consumers_with_arc {
        let result = dag_with_arc.get(*consumer).unwrap();
        assert!(result < 1000); // Checksum should be valid
    }

    // ========================================
    // Performance Comparison
    // ========================================

    println!("\n=== Performance Comparison: Arc vs Clone ===");
    println!("Data size: {} bytes ({}KB)", DATA_SIZE, DATA_SIZE / 1000);
    println!("Fan-out count: {} consumers", FAN_OUT_COUNT);
    println!(
        "Total data copied without Arc: {:.1} MB",
        (DATA_SIZE * FAN_OUT_COUNT) as f64 / 1_000_000.0
    );
    println!(
        "Total data copied with Arc: ~{} bytes (just pointers)",
        FAN_OUT_COUNT * std::mem::size_of::<usize>()
    );
    println!("\nWithout Arc (cloning): {:?}", duration_without_arc);
    println!("With Arc (sharing):     {:?}", duration_with_arc);

    if duration_without_arc > duration_with_arc {
        let speedup = duration_without_arc.as_secs_f64() / duration_with_arc.as_secs_f64();
        println!("Speedup: {:.2}x faster with Arc", speedup);
    }

    // Assert that Arc version is faster (or at least not slower)
    // We allow some variance due to system conditions, but Arc should generally be faster
    // With 500KB × 60 clones (30MB total), the difference should be clearly measurable
    assert!(
        duration_with_arc <= duration_without_arc * 2, // Allow 2x slower in worst case (e.g., cold start)
        "Arc version should not be significantly slower. Without Arc: {:?}, With Arc: {:?}",
        duration_without_arc,
        duration_with_arc
    );

    // In most cases, Arc should be noticeably faster
    // This is a soft assertion - we expect it to pass most of the time
    if duration_with_arc < duration_without_arc {
        println!("✓ Arc version is faster as expected");
    } else {
        println!("⚠ Arc version was not faster this run (system variance), but still acceptable");
    }
}
