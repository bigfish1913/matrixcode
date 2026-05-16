// ============================================================================
// MatrixCode Library Modules
// ============================================================================

// Core Components
pub mod agent;        // Main agent implementation (chat loop, tool execution)
pub mod providers;    // LLM API providers (Anthropic, OpenAI)
pub mod tools;        // Tool implementations (read, write, bash, etc.)
pub mod workspace;    // Working directory management
pub mod config;       // Configuration loading (matrix + cc-switch fallback)

// Session & State Management
pub mod session;      // Session persistence and management
pub mod memory;       // Auto memory accumulation (new)
pub mod cancel;       // Cancellation tokens for interrupting operations
pub mod approval;     // User approval gates for dangerous operations

// Context Management
pub mod compress;     // Context compression for long conversations
pub mod models;       // Model configuration and task planning

// Utilities
pub mod ui;           // UI display utilities (formatting, printing)
pub mod markdown;     // Terminal markdown rendering
pub mod overview;     // Project overview generation and management
pub mod prompt;       // System prompt building and profiles
pub mod skills;       // Skill loading and management
