//! Coverage tests for uncovered error paths in extract.rs
//!
//! These tests specifically target type mismatch error messages that aren't hit
//! in normal usage because the #[task] macro generates type-safe code.
//! We directly call extract_from_deps with mismatched types.

use crate::extract::ExtractInput;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_result_type_mismatch_coverage() {
    // Tests line 95 in extract.rs - Result<T,E> type mismatch error message
    // Create a dependency that provides the WRONG type (String instead of Result<i32, String>)
    // Try to extract as Result<i32, String> - this will fail with type mismatch
    let deps = vec![Arc::new("hello") as Arc<dyn Any + Send + Sync>];
    let result = Result::<i32, String>::extract_from_deps(deps).await;

    // Verify the error message contains the expected type information
    assert!(result.is_err());
    let err_msg = result.unwrap_err();

    // The error should mention Result type mismatch (line 95)
    assert!(
        err_msg.contains("Result"),
        "Expected type mismatch error mentioning Result, got: {}",
        err_msg
    );
    assert!(
        err_msg.contains("Type mismatch"),
        "Expected 'Type mismatch' in error, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_option_type_mismatch_coverage() {
    // Tests line 124 in extract.rs - Option<T> type mismatch error message
    // Create a dependency that provides the WRONG type (i32 instead of Option<i32>)
    // Try to extract as Option<i32> - this will fail with type mismatch
    let deps = vec![Arc::new(42) as Arc<dyn Any + Send + Sync>];
    let result = Option::<i32>::extract_from_deps(deps).await;

    // Verify the error message contains the expected type information
    assert!(result.is_err());
    let err_msg = result.unwrap_err();

    // The error should mention Option type mismatch (line 124)
    assert!(
        err_msg.contains("Option"),
        "Expected type mismatch error mentioning Option, got: {}",
        err_msg
    );
    assert!(
        err_msg.contains("Type mismatch"),
        "Expected 'Type mismatch' in error, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_vec_type_mismatch_coverage() {
    // Tests line 152 in extract.rs - Vec<T> type mismatch error message
    // Create a dependency that provides the WRONG type (String instead of Vec<u8>)
    // Try to extract as Vec<u8> - this will fail with type mismatch
    let deps = vec![Arc::new("not a vec".to_string()) as Arc<dyn Any + Send + Sync>];
    let result = Vec::<u8>::extract_from_deps(deps).await;

    // Verify the error message contains the expected type information
    assert!(result.is_err());
    let err_msg = result.unwrap_err();

    // The error should mention Vec type mismatch (line 152)
    assert!(
        err_msg.contains("Vec"),
        "Expected type mismatch error mentioning Vec, got: {}",
        err_msg
    );
    assert!(
        err_msg.contains("Type mismatch"),
        "Expected 'Type mismatch' in error, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_hashmap_type_mismatch_coverage() {
    // Tests line 181 in extract.rs - HashMap<K,V> type mismatch error message
    // Create a dependency that provides the WRONG type (Vec<i32> instead of HashMap)
    // Try to extract as HashMap<String, i32> - this will fail with type mismatch
    let deps = vec![Arc::new(vec![1, 2, 3]) as Arc<dyn Any + Send + Sync>];
    let result = HashMap::<String, i32>::extract_from_deps(deps).await;

    // Verify the error message contains the expected type information
    assert!(result.is_err());
    let err_msg = result.unwrap_err();

    // The error should mention HashMap type mismatch (line 181)
    assert!(
        err_msg.contains("HashMap"),
        "Expected type mismatch error mentioning HashMap, got: {}",
        err_msg
    );
    assert!(
        err_msg.contains("Type mismatch"),
        "Expected 'Type mismatch' in error, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_arc_type_mismatch_coverage() {
    // Tests line 211 in extract.rs - Arc<T> type mismatch error message
    // Arc is special: it expects Arc<Arc<T>> from channel
    // Create a dependency that provides Arc<String> instead of Arc<Arc<String>>
    let deps = vec![Arc::new("not double arc".to_string()) as Arc<dyn Any + Send + Sync>];

    // Try to extract as Arc<String> - this will fail with type mismatch
    // because it expects Arc<Arc<String>> in the channel
    let result = Arc::<String>::extract_from_deps(deps).await;

    // Verify the error message contains the expected type information
    assert!(result.is_err());
    let err_msg = result.unwrap_err();

    // The error should mention Arc type mismatch (line 211)
    assert!(
        err_msg.contains("Arc"),
        "Expected type mismatch error mentioning Arc, got: {}",
        err_msg
    );
    assert!(
        err_msg.contains("Type mismatch"),
        "Expected 'Type mismatch' in error, got: {}",
        err_msg
    );
}

// Additional tests for better coverage of complex scenarios

#[tokio::test]
async fn test_result_with_wrong_ok_type() {
    // Test Result with wrong Ok type
    // Try to extract as Result<i32, String> - wrong Ok type
    let deps = vec![Arc::new(Ok::<_, String>("test".to_string())) as Arc<dyn Any + Send + Sync>];
    let result = Result::<i32, String>::extract_from_deps(deps).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Result") && err_msg.contains("Type mismatch"),
        "Expected Result type mismatch, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_result_with_wrong_err_type() {
    // Test Result with wrong Err type
    // Try to extract as Result<i32, String> - wrong Err type
    let deps = vec![Arc::new(Err::<i32, i32>(42)) as Arc<dyn Any + Send + Sync>];
    let result = Result::<i32, String>::extract_from_deps(deps).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Result") && err_msg.contains("Type mismatch"),
        "Expected Result type mismatch, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_option_with_wrong_inner_type() {
    // Test Option with wrong inner type
    // Try to extract as Option<i32> - wrong inner type
    let deps = vec![Arc::new(Some("test".to_string())) as Arc<dyn Any + Send + Sync>];
    let result = Option::<i32>::extract_from_deps(deps).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Option") && err_msg.contains("Type mismatch"),
        "Expected Option type mismatch, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_vec_with_wrong_element_type() {
    // Test Vec with wrong element type
    // Try to extract as Vec<i32> - wrong element type
    let deps = vec![Arc::new(vec!["a".to_string(), "b".to_string()]) as Arc<dyn Any + Send + Sync>];
    let result = Vec::<i32>::extract_from_deps(deps).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Vec") && err_msg.contains("Type mismatch"),
        "Expected Vec type mismatch, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_hashmap_with_wrong_key_type() {
    // Test HashMap with wrong key type
    let mut map = HashMap::new();
    map.insert(1, "one".to_string());
    // Try to extract as HashMap<String, String> - wrong key type
    let deps = vec![Arc::new(map) as Arc<dyn Any + Send + Sync>];
    let result = HashMap::<String, String>::extract_from_deps(deps).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("HashMap") && err_msg.contains("Type mismatch"),
        "Expected HashMap type mismatch, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_hashmap_with_wrong_value_type() {
    // Test HashMap with wrong value type
    let mut map = HashMap::new();
    map.insert("key".to_string(), 42);
    // Try to extract as HashMap<String, String> - wrong value type
    let deps = vec![Arc::new(map) as Arc<dyn Any + Send + Sync>];
    let result = HashMap::<String, String>::extract_from_deps(deps).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("HashMap") && err_msg.contains("Type mismatch"),
        "Expected HashMap type mismatch, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_arc_with_wrong_inner_type() {
    // Test Arc with completely wrong inner type
    // Send Arc<i32> but expect Arc<String>
    // Try to extract as Arc<String> - wrong inner type
    let deps = vec![Arc::new(Arc::new(Arc::new(42))) as Arc<dyn Any + Send + Sync>];
    let result = Arc::<String>::extract_from_deps(deps).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Arc") && err_msg.contains("Type mismatch"),
        "Expected Arc type mismatch, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_primitive_type_mismatch_for_completeness() {
    // Test primitive type mismatch (already covered in existing tests but good for completeness)
    // Try to extract as i32 - type mismatch
    let deps = vec![Arc::new("not an i32".to_string()) as Arc<dyn Any + Send + Sync>];
    let result = i32::extract_from_deps(deps).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Type mismatch"),
        "Expected type mismatch error, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_complex_nested_result_option() {
    // Test complex nested type: send Result<String, String>, expect Result<Option<i32>, String>
    // Try to extract as Result<Option<i32>, String> - nested type mismatch
    let deps =
        vec![Arc::new(Ok::<String, String>("test".to_string())) as Arc<dyn Any + Send + Sync>];
    let result = Result::<Option<i32>, String>::extract_from_deps(deps).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Result") && err_msg.contains("Type mismatch"),
        "Expected Result type mismatch, got: {}",
        err_msg
    );
}
