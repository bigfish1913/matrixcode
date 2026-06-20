/**
 * Type definitions for MatrixCode VSCode Extension
 */

/**
 * Chat message structure
 */
export interface ChatMessage {
    id: string;
    role: 'user' | 'assistant' | 'system';
    content: string;
    timestamp: Date;
    thinking?: string[];
    toolUses?: ToolUse[];
    isStreaming?: boolean;
    codeBlocks?: CodeBlock[];
}

/**
 * Tool use information
 */
export interface ToolUse {
    id: string;
    name: string;
    input: unknown;
    result?: string;
    status: 'running' | 'done' | 'error';
}

/**
 * Code block in message
 */
export interface CodeBlock {
    language: string;
    code: string;
    filename?: string;
    diff?: DiffInfo;
}

/**
 * Diff information for code changes
 */
export interface DiffInfo {
    original: string;
    suggested: string;
}

/**
 * Editor context for quick actions
 */
export interface EditorContext {
    workspace?: string;
    file?: string;
    language?: string;
    selection?: {
        start: { line: number; character: number };
        end: { line: number; character: number };
    };
    diagnostics?: Array<{
        severity: string;
        message: string;
        range: { 
            start: { line: number; character: number }; 
            end: { line: number; character: number } 
        };
    }>;
}

/**
 * Stream event from CLI
 */
export interface StreamEvent {
    type: 'text' | 'thinking' | 'tool_use' | 'tool_result' | 'done' | 'error' | 'session_started';
    content?: string;
    id?: string;
    name?: string;
    input?: unknown;
    message?: string;
}

/**
 * Request context for CLI
 */
export interface RequestContext {
    workspace?: string;
    file?: string;
    language?: string;
    selection?: {
        start: { line: number; character: number };
        end: { line: number; character: number };
    };
}

/**
 * CLI configuration
 */
export interface CliConfig {
    cliPath?: string;
    provider?: string;
    model?: string;
    think?: boolean;
    markdown?: boolean;
    maxTokens?: number;
    compressModel?: string;
    daemonMode?: boolean;
}

/**
 * Webview message types
 */
export type WebviewMessage = 
    | { type: 'sendMessage'; content: string }
    | { type: 'clearHistory' }
    | { type: 'newSession' }
    | { type: 'getHistory' }
    | { type: 'applyCode'; code: string; filename?: string }
    | { type: 'copyCode'; code: string }
    | { type: 'saveSession' }
    | { type: 'loadSession' }
    | { type: 'deleteSession' };

export type ExtensionMessage =
    | { type: 'newMessage'; message: ChatMessage }
    | { type: 'updateMessage'; messageId: string; field: string; value: unknown }
    | { type: 'history'; messages: ChatMessage[] }
    | { type: 'clearHistory' }
    | { type: 'error'; message: string }
    | { type: 'copied'; success: boolean }
    | { type: 'applied'; success: boolean; error?: string }
    | { type: 'sessions'; sessions: SessionInfo[] };

/**
 * Session info for listing
 */
export interface SessionInfo {
    id: string;
    name: string;
    createdAt: number;
    updatedAt: number;
    messageCount: number;
}