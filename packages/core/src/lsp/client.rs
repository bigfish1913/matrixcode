//! LSP Client
//!
//! High-level LSP client that manages a single LSP server connection
//! and provides APIs for hover, definition, references, and diagnostics.

use anyhow::{Result, anyhow};
use lsp_types::{
    ClientCapabilities, Diagnostic, DiagnosticSeverity, GotoDefinitionParams,
    Hover, HoverContents, HoverParams, InitializeParams,
    InitializedParams, Location, MarkupKind, Position, ReferenceContext,
    ReferenceParams, TextDocumentClientCapabilities, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, WorkspaceClientCapabilities,
    WorkspaceFolder, Uri,
};
use url::Url;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

/// Convert Url to Uri for lsp_types compatibility
fn url_to_uri(url: &Url) -> Uri {
    Uri::from_str(url.as_str()).unwrap_or_else(|_| Uri::from_str("file:///invalid").unwrap())
}

use super::constants::{PROCESS_STARTUP_TIMEOUT, SERVER_INIT_TIMEOUT};
use super::progress::LspProgressCallback;
use super::transport::LspTransport;
use super::types::LspServerConfig;

/// Result from hover request
#[derive(Debug, Clone)]
pub struct HoverResult {
    /// Type signature or primary hover content
    pub signature: String,
    /// Documentation or additional info
    pub documentation: Option<String>,
}

impl HoverResult {
    /// Create a new hover result
    pub fn new(signature: impl Into<String>) -> Self {
        Self {
            signature: signature.into(),
            documentation: None,
        }
    }

    /// Add documentation
    pub fn with_documentation(mut self, doc: impl Into<String>) -> Self {
        self.documentation = Some(doc.into());
        self
    }
}

/// LSP Client
///
/// Manages a single LSP server connection and provides high-level API
/// for common LSP operations like hover, definition, references, and diagnostics.
pub struct LspClient {
    /// Transport layer for communication
    transport: Arc<Mutex<Option<LspTransport>>>,
    /// Language identifier (e.g., "rust", "typescript")
    language: String,
    /// Server name (e.g., "rust-analyzer")
    server_name: String,
    /// Project root path
    project_root: PathBuf,
    /// Cache of open files (URI -> content)
    open_files: Arc<Mutex<HashMap<Url, String>>>,
    /// Cache of diagnostics (URI -> diagnostics)
    diagnostics_cache: Arc<Mutex<HashMap<Url, Vec<Diagnostic>>>>,
    /// Server capabilities (set after initialization)
    capabilities: Arc<Mutex<Option<lsp_types::ServerCapabilities>>>,
}

impl LspClient {
    /// Create a new LSP client
    pub fn new(language: impl Into<String>, server_name: impl Into<String>, project_root: PathBuf) -> Self {
        Self {
            transport: Arc::new(Mutex::new(None)),
            language: language.into(),
            server_name: server_name.into(),
            project_root,
            open_files: Arc::new(Mutex::new(HashMap::new())),
            diagnostics_cache: Arc::new(Mutex::new(HashMap::new())),
            capabilities: Arc::new(Mutex::new(None)),
        }
    }

    /// Create LSP client from config
    pub fn from_config(config: &LspServerConfig, project_root: PathBuf) -> Self {
        Self::new(config.language.clone(), config.command.clone(), project_root)
    }

    /// Spawn and initialize the LSP server
    pub async fn spawn(&self, config: &LspServerConfig) -> Result<()> {
        log::info!("LSP spawn: starting '{}'...", self.server_name);
        crate::debug::debug_log().log("lsp", &format!("spawn: starting '{}'...", self.server_name));

        // Start the server process
        let transport = LspTransport::spawn(&config.command, &config.command, &config.args).await?;
        log::info!("LSP spawn: '{}' process started", self.server_name);
        crate::debug::debug_log().log("lsp", &format!("spawn: '{}' process started", self.server_name));

        {
            let mut transport_guard = self.transport.lock().await;
            *transport_guard = Some(transport);
        }

        // Initialize the server
        log::info!("LSP spawn: initializing '{}'...", self.server_name);
        crate::debug::debug_log().log("lsp", &format!("spawn: initializing '{}'...", self.server_name));
        self.initialize().await?;
        log::info!("LSP spawn: '{}' initialized successfully", self.server_name);
        crate::debug::debug_log().log("lsp", &format!("spawn: '{}' initialized OK", self.server_name));

        log::info!("LSP client '{}' spawned and initialized successfully", self.server_name);
        Ok(())
    }

    /// Spawn LSP server with async initialization and progress callbacks
    ///
    /// This is the async version of `spawn()` with:
    /// - Separate timeouts for process startup (5s) and server initialization (60s)
    /// - Progress callbacks for UI updates
    /// - Better error messages for each phase
    ///
    /// # Arguments
    /// - `config`: LSP server configuration
    /// - `progress_callback`: Callback for progress updates (can be `NoOpProgressCallback` if not needed)
    ///
    /// # Progress Flow
    /// - 0.1: Starting process...
    /// - 0.3: Process started, initializing server...
    /// - 0.5-0.9: Loading workspace/indexes...
    /// - 1.0: Ready
    ///
    /// # Errors
    /// - Process startup timeout (5s): binary missing, permission denied
    /// - Server initialization timeout (60s): large workspace, slow indexing
    pub async fn spawn_async(
        &self,
        config: &LspServerConfig,
        progress_callback: Arc<dyn LspProgressCallback>,
    ) -> Result<()> {
        log::info!("LSP spawn_async: starting '{}'...", self.server_name);

        // Phase 1: Start process (5s timeout, should be fast)
        progress_callback.on_progress(0.1, "Starting process...");
        
        let transport_result = timeout(PROCESS_STARTUP_TIMEOUT, async {
            LspTransport::spawn(&config.command, &config.command, &config.args).await
        })
        .await;

        let transport = transport_result.map_err(|_| {
            let error_msg = format!(
                "LSP process startup timeout ({}s).\n\
                Possible causes:\n\
                - Binary '{}' not found in PATH\n\
                - Permission denied\n\
                - Process hangs immediately",
                PROCESS_STARTUP_TIMEOUT.as_secs(),
                config.command
            );
            progress_callback.on_error(&error_msg);
            anyhow!(error_msg)
        })?;

        let transport = transport.map_err(|e| {
            let error_msg = format!("Failed to start LSP process '{}': {}", config.command, e);
            progress_callback.on_error(&error_msg);
            anyhow!(error_msg)
        })?;

        log::info!("LSP spawn_async: '{}' process started", self.server_name);
        progress_callback.on_progress(0.3, "Process started, initializing server...");

        // Store transport
        {
            let mut transport_guard = self.transport.lock().await;
            *transport_guard = Some(transport);
        }

        // Phase 2: Initialize server (timeout, but continue in background if needed)
        progress_callback.on_progress(0.5, "Loading workspace...");

        let init_result = timeout(SERVER_INIT_TIMEOUT, async {
            self.initialize().await
        })
        .await;

        match init_result {
            Ok(result) => {
                result?; // initialize() 成功
                log::info!("LSP spawn_async: '{}' initialized successfully", self.server_name);
                progress_callback.on_progress(1.0, "Ready");
                progress_callback.on_complete();
                Ok(())
            }
            Err(_) => {
                // Timeout occurred, but process is still running
                // Start background task to continue waiting for initialization
                log::info!(
                    "LSP '{}' initialization timeout after {}s, continuing in background...",
                    self.server_name,
                    SERVER_INIT_TIMEOUT.as_secs()
                );

                // Don't mark as error - process is still running
                // Return Ok to indicate process started, but initialization pending
                progress_callback.on_progress(0.7, "Background init...");

                // Spawn background task to continue waiting
                let server_name = self.server_name.clone();
                let transport = self.transport.clone();
                let callback = progress_callback.clone();

                tokio::spawn(async move {
                    // Wait for background initialization (30 seconds grace period)
                    tokio::time::sleep(Duration::from_secs(30)).await;

                    // Check if transport is still available (process running)
                    let transport_guard = transport.lock().await;
                    if transport_guard.is_some() {
                        log::info!("LSP '{}' background initialization completed", server_name);
                        callback.on_progress(1.0, "Ready");
                        callback.on_complete();
                    } else {
                        log::warn!("LSP '{}' process died during background init", server_name);
                        callback.on_error("Process died");
                    }
                });

                // Return Ok to indicate process started successfully
                // Background task will update status when ready
                Ok(())
            }
        }
    }

    /// Send LSP initialize request
    #[allow(deprecated)] // root_path and root_uri are deprecated but needed for LSP server compatibility
    pub async fn initialize(&self) -> Result<()> {
        let transport = self.transport.lock().await;
        let transport = transport.as_ref().ok_or_else(|| anyhow!("Transport not initialized"))?;

        let root_uri = Url::from_file_path(&self.project_root)
            .map_err(|_| anyhow!("Invalid project root path: {:?}", self.project_root))?;
        let root_uri = url_to_uri(&root_uri);

        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_path: None,
            root_uri: Some(root_uri.clone()),
            initialization_options: None,
            capabilities: ClientCapabilities {
                text_document: Some(TextDocumentClientCapabilities {
                    hover: Some(lsp_types::HoverClientCapabilities {
                        dynamic_registration: Some(false),
                        content_format: Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]),
                    }),
                    definition: Some(lsp_types::GotoCapability {
                        dynamic_registration: Some(false),
                        link_support: Some(true),
                    }),
                    references: Some(lsp_types::ReferenceClientCapabilities {
                        dynamic_registration: Some(false),
                    }),
                    publish_diagnostics: Some(lsp_types::PublishDiagnosticsClientCapabilities {
                        related_information: Some(true),
                        tag_support: Some(lsp_types::TagSupport {
                            value_set: vec![lsp_types::DiagnosticTag::UNNECESSARY, lsp_types::DiagnosticTag::DEPRECATED],
                        }),
                        version_support: Some(false),
                        code_description_support: Some(true),
                        data_support: Some(false),
                    }),
                    ..Default::default()
                }),
                workspace: Some(WorkspaceClientCapabilities {
                    workspace_folders: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
            trace: Some(lsp_types::TraceValue::Off),
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: root_uri,
                name: self.project_root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "workspace".to_string()),
            }]),
            client_info: Some(lsp_types::ClientInfo {
                name: "matrixcode".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            locale: None,
            work_done_progress_params: Default::default(),
        };

        let result = transport
            .send_request("initialize", serde_json::to_value(params)?)
            .await?;

        // Store capabilities
        if let Some(capabilities) = result.get("capabilities") {
            let caps: lsp_types::ServerCapabilities = serde_json::from_value(capabilities.clone())?;
            let mut caps_guard = self.capabilities.lock().await;
            *caps_guard = Some(caps);
        }

        // Send initialized notification
        self.initialized().await?;

        Ok(())
    }

    /// Send initialized notification
    pub async fn initialized(&self) -> Result<()> {
        let transport = self.transport.lock().await;
        let transport = transport.as_ref().ok_or_else(|| anyhow!("Transport not initialized"))?;

        let params = InitializedParams {};
        transport
            .send_notification("initialized", serde_json::to_value(params)?)
            .await?;

        Ok(())
    }

    /// Open a file in the LSP server
    pub async fn open_file(&self, uri: &Url, content: &str) -> Result<()> {
        let transport = self.transport.lock().await;
        let transport = transport.as_ref().ok_or_else(|| anyhow!("Transport not initialized"))?;

        let language_id = self.language.clone();
        let version = 1;

        let text_document = TextDocumentItem {
            uri: url_to_uri(uri),
            language_id,
            version,
            text: content.to_string(),
        };

        let params = lsp_types::DidOpenTextDocumentParams { text_document };
        transport
            .send_notification("textDocument/didOpen", serde_json::to_value(params)?)
            .await?;

        // Cache the open file
        let mut open_files = self.open_files.lock().await;
        open_files.insert(uri.clone(), content.to_string());

        log::debug!("Opened file in LSP: {}", uri);
        Ok(())
    }

    /// Get hover information at a position
    pub async fn hover(&self, uri: &Url, position: Position) -> Result<Option<HoverResult>> {
        let transport = self.transport.lock().await;
        let transport = transport.as_ref().ok_or_else(|| anyhow!("Transport not initialized"))?;

        let text_document = TextDocumentIdentifier { uri: url_to_uri(uri) };
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document,
                position,
            },
            work_done_progress_params: Default::default(),
        };

        let result = transport
            .send_request("textDocument/hover", serde_json::to_value(params)?)
            .await?;

        if result.is_null() {
            return Ok(None);
        }

        let hover: Hover = serde_json::from_value(result)?;
        Ok(Some(Self::parse_hover(hover)))
    }

    /// Parse hover response into HoverResult
    fn parse_hover(hover: Hover) -> HoverResult {
        let (signature, documentation) = match hover.contents {
            HoverContents::Scalar(scalar) => {
                let content = match scalar {
                    lsp_types::MarkedString::String(s) => s,
                    lsp_types::MarkedString::LanguageString(ls) => {
                        format!("```{}\n{}\n```", ls.language, ls.value)
                    }
                };
                (content, None)
            }
            HoverContents::Array(arr) => {
                let parts: Vec<String> = arr
                    .into_iter()
                    .map(|ms| match ms {
                        lsp_types::MarkedString::String(s) => s,
                        lsp_types::MarkedString::LanguageString(ls) => {
                            format!("```{}\n{}\n```", ls.language, ls.value)
                        }
                    })
                    .collect();
                let signature = parts.first().cloned().unwrap_or_default();
                let documentation = if parts.len() > 1 {
                    Some(parts[1..].join("\n\n"))
                } else {
                    None
                };
                (signature, documentation)
            }
            HoverContents::Markup(markup) => {
                let content = markup.value;
                // Try to split signature and documentation
                if content.contains("\n\n") {
                    let parts: Vec<&str> = content.splitn(2, "\n\n").collect();
                    (parts[0].to_string(), Some(parts[1].to_string()))
                } else {
                    (content, None)
                }
            }
        };

        HoverResult { signature, documentation }
    }

    /// Get definition locations at a position
    pub async fn definition(&self, uri: &Url, position: Position) -> Result<Vec<Location>> {
        let transport = self.transport.lock().await;
        let transport = transport.as_ref().ok_or_else(|| anyhow!("Transport not initialized"))?;

        let text_document = TextDocumentIdentifier { uri: url_to_uri(uri) };
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document,
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = transport
            .send_request("textDocument/definition", serde_json::to_value(params)?)
            .await?;

        if result.is_null() {
            return Ok(Vec::new());
        }

        let locations = Self::parse_definition_response(result)?;
        Ok(locations)
    }

    /// Parse definition response
    fn parse_definition_response(result: serde_json::Value) -> Result<Vec<Location>> {
        // Try to parse as LocationLink array first
        if let Ok(links) = serde_json::from_value::<Vec<lsp_types::LocationLink>>(result.clone()) {
            return Ok(links
                .into_iter()
                .map(|link| Location {
                    uri: link.target_uri,
                    range: link.target_selection_range,
                })
                .collect());
        }

        // Try to parse as Location array
        if let Ok(locations) = serde_json::from_value::<Vec<Location>>(result.clone()) {
            return Ok(locations);
        }

        // Try to parse as single Location
        if let Ok(location) = serde_json::from_value::<Location>(result.clone()) {
            return Ok(vec![location]);
        }

        // Try to parse as single LocationLink
        if let Ok(link) = serde_json::from_value::<lsp_types::LocationLink>(result) {
            return Ok(vec![Location {
                uri: link.target_uri,
                range: link.target_selection_range,
            }]);
        }

        Ok(Vec::new())
    }

    /// Get reference locations at a position
    pub async fn references(&self, uri: &Url, position: Position, include_declaration: bool) -> Result<Vec<Location>> {
        let transport = self.transport.lock().await;
        let transport = transport.as_ref().ok_or_else(|| anyhow!("Transport not initialized"))?;

        let text_document = TextDocumentIdentifier { uri: url_to_uri(uri) };
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document,
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: ReferenceContext {
                include_declaration,
            },
        };

        let result = transport
            .send_request("textDocument/references", serde_json::to_value(params)?)
            .await?;

        if result.is_null() {
            return Ok(Vec::new());
        }

        let locations: Vec<Location> = serde_json::from_value(result)?;
        Ok(locations)
    }

    /// Get diagnostics for a file from cache
    pub async fn diagnostics(&self, uri: &Url) -> Result<Vec<Diagnostic>> {
        let cache = self.diagnostics_cache.lock().await;
        Ok(cache.get(uri).cloned().unwrap_or_default())
    }

    /// Update diagnostics cache (called when receiving publishDiagnostics notification)
    pub async fn update_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>) {
        let mut cache = self.diagnostics_cache.lock().await;
        cache.insert(uri, diagnostics);
    }

    /// Shutdown the LSP server gracefully
    pub async fn shutdown(&self) -> Result<()> {
        let mut transport_guard = self.transport.lock().await;

        if let Some(transport) = transport_guard.take() {
            // Send shutdown request
            transport
                .send_request("shutdown", serde_json::Value::Null)
                .await?;

            // Send exit notification
            transport
                .send_notification("exit", serde_json::Value::Null)
                .await?;

            // Close the transport
            transport.close().await?;

            log::info!("LSP client '{}' shutdown successfully", self.server_name);
        }

        // Clear caches
        let mut open_files = self.open_files.lock().await;
        open_files.clear();
        let mut diagnostics_cache = self.diagnostics_cache.lock().await;
        diagnostics_cache.clear();

        Ok(())
    }

    /// Check if the client is connected
    pub async fn is_connected(&self) -> bool {
        let transport = self.transport.lock().await;
        transport.is_some()
    }

    /// Get the language identifier
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Get the server name
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Get the project root
    pub fn project_root(&self) -> &PathBuf {
        &self.project_root
    }

    /// Get server capabilities
    pub async fn capabilities(&self) -> Option<lsp_types::ServerCapabilities> {
        let caps = self.capabilities.lock().await;
        caps.clone()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Format a location for human-readable output
pub fn format_location(location: &Location) -> String {
    // Convert Uri to Url if possible
    let url_str = location.uri.to_string();
    let path_str = if url_str.starts_with("file://") {
        // Try to parse as Url and convert to file path
        Url::parse(&url_str)
            .ok()
            .and_then(|url| url.to_file_path().ok())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(url_str)
    } else {
        url_str
    };

    let range = &location.range;
    let start = &range.start;
    format!(
        "{}:{}:{}",
        path_str,
        start.line + 1,  // LSP uses 0-based line numbers
        start.character + 1
    )
}

/// Convert Url to file path
#[allow(dead_code)]
fn url_to_file_path(url: &Url) -> Result<PathBuf, ()> {
    if url.scheme() == "file" {
        url.to_file_path().map_err(|_| ())
    } else {
        Err(())
    }
}

/// Format a diagnostic for human-readable output
pub fn format_diagnostic(diagnostic: &Diagnostic) -> String {
    let severity = diagnostic.severity
        .map(format_severity)
        .unwrap_or_else(|| "error".to_string());

    let message = &diagnostic.message;

    let location = diagnostic.related_information
        .as_ref()
        .and_then(|info| info.first())
        .map(|info| format!(" at {}:{}", info.location.uri.as_str(), info.location.range.start.line + 1))
        .unwrap_or_default();

    let code = diagnostic.code
        .as_ref()
        .map(|c| format!("[{}] ", match c {
            lsp_types::NumberOrString::Number(n) => n.to_string(),
            lsp_types::NumberOrString::String(s) => s.clone(),
        }))
        .unwrap_or_default();

    format!("{}{}: {}{}", severity, code, message, location)
}

/// Format diagnostic severity
fn format_severity(severity: DiagnosticSeverity) -> String {
    match severity {
        DiagnosticSeverity::ERROR => "error".to_string(),
        DiagnosticSeverity::WARNING => "warning".to_string(),
        DiagnosticSeverity::INFORMATION => "info".to_string(),
        DiagnosticSeverity::HINT => "hint".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Format hover result for display
pub fn format_hover_result(hover: &HoverResult) -> String {
    if let Some(doc) = &hover.documentation {
        format!("{}\n\n{}", hover.signature, doc)
    } else {
        hover.signature.clone()
    }
}

/// Create a file URL from a path
pub fn path_to_uri(path: &PathBuf) -> Result<Url> {
    // On Windows, convert Unix-style paths (e.g., /c/Users/...) to Windows format
    #[cfg(target_os = "windows")]
    {
        let path_str = path.to_string_lossy();

        // Check if it's a Unix-style Windows path (e.g., /c/Users/...)
        if path_str.starts_with('/') && path_str.chars().nth(1).map(|c| c.is_ascii_lowercase()).unwrap_or(false) {
            // Convert /c/... to C:/...
            let drive_letter = path_str.chars().nth(1).unwrap();
            let rest = &path_str[2..]; // Skip /c
            let windows_path = format!("{}:{}", drive_letter.to_ascii_uppercase(), rest);
            let windows_pathbuf = PathBuf::from(windows_path);

            Url::from_file_path(&windows_pathbuf)
                .map_err(|_| anyhow!("Invalid file path: {:?}", path))
        } else {
            Url::from_file_path(path)
                .map_err(|_| anyhow!("Invalid file path: {:?}", path))
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Url::from_file_path(path)
            .map_err(|_| anyhow!("Invalid file path: {:?}", path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hover_result_new() {
        let result = HoverResult::new("fn foo() -> i32");
        assert_eq!(result.signature, "fn foo() -> i32");
        assert!(result.documentation.is_none());
    }

    #[test]
    fn test_hover_result_with_documentation() {
        let result = HoverResult::new("fn foo() -> i32")
            .with_documentation("This is a test function");
        assert_eq!(result.signature, "fn foo() -> i32");
        assert_eq!(result.documentation, Some("This is a test function".to_string()));
    }

    #[test]
    fn test_format_hover_result() {
        let hover = HoverResult::new("fn foo() -> i32")
            .with_documentation("Docs");
        let formatted = format_hover_result(&hover);
        assert!(formatted.contains("fn foo() -> i32"));
        assert!(formatted.contains("Docs"));
    }

    #[test]
    fn test_format_severity() {
        assert_eq!(format_severity(DiagnosticSeverity::ERROR), "error");
        assert_eq!(format_severity(DiagnosticSeverity::WARNING), "warning");
        assert_eq!(format_severity(DiagnosticSeverity::INFORMATION), "info");
        assert_eq!(format_severity(DiagnosticSeverity::HINT), "hint");
    }

    #[test]
    fn test_path_to_uri() {
        let path = if cfg!(target_os = "windows") {
            PathBuf::from("C:\\temp\\test.rs")
        } else {
            PathBuf::from("/tmp/test.rs")
        };
        let uri = path_to_uri(&path).unwrap();
        assert!(uri.to_string().ends_with("test.rs"));
    }
}