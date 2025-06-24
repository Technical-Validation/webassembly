mod utils;

use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet(from: &str) {
    alert(&format!("Hello, {}! >>> From WebAssembly", from));
}

#[wasm_bindgen]
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[wasm_bindgen]
pub fn json_reverse(json: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(json).unwrap_or_default();

    match parsed {
        serde_json::Value::Object(map) => {
            let reversed: serde_json::Map<String, serde_json::Value> = map
                .into_iter()
                .map(|(k, v)| (v.to_string(), serde_json::Value::String(k)))
                .collect();
            serde_json::to_string(&reversed).unwrap_or_default()
        }
        _ => String::from("{}"), // 如果不是对象类型，返回空对象
    }
}
