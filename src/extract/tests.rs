//! Unit tests for extract module

#![allow(dead_code)] // Test helpers generate unused code via macro

use crate::extract::ExtractInput;
use crate::task::Task;
use futures::channel::oneshot;
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
    async fn run(&self) -> i32 {
        self.value
    }
}

#[allow(dead_code)]
struct TestTaskWithInput;

#[crate::task]
#[allow(dead_code)]
impl TestTaskWithInput {
    async fn run(input: &i32) -> i32 {
        input * 2
    }
}

#[tokio::test]
async fn test_extract_result_type_single_dependency_error() {
    // Test Result<T,E> with wrong dependency count
    let deps = vec![
        Box::new(oneshot::channel::<Result<i32, String>>().1) as Box<dyn Any + Send>,
        Box::new(oneshot::channel::<Result<i32, String>>().1) as Box<dyn Any + Send>,
    ]; // Two dependencies when expecting 1

    let result = Result::<i32, String>::extract_from_channels(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 2"));
}

#[tokio::test]
async fn test_extract_result_type_success() {
    // Test Result<T,E> successful extraction
    let (tx, rx) = oneshot::channel::<Arc<Result<i32, String>>>();

    // Send value in background
    tokio::spawn(async move {
        let _ = tx.send(Arc::new(Ok(42)));
    });

    let deps = vec![Box::new(rx) as Box<dyn Any + Send>];
    let result = Result::<i32, String>::extract_from_channels(deps).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Ok(42));
}

#[tokio::test]
async fn test_extract_option_type_single_dependency_error() {
    // Test Option<T> with wrong dependency count
    let deps = vec![
        Box::new(oneshot::channel::<Option<String>>().1) as Box<dyn Any + Send>,
        Box::new(oneshot::channel::<Option<String>>().1) as Box<dyn Any + Send>,
        Box::new(oneshot::channel::<Option<String>>().1) as Box<dyn Any + Send>,
    ]; // Three dependencies when expecting 1

    let result = Option::<String>::extract_from_channels(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 3"));
}

#[tokio::test]
async fn test_extract_option_type_success() {
    // Test Option<T> successful extraction
    let (tx, rx) = oneshot::channel::<Arc<Option<i32>>>();

    // Send value in background
    tokio::spawn(async move {
        let _ = tx.send(Arc::new(Some(99)));
    });

    let deps = vec![Box::new(rx) as Box<dyn Any + Send>];
    let result = Option::<i32>::extract_from_channels(deps).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some(99));
}

#[tokio::test]
async fn test_extract_vec_type_single_dependency_error() {
    // Test Vec<T> with wrong dependency count
    let deps = vec![]; // Zero dependencies when expecting 1

    let result = Vec::<u8>::extract_from_channels(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 0"));
}

#[tokio::test]
async fn test_extract_vec_type_success() {
    // Test Vec<T> successful extraction
    let (tx, rx) = oneshot::channel::<Arc<Vec<i32>>>();

    // Send value in background
    tokio::spawn(async move {
        let _ = tx.send(Arc::new(vec![1, 2, 3]));
    });

    let deps = vec![Box::new(rx) as Box<dyn Any + Send>];
    let result = Vec::<i32>::extract_from_channels(deps).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), vec![1, 2, 3]);
}

#[tokio::test]
async fn test_extract_primitive_single_dependency_error() {
    // Test primitive types with wrong dependency count
    let deps = vec![
        Box::new(oneshot::channel::<i32>().1) as Box<dyn Any + Send>,
        Box::new(oneshot::channel::<i32>().1) as Box<dyn Any + Send>,
    ]; // Two dependencies when expecting 1

    let result = i32::extract_from_channels(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 2"));

    // Test with String
    let deps = vec![
        Box::new(oneshot::channel::<String>().1) as Box<dyn Any + Send>,
        Box::new(oneshot::channel::<String>().1) as Box<dyn Any + Send>,
    ];
    let result = String::extract_from_channels(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 2"));

    // Test with f64
    let deps = vec![
        Box::new(oneshot::channel::<f64>().1) as Box<dyn Any + Send>,
        Box::new(oneshot::channel::<f64>().1) as Box<dyn Any + Send>,
    ];
    let result = f64::extract_from_channels(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Expected 1 dependency, got 2"));
}

#[tokio::test]
async fn test_extract_unit_type() {
    // Test that unit type always succeeds regardless of dependencies
    let deps = vec![];
    let result = <()>::extract_from_channels(deps).await;
    assert!(result.is_ok());

    // Even with dependencies (ignored), unit type succeeds
    let deps = vec![
        Box::new(oneshot::channel::<i32>().1) as Box<dyn Any + Send>,
        Box::new(oneshot::channel::<i32>().1) as Box<dyn Any + Send>,
    ];
    let result = <()>::extract_from_channels(deps).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_extract_tuple_basic() {
    // Test tuple extraction with channels
    let (tx1, rx1) = oneshot::channel::<Arc<i32>>();
    let (tx2, rx2) = oneshot::channel::<Arc<i32>>();

    // Send values in background
    tokio::spawn(async move {
        let _ = tx1.send(Arc::new(42));
        let _ = tx2.send(Arc::new(100));
    });

    let deps = vec![
        Box::new(rx1) as Box<dyn Any + Send>,
        Box::new(rx2) as Box<dyn Any + Send>,
    ];

    // Extract a tuple (i32, i32)
    let result = <(i32, i32)>::extract_from_channels(deps).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), (42, 100));
}

#[tokio::test]
async fn test_extract_channel_closed() {
    // Test error when channel is closed
    let (tx, rx) = oneshot::channel::<Arc<i32>>();
    // Explicitly drop tx to close the channel
    drop(tx);

    let deps = vec![Box::new(rx) as Box<dyn Any + Send>];
    let result = i32::extract_from_channels(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Channel closed"));
}

#[tokio::test]
async fn test_extract_type_mismatch() {
    // Test type mismatch during downcast
    let (tx, rx) = oneshot::channel::<String>();

    // Send a String
    tokio::spawn(async move {
        let _ = tx.send("hello".to_string());
    });

    // Try to extract as wrong type - put wrong receiver type in deps
    let deps = vec![Box::new(rx) as Box<dyn Any + Send>];
    let result = i32::extract_from_channels(deps).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Type mismatch"));
}

#[tokio::test]
async fn test_extract_all_primitive_types() {
    // Test that all primitive types in impl_extract_single! work correctly
    macro_rules! test_type {
        ($t:ty, $val:expr) => {
            let (tx, rx) = oneshot::channel::<Arc<$t>>();

            tokio::spawn(async move {
                let _ = tx.send(Arc::new($val));
            });

            let deps = vec![Box::new(rx) as Box<dyn Any + Send>];
            let result = <$t>::extract_from_channels(deps).await;
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
    let (tx1, rx1) = oneshot::channel::<Arc<i32>>();
    let (tx2, rx2) = oneshot::channel::<Arc<i32>>();
    let (tx3, rx3) = oneshot::channel::<Arc<i32>>();
    let (tx4, rx4) = oneshot::channel::<Arc<i32>>();
    let (tx5, rx5) = oneshot::channel::<Arc<i32>>();

    // Send values in background
    tokio::spawn(async move {
        let _ = tx1.send(Arc::new(0));
        let _ = tx2.send(Arc::new(1));
        let _ = tx3.send(Arc::new(2));
        let _ = tx4.send(Arc::new(3));
        let _ = tx5.send(Arc::new(4));
    });

    let deps = vec![
        Box::new(rx1) as Box<dyn Any + Send>,
        Box::new(rx2) as Box<dyn Any + Send>,
        Box::new(rx3) as Box<dyn Any + Send>,
        Box::new(rx4) as Box<dyn Any + Send>,
        Box::new(rx5) as Box<dyn Any + Send>,
    ];

    // Extract a 5-element tuple
    let result = <(i32, i32, i32, i32, i32)>::extract_from_channels(deps).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), (0, 1, 2, 3, 4));
}

#[tokio::test]
async fn test_extract_arc_type() {
    // Test Arc<T> extraction
    let (tx, rx) = oneshot::channel::<Arc<Arc<String>>>();

    let test_arc = Arc::new("test".to_string());
    let test_arc_clone = test_arc.clone();

    tokio::spawn(async move {
        let _ = tx.send(Arc::new(test_arc));
    });

    let deps = vec![Box::new(rx) as Box<dyn Any + Send>];
    let result = Arc::<String>::extract_from_channels(deps).await;
    assert!(result.is_ok());
    assert_eq!(*result.unwrap(), *test_arc_clone);
}
