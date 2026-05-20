use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoMemory {
    pub entries: Vec<MemoryEntry>,
    #[serde(default)]
    pub config: MemoryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryConfig {}

fn main() {
    // This would fail because file is an array, not an object
    let json_array = r#"[{"id": "1", "content": "test"}]"#;
    let json_object = r#"{"entries": [{"id": "1", "content": "test"}]}"#;
    
    println!("Array format (current file): {}", json_array);
    println!("Object format (expected): {}", json_object);
}
