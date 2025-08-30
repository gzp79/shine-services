// Example usage of the new type-erased function approach

use bevy::math::Vec3;

// Example of how to use the new approach:
fn example_usage() {
    let mut param: Vec3 = Vec3::new(1.0, 2.0, 3.0);
    
    // Old approach - had to wrap in ValueType:
    // param.set_with_value(&|v| match v {
    //     ValueType::Vec3(vec) => ValueType::Vec3(vec * 2.0),
    //     _ => v,
    // });
    
    // New approach - completely type-safe, no ValueType wrapping:
    let scale_function = TypedFunction::new(|vec: Vec3| vec * 2.0);
    param.set_with_function(&scale_function).unwrap();
    
    // You can also create more complex functions:
    let normalize_and_scale = TypedFunction::new(|vec: Vec3| {
        if vec.length() > 0.0 {
            vec.normalize() * 5.0
        } else {
            Vec3::ZERO
        }
    });
    
    param.set_with_function(&normalize_and_scale).unwrap();
}

// The key benefits:
// 1. No ValueType wrapping - work directly with concrete types (Vec3, f32, etc.)
// 2. Type safety - the compiler ensures type correctness
// 3. Object safety - the trait remains object-safe
// 4. Clean API - much more ergonomic than the ValueType approach
// 5. Performance - no unnecessary conversions through ValueType
