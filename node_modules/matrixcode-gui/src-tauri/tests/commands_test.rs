//! Basic tests for MatrixCode GUI backend logic.

use matrixcode_gui::AppState;

#[test]
fn test_appstate_new() {
    // AppState::new() should succeed (loads config and session manager)
    let _state = AppState::new();
}
