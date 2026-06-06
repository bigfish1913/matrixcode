/// A simple function for LSP testing
pub fn hello_world() -> String {
    "Hello, World!".to_string()
}

/// Adds two numbers
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello() {
        assert_eq!(hello_world(), "Hello, World!");
    }

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }
}