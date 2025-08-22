//! Tests for serde (de)serialization of Transform / TransformFunction shapes.
use serde_json::json;

use takumi::layout::style::{Transform, TransformFunction};

#[test]
fn deserialize_rotate_transform() {
  let value = json!([{ "rotate": 45 }]);
  let transforms: Transform =
    serde_json::from_value(value).expect("should deserialize rotate transform");
  assert_eq!(transforms.0.len(), 1);
  match &transforms.0[0] {
    TransformFunction::Rotate(rotate) => assert!((*rotate - 45.0).abs() < f32::EPSILON),
    other => panic!("unexpected variant: {:?}", other),
  }
}
