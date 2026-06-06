/// A simple function for testing edit functionality
pub fn hello_world() -> String {
    "Hello, MatrixCode!".to_string()
}

/// Adds two numbers together
/// 
/// # Arguments
/// * `a` - First number
/// * `b` - Second number
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello() {
        assert_eq!(hello_world(), "Hello, MatrixCode!");
    }

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }
}