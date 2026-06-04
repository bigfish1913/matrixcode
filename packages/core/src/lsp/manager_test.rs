//! LSP Manager Tests

use crate::lsp::{LspConfig, LspManager, LspServerConfig, LspServerInfo, LspServerStatus};

#[test]
fn test_lsp_manager_creation() {
    let manager = LspManager::new();
    
    assert!(!manager.has_servers());
    assert_eq!(manager.server_names().len(), 0);
    assert_eq!(manager.connected_count(), 0);
    assert_eq!(manager.error_count(), 0);
}

#[test]
fn test_lsp_manager_from_config() {
    let mut config = LspConfig::new();
    config.add_server(LspServerConfig::new("rust-analyzer", "rust"));
    config.add_server(LspServerConfig::new("typescript-language-server", "typescript"));
    
    let manager = LspManager::from_config(&config);
    
    assert!(manager.has_servers());
    assert_eq!(manager.server_names().len(), 2);
    assert_eq!(manager.connected_count(), 0); // 初始状态未连接
}

#[test]
fn test_lsp_manager_add_server() {
    let mut manager = LspManager::new();
    
    manager.add_server(LspServerConfig::new("rust-analyzer", "rust"));
    
    assert!(manager.has_servers());
    assert_eq!(manager.server_names().len(), 1);
    assert_eq!(manager.get_status("rust"), LspServerStatus::NotStarted);
}

#[test]
fn test_lsp_manager_remove_server() {
    let mut manager = LspManager::new();
    
    manager.add_server(LspServerConfig::new("rust-analyzer", "rust"));
    assert!(manager.has_servers());
    
    manager.remove_server("rust");
    assert!(!manager.has_servers());
}

#[test]
fn test_lsp_manager_status_update() {
    let mut manager = LspManager::new();
    manager.add_server(LspServerConfig::new("rust-analyzer", "rust"));
    
    // 测试状态更新
    manager.mark_starting("rust");
    assert_eq!(manager.get_status("rust"), LspServerStatus::Starting);
    
    manager.mark_connected("rust");
    assert_eq!(manager.get_status("rust"), LspServerStatus::Connected);
    assert_eq!(manager.connected_count(), 1);
    
    manager.mark_error("rust", "Connection failed");
    assert!(matches!(manager.get_status("rust"), LspServerStatus::Error(_)));
    assert_eq!(manager.error_count(), 1);
}

#[test]
fn test_lsp_manager_server_infos() {
    let mut manager = LspManager::new();
    manager.add_server(LspServerConfig::new("rust-analyzer", "rust"));
    manager.add_server(LspServerConfig::new("typescript-language-server", "typescript"));
    
    manager.mark_connected("rust");
    manager.mark_error("typescript", "Not installed");
    
    let infos = manager.server_infos();
    assert_eq!(infos.len(), 2);
    
    let rust_info = infos.iter().find(|i| i.language == "rust").unwrap();
    assert_eq!(rust_info.status, LspServerStatus::Connected);
    assert!(rust_info.status.is_ok());
    assert!(!rust_info.status.is_error());
    
    let ts_info = infos.iter().find(|i| i.language == "typescript").unwrap();
    assert!(matches!(ts_info.status, LspServerStatus::Error(_)));
    assert!(!ts_info.status.is_ok());
    assert!(ts_info.status.is_error());
}

#[test]
fn test_lsp_manager_reset_all() {
    let mut manager = LspManager::new();
    manager.add_server(LspServerConfig::new("rust-analyzer", "rust"));
    
    manager.mark_connected("rust");
    assert_eq!(manager.get_status("rust"), LspServerStatus::Connected);
    
    manager.reset_all();
    assert_eq!(manager.get_status("rust"), LspServerStatus::NotStarted);
}

#[test]
fn test_lsp_status_helpers() {
    // 测试 LspServerStatus 的辅助方法
    assert!(LspServerStatus::NotStarted.label() == "off");
    assert!(LspServerStatus::Starting.label() == "starting...");
    assert!(LspServerStatus::Connected.label() == "ok");
    
    assert!(!LspServerStatus::NotStarted.is_ok());
    assert!(!LspServerStatus::NotStarted.is_error());
    
    assert!(LspServerStatus::Connected.is_ok());
    assert!(!LspServerStatus::Connected.is_error());
    
    assert!(!LspServerStatus::Error("test".into()).is_ok());
    assert!(LspServerStatus::Error("test".into()).is_error());
}

#[test]
fn test_lsp_server_info_with_status() {
    let info = LspServerInfo::new("rust-analyzer", "rust");
    assert_eq!(info.status, LspServerStatus::NotStarted);
    
    let connected_info = info.with_status(LspServerStatus::Connected);
    assert_eq!(connected_info.status, LspServerStatus::Connected);
    assert_eq!(connected_info.name, info.name);
    assert_eq!(connected_info.language, info.language);
}