#![cfg(feature = "serde")]
use orx_concurrent_vec::*;

#[test]
fn serialize() {
    let empty_vec = ConcurrentVec::<String>::new();
    let json = serde_json::to_string(&empty_vec).unwrap();
    assert_eq!(json, "[]");

    let vec = ConcurrentVec::new();
    for i in 0..7 {
        vec.push(i.to_string());
    }
    let json = serde_json::to_string(&vec).unwrap();
    assert_eq!(json, "[\"0\",\"1\",\"2\",\"3\",\"4\",\"5\",\"6\"]");
}

#[test]
fn deserialize() {
    let json = "[]";
    let result: Result<ConcurrentVec<String>, _> = serde_json::from_str(json);
    assert!(result.is_ok());
    let empty_vec = result.unwrap();
    assert!(empty_vec.is_empty());

    let json = "[0, 1, 2, 3, 4, 5, 6]";
    let result: Result<ConcurrentVec<u64>, _> = serde_json::from_str(json);
    assert!(result.is_ok());
    let vec = result.unwrap();
    assert_eq!(vec, [0, 1, 2, 3, 4, 5, 6]);
}
