//! LSP integration tests

use matrixcode_core::lsp::{LspServerConfig, LspClientRegistry};
use std::path::PathBuf;

// ============================================================================
// Registry Tests
// ============================================================================

#[tokio::test]
async fn test_lsp_registry_creation() {
    let registry = LspClientRegistry::new();
    assert!(!registry.has_active_clients().await);
}

#[tokio::test]
async fn test_lsp_registry_active_languages() {
    let registry = LspClientRegistry::new();
    let languages = registry.active_languages().await;
    assert!(languages.is_empty());
}

#[tokio::test]
async fn test_lsp_registry_default() {
    let registry = LspClientRegistry::default();
    assert!(!registry.has_active_clients().await);
}

// ============================================================================
// LspServerConfig Tests
// ============================================================================

#[test]
fn test_lsp_server_config_creation() {
    let config = LspServerConfig::new("rust-analyzer", "rust");
    assert_eq!(config.command, "rust-analyzer");
    assert_eq!(config.language, "rust");
    assert!(config.enabled);
    assert!(config.args.is_empty());
}

#[test]
fn test_lsp_server_config_with_args() {
    let config = LspServerConfig::new("typescript-language-server", "typescript")
        .with_args(vec!["--stdio".to_string()]);
    assert_eq!(config.command, "typescript-language-server");
    assert_eq!(config.language, "typescript");
    assert_eq!(config.args.len(), 1);
    assert_eq!(config.args[0], "--stdio");
    assert!(config.enabled);
}

#[test]
fn test_lsp_server_config_with_multiple_args() {
    let config = LspServerConfig::new("gopls", "go")
        .with_args(vec!["-mode=go".to_string(), "-verbose".to_string()]);
    assert_eq!(config.command, "gopls");
    assert_eq!(config.args.len(), 2);
    assert_eq!(config.args[0], "-mode=go");
    assert_eq!(config.args[1], "-verbose");
}

// ============================================================================
// LspServerStatus Tests (via types module)
// ============================================================================

#[test]
fn test_lsp_server_status_is_ok() {
    use matrixcode_core::lsp::LspServerStatus;

    assert!(LspServerStatus::Connected.is_ok());
    assert!(!LspServerStatus::NotStarted.is_ok());
    assert!(!LspServerStatus::Error("test".to_string()).is_ok());
}

#[test]
fn test_lsp_server_status_is_error() {
    use matrixcode_core::lsp::LspServerStatus;

    assert!(LspServerStatus::Error("test".to_string()).is_error());
    assert!(!LspServerStatus::Connected.is_error());
    assert!(!LspServerStatus::NotStarted.is_error());
}

#[test]
fn test_lsp_server_status_label() {
    use matrixcode_core::lsp::LspServerStatus;

    assert_eq!(LspServerStatus::NotStarted.label(), "off");
    assert_eq!(LspServerStatus::Connected.label(), "ok");
    assert_eq!(LspServerStatus::Error("timeout".to_string()).label(), "err: timeout");
}

// ============================================================================
// LspServerInfo Tests
// ============================================================================

#[test]
fn test_lsp_server_info_creation() {
    use matrixcode_core::lsp::{LspServerInfo, LspServerStatus};

    let info = LspServerInfo::new("rust-analyzer", "rust");
    assert_eq!(info.name, "rust-analyzer");
    assert_eq!(info.language, "rust");
    assert_eq!(info.status, LspServerStatus::NotStarted);
}

#[test]
fn test_lsp_server_info_connected() {
    use matrixcode_core::lsp::{LspServerInfo, LspServerStatus};

    let info = LspServerInfo::connected("rust-analyzer", "rust");
    assert_eq!(info.status, LspServerStatus::Connected);
}

#[test]
fn test_lsp_server_info_error() {
    use matrixcode_core::lsp::{LspServerInfo, LspServerStatus};

    let info = LspServerInfo::error("rust-analyzer", "rust", "connection failed");
    assert!(matches!(info.status, LspServerStatus::Error(_)));
    if let LspServerStatus::Error(msg) = info.status {
        assert_eq!(msg, "connection failed");
    }
}

#[test]
fn test_lsp_server_info_with_status() {
    use matrixcode_core::lsp::{LspServerInfo, LspServerStatus};

    let info = LspServerInfo::new("rust-analyzer", "rust")
        .with_status(LspServerStatus::Connected);
    assert_eq!(info.status, LspServerStatus::Connected);
}

// ============================================================================
// LspConfig Tests
// ============================================================================

#[test]
fn test_lsp_config_new() {
    use matrixcode_core::lsp::LspConfig;

    let config = LspConfig::new();
    assert!(config.servers.is_empty());
}

#[test]
fn test_lsp_config_add_server() {
    use matrixcode_core::lsp::LspConfig;

    let mut config = LspConfig::new();
    config.add_server(LspServerConfig::new("rust-analyzer", "rust"));
    assert_eq!(config.servers.len(), 1);
    assert_eq!(config.servers[0].command, "rust-analyzer");
}

#[test]
fn test_lsp_config_enabled_servers() {
    use matrixcode_core::lsp::LspConfig;

    let mut config = LspConfig::new();
    config.add_server(LspServerConfig::new("rust-analyzer", "rust"));

    // Add a disabled server
    let mut disabled = LspServerConfig::new("disabled-server", "test");
    disabled.enabled = false;
    config.add_server(disabled);

    let enabled = config.enabled_servers();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].command, "rust-analyzer");
}

// ============================================================================
// Default Config Tests
// ============================================================================

#[test]
fn test_default_rust_analyzer_config() {
    use matrixcode_core::lsp::default_rust_analyzer_config;

    let config = default_rust_analyzer_config();
    assert_eq!(config.command, "rust-analyzer");
    assert_eq!(config.language, "rust");
    assert!(config.enabled);
}

#[test]
fn test_default_typescript_config() {
    use matrixcode_core::lsp::default_typescript_config;

    let config = default_typescript_config();
    assert_eq!(config.command, "typescript-language-server");
    assert_eq!(config.language, "typescript");
    assert!(config.args.contains(&"--stdio".to_string()));
}

#[test]
fn test_default_python_config() {
    use matrixcode_core::lsp::default_python_config;

    let config = default_python_config();
    assert_eq!(config.command, "pyright-langserver");
    assert_eq!(config.language, "python");
    assert!(config.args.contains(&"--stdio".to_string()));
}

#[test]
fn test_default_lsp_config() {
    use matrixcode_core::lsp::default_lsp_config;

    let config = default_lsp_config();
    assert!(!config.servers.is_empty());
    assert!(config.servers.iter().any(|s| s.language == "rust"));
}

// ============================================================================
// LSP Context Injection Tests
// ============================================================================

#[test]
fn test_should_inject_lsp_context() {
    use matrixcode_core::lsp::{should_inject_lsp_context, LspServerInfo};

    // Empty servers - should not inject
    let servers: Vec<LspServerInfo> = vec![];
    assert!(!should_inject_lsp_context(&servers));

    // Only not-started servers - should not inject
    let servers = vec![LspServerInfo::new("rust-analyzer", "rust")];
    assert!(!should_inject_lsp_context(&servers));

    // Connected server - should inject
    let servers = vec![LspServerInfo::connected("rust-analyzer", "rust")];
    assert!(should_inject_lsp_context(&servers));

    // Mixed servers with at least one connected - should inject
    let servers = vec![
        LspServerInfo::new("gopls", "go"),
        LspServerInfo::connected("rust-analyzer", "rust"),
    ];
    assert!(should_inject_lsp_context(&servers));
}

#[test]
fn test_get_active_lsp_servers() {
    use matrixcode_core::lsp::{get_active_lsp_servers, LspServerInfo};

    let servers = vec![
        LspServerInfo::new("gopls", "go"),
        LspServerInfo::connected("rust-analyzer", "rust"),
        LspServerInfo::error("pyright", "python", "failed"),
    ];

    let active = get_active_lsp_servers(&servers);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, "rust-analyzer");
}

// ============================================================================
// Language Detection Tests
// ============================================================================

#[test]
fn test_detect_language_from_path_rust() {
    use matrixcode_core::lsp::detect_language_from_path;

    assert_eq!(
        detect_language_from_path(&PathBuf::from("src/main.rs")),
        Some("rust".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("lib.rs")),
        Some("rust".to_string())
    );
}

#[test]
fn test_detect_language_from_path_typescript() {
    use matrixcode_core::lsp::detect_language_from_path;

    assert_eq!(
        detect_language_from_path(&PathBuf::from("app.ts")),
        Some("typescript".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("component.tsx")),
        Some("typescript".to_string())
    );
}

#[test]
fn test_detect_language_from_path_javascript() {
    use matrixcode_core::lsp::detect_language_from_path;

    assert_eq!(
        detect_language_from_path(&PathBuf::from("index.js")),
        Some("javascript".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("App.jsx")),
        Some("javascript".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("module.mjs")),
        Some("javascript".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("common.cjs")),
        Some("javascript".to_string())
    );
}

#[test]
fn test_detect_language_from_path_python() {
    use matrixcode_core::lsp::detect_language_from_path;

    assert_eq!(
        detect_language_from_path(&PathBuf::from("main.py")),
        Some("python".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("__init__.pyi")),
        Some("python".to_string())
    );
}

#[test]
fn test_detect_language_from_path_go() {
    use matrixcode_core::lsp::detect_language_from_path;

    assert_eq!(
        detect_language_from_path(&PathBuf::from("main.go")),
        Some("go".to_string())
    );
}

#[test]
fn test_detect_language_from_path_c_cpp() {
    use matrixcode_core::lsp::detect_language_from_path;

    assert_eq!(
        detect_language_from_path(&PathBuf::from("main.c")),
        Some("c".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("header.h")),
        Some("c".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("main.cpp")),
        Some("cpp".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("header.hpp")),
        Some("cpp".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("impl.cc")),
        Some("cpp".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("impl.cxx")),
        Some("cpp".to_string())
    );
}

#[test]
fn test_detect_language_from_path_java() {
    use matrixcode_core::lsp::detect_language_from_path;

    assert_eq!(
        detect_language_from_path(&PathBuf::from("Main.java")),
        Some("java".to_string())
    );
}

#[test]
fn test_detect_language_from_path_other_languages() {
    use matrixcode_core::lsp::detect_language_from_path;

    // C#
    assert_eq!(
        detect_language_from_path(&PathBuf::from("Program.cs")),
        Some("csharp".to_string())
    );

    // Ruby
    assert_eq!(
        detect_language_from_path(&PathBuf::from("app.rb")),
        Some("ruby".to_string())
    );

    // PHP
    assert_eq!(
        detect_language_from_path(&PathBuf::from("index.php")),
        Some("php".to_string())
    );

    // Swift
    assert_eq!(
        detect_language_from_path(&PathBuf::from("main.swift")),
        Some("swift".to_string())
    );

    // Kotlin
    assert_eq!(
        detect_language_from_path(&PathBuf::from("Main.kt")),
        Some("kotlin".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("build.gradle.kts")),
        Some("kotlin".to_string())
    );

    // Scala
    assert_eq!(
        detect_language_from_path(&PathBuf::from("Main.scala")),
        Some("scala".to_string())
    );

    // Lua
    assert_eq!(
        detect_language_from_path(&PathBuf::from("init.lua")),
        Some("lua".to_string())
    );
}

#[test]
fn test_detect_language_from_path_web() {
    use matrixcode_core::lsp::detect_language_from_path;

    // Vue
    assert_eq!(
        detect_language_from_path(&PathBuf::from("App.vue")),
        Some("vue".to_string())
    );

    // Svelte
    assert_eq!(
        detect_language_from_path(&PathBuf::from("Component.svelte")),
        Some("svelte".to_string())
    );

    // HTML
    assert_eq!(
        detect_language_from_path(&PathBuf::from("index.html")),
        Some("html".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("page.htm")),
        Some("html".to_string())
    );

    // CSS
    assert_eq!(
        detect_language_from_path(&PathBuf::from("style.css")),
        Some("css".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("style.scss")),
        Some("scss".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("style.less")),
        Some("less".to_string())
    );
}

#[test]
fn test_detect_language_from_path_config() {
    use matrixcode_core::lsp::detect_language_from_path;

    // JSON
    assert_eq!(
        detect_language_from_path(&PathBuf::from("package.json")),
        Some("json".to_string())
    );

    // YAML
    assert_eq!(
        detect_language_from_path(&PathBuf::from("config.yaml")),
        Some("yaml".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("config.yml")),
        Some("yaml".to_string())
    );

    // TOML
    assert_eq!(
        detect_language_from_path(&PathBuf::from("Cargo.toml")),
        Some("toml".to_string())
    );

    // Markdown
    assert_eq!(
        detect_language_from_path(&PathBuf::from("README.md")),
        Some("markdown".to_string())
    );
}

#[test]
fn test_detect_language_from_path_shell() {
    use matrixcode_core::lsp::detect_language_from_path;

    assert_eq!(
        detect_language_from_path(&PathBuf::from("script.sh")),
        Some("shell".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("script.bash")),
        Some("shell".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("script.zsh")),
        Some("shell".to_string())
    );
}

#[test]
fn test_detect_language_from_path_unknown() {
    use matrixcode_core::lsp::detect_language_from_path;

    // Unknown extensions return None
    assert_eq!(
        detect_language_from_path(&PathBuf::from("file.unknown")),
        None
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("file.xyz")),
        None
    );

    // No extension returns None
    assert_eq!(
        detect_language_from_path(&PathBuf::from("Makefile")),
        None
    );
}

#[test]
fn test_detect_language_from_path_case_insensitive() {
    use matrixcode_core::lsp::detect_language_from_path;

    // Extension detection is case-insensitive (lowercased)
    assert_eq!(
        detect_language_from_path(&PathBuf::from("file.RS")),
        Some("rust".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("file.TS")),
        Some("typescript".to_string())
    );
    assert_eq!(
        detect_language_from_path(&PathBuf::from("file.PY")),
        Some("python".to_string())
    );
}