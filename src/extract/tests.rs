//! Unit tests for extract module

#![allow(dead_code)] // Test helpers generate unused code via macro

use crate::extract::ExtractInput;
use std::any::Any;
use std::sync::Arc;

// Test task for error scenarios
#[allow(dead_code)]
struct TestTask {
    value: i32,
}

#[crate::task]
#[allow(dead_code)]
impl TestTask {
    #[cfg(not(tarpaulin_include))]
    async fn run(&self) -> i32 {
        self.value
    }
}

#[allow(dead_code)]
struct TestTaskWithInput;

#[crate::task]
#[allow(dead_code)]
impl TestTaskWithInput {
    #[cfg(not(tarpaulin_include))]
    async fn run(input: &i32) -> i32 {
        input * 2
    }
}

#[tokio::test]
async fn test_extract_result_type_single_dependency_error() {
    // Test Result<T,E> with wrong dependency count
    let deps = vec![
        Arc::new(Ok::<i32, String>(5)) as Arc<dyn Any + Send + Sync>,
        Arc::new(Err::<i32, String>("test".to_string())) as Arc<dyn Any + Send + Sync>,
    ]; // Two dependencies when expecting 1

    let result = Result::<i32, String>::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 2"));
}

#[tokio::test]
async fn test_extract_result_type_success() {
    // Test Result<T,E> successful extraction

    let deps = vec![Arc::new(Ok::<i32, String>(42)) as Arc<dyn Any + Send + Sync>];
    let result = Result::<i32, String>::extract_from_deps(deps).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Ok(42));
}

#[tokio::test]
async fn test_extract_option_type_single_dependency_error() {
    // Test Option<T> with wrong dependency count
    let deps = vec![
        Arc::new(None::<String>) as Arc<dyn Any + Send + Sync>,
        Arc::new(None::<String>) as Arc<dyn Any + Send + Sync>,
        Arc::new(None::<String>) as Arc<dyn Any + Send + Sync>,
    ]; // Three dependencies when expecting 1

    let result = Option::<String>::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 3"));
}

#[tokio::test]
async fn test_extract_option_type_success() {
    // Test Option<T> successful extraction
    let deps = vec![Arc::new(Some(99)) as Arc<dyn Any + Send + Sync>];
    let result = Option::<i32>::extract_from_deps(deps).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some(99));
}

#[tokio::test]
async fn test_extract_vec_type_single_dependency_error() {
    // Test Vec<T> with wrong dependency count
    let deps = vec![]; // Zero dependencies when expecting 1

    let result = Vec::<u8>::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 0"));
}

#[tokio::test]
async fn test_extract_vec_type_success() {
    // Test Vec<T> successful extraction
    let deps = vec![Arc::new(vec![1, 2, 3]) as Arc<dyn Any + Send + Sync>];
    let result = Vec::<i32>::extract_from_deps(deps).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), vec![1, 2, 3]);
}

#[tokio::test]
async fn test_extract_primitive_single_dependency_error() {
    // Test primitive types with wrong dependency count
    let deps = vec![
        Arc::new(0) as Arc<dyn Any + Send + Sync>,
        Arc::new(0) as Arc<dyn Any + Send + Sync>,
    ]; // Two dependencies when expecting 1

    let result = i32::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 2"));

    // Test with String
    let deps = vec![
        Arc::new(String::default()) as Arc<dyn Any + Send + Sync>,
        Arc::new(String::default()) as Arc<dyn Any + Send + Sync>,
    ];
    let result = String::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 2"));

    // Test with f64
    let deps = vec![
        Arc::new(0.0) as Arc<dyn Any + Send + Sync>,
        Arc::new(0.0) as Arc<dyn Any + Send + Sync>,
    ];
    let result = f64::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 2"));
}

#[tokio::test]
async fn test_extract_unit_type() {
    // Test that unit type always succeeds regardless of dependencies
    let deps = vec![];
    let result = <()>::extract_from_deps(deps).await;
    assert!(result.is_ok());

    // Even with dependencies (ignored), unit type succeeds
    let deps = vec![
        Arc::new(0) as Arc<dyn Any + Send + Sync>,
        Arc::new(0) as Arc<dyn Any + Send + Sync>,
    ];
    let result = <()>::extract_from_deps(deps).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_extract_tuple_basic() {
    // Test tuple extraction
    let deps = vec![
        Arc::new(42) as Arc<dyn Any + Send + Sync>,
        Arc::new(100) as Arc<dyn Any + Send + Sync>,
    ];

    // Extract a tuple (i32, i32)
    let result = <(i32, i32)>::extract_from_deps(deps).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), (42, 100));
}

#[tokio::test]
async fn test_extract_type_mismatch() {
    // Test type mismatch during downcast
    // Try to extract as wrong type - put wrong receiver type in deps
    let deps = vec![Arc::new("hello".to_string()) as Arc<dyn Any + Send + Sync>];
    let result = i32::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Type mismatch"));
}

#[tokio::test]
async fn test_extract_all_primitive_types() {
    // Test that all primitive types in impl_extract_single! work correctly
    macro_rules! test_type {
        ($t:ty, $val:expr) => {
            let deps = vec![Arc::new($val) as Arc<dyn Any + Send + Sync>];
            let result = <$t>::extract_from_deps(deps).await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), $val);
        };
    }

    test_type!(i8, 42i8);
    test_type!(i16, 42i16);
    test_type!(i32, 42i32);
    test_type!(i64, 42i64);
    test_type!(String, "test".to_string());
}

#[tokio::test]
async fn test_extract_larger_tuples() {
    // Test extraction for larger tuple sizes
    let deps = vec![
        Arc::new(0) as Arc<dyn Any + Send + Sync>,
        Arc::new(1) as Arc<dyn Any + Send + Sync>,
        Arc::new(2) as Arc<dyn Any + Send + Sync>,
        Arc::new(3) as Arc<dyn Any + Send + Sync>,
        Arc::new(4) as Arc<dyn Any + Send + Sync>,
    ];

    // Extract a 5-element tuple
    let result = <(i32, i32, i32, i32, i32)>::extract_from_deps(deps).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), (0, 1, 2, 3, 4));
}

#[tokio::test]
async fn test_extract_arc_type() {
    // Test Arc<T> extraction
    let test_arc = Arc::new("test".to_string());
    let test_arc_clone = test_arc.clone();

    let deps = vec![Arc::new(test_arc) as Arc<dyn Any + Send + Sync>];
    let result = Arc::<String>::extract_from_deps(deps).await;
    assert!(result.is_ok());
    assert_eq!(*result.unwrap(), *test_arc_clone);
}

// Error path tests for ExtractInput implementations
use std::collections::HashMap;

#[tokio::test]
async fn test_hashmap_extract_wrong_dependency_count() {
    let deps = vec![
        Arc::new(HashMap::<String, i32>::default()) as Arc<dyn Any + Send + Sync>,
        Arc::new(HashMap::<String, i32>::default()) as Arc<dyn Any + Send + Sync>,
    ];

    let result = HashMap::<String, i32>::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 2"));
}

#[tokio::test]
async fn test_hashmap_extract_type_mismatch() {
    let deps = vec![Arc::new("wrong type".to_string()) as Arc<dyn Any + Send + Sync>];
    let result = HashMap::<String, i32>::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Type mismatch"));
}

#[tokio::test]
async fn test_result_extract_downcast_failure() {
    let deps = vec![Arc::new("wrong".to_string()) as Arc<dyn Any + Send + Sync>];
    let result = Result::<i32, String>::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Type mismatch"));
}

#[tokio::test]
async fn test_option_extract_downcast_failure() {
    let deps = vec![Arc::new("wrong".to_string()) as Arc<dyn Any + Send + Sync>];
    let result = Option::<i32>::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Type mismatch"));
}

#[tokio::test]
async fn test_vec_extract_downcast_failure() {
    let deps = vec![Arc::new("wrong".to_string()) as Arc<dyn Any + Send + Sync>];
    let result = Vec::<i32>::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Type mismatch"));
}

#[tokio::test]
async fn test_arc_extract_wrong_dependency_count() {
    let deps = vec![];
    let result = Arc::<i32>::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 0"));
}

#[tokio::test]
async fn test_arc_extract_downcast_failure() {
    let deps = vec![Arc::new("wrong".to_string()) as Arc<dyn Any + Send + Sync>];
    let result = Arc::<i32>::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Type mismatch"));
}

#[tokio::test]
async fn test_tuple_wrong_dependency_count() {
    let deps = vec![
        Arc::new(0) as Arc<dyn Any + Send + Sync>,
        Arc::new(0) as Arc<dyn Any + Send + Sync>,
        Arc::new(0) as Arc<dyn Any + Send + Sync>,
    ];

    let result = <(i32, i32)>::extract_from_deps(deps).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Expected 2 dependencies, got 3"));
}
