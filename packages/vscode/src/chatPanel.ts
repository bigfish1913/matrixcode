/**
 * Chat Panel Provider - Editor Tab Style (like Claude Code)
 * 
 * Provides the chat interface in a dedicated editor tab,
 * instead of a sidebar webview.
 */

import * as vscode from 'vscode';
import { MatrixCodeClient, StreamEvent, RequestContext } from './matrixcodeClient';
import { SessionManager, Session } from './sessionManager';
import { ConfigManager } from './configManager';

interface ChatMessage {
    id: string;
    role: 'user' | 'assistant' | 'system';
    content: string;
    timestamp: Date;
    thinking?: string[];
    toolUses?: ToolUse[];
    isStreaming?: boolean;
    codeBlocks?: CodeBlock[];
}

interface ToolUse {
    id: string;
    name: string;
    input: unknown;
    result?: string;
    status: 'running' | 'done' | 'error';
}

interface CodeBlock {
    language: string;
    code: string;
    filename?: string;
    diff?: DiffInfo;
}

interface DiffInfo {
    original: string;
    suggested: string;
}

/**
 * ChatPanelProvider manages the webview panel that displays the chat interface.
 * Unlike sidebar webviews, this opens in a dedicated editor tab.
 */
export class ChatPanelProvider implements vscode.Disposable {
    private panel: vscode.WebviewPanel | undefined;
    private client: MatrixCodeClient;
    private configManager: ConfigManager;
    private messages: ChatMessage[] = [];
    private currentAssistantMessage: ChatMessage | null = null;
    private extensionUri: vscode.Uri;
    private disposables: vscode.Disposable[] = [];
    private outputChannel: vscode.OutputChannel;
    private sessionManager: SessionManager;
    private currentSession: Session | null = null;
    
    constructor(
        extensionUri: vscode.Uri,
        client: MatrixCodeClient,
        configManager: ConfigManager,
        outputChannel: vscode.OutputChannel
    ) {
        this.extensionUri = extensionUri;
        this.client = client;
        this.configManager = configManager;
        this.outputChannel = outputChannel;
        this.sessionManager = sessionManager;
        this.currentSession = sessionManager.createSession();
        
        // Listen to client events
        this.client.onEvent(this.handleStreamEvent.bind(this));
        this.client.onError(this.handleError.bind(this));
        
        this.outputChannel.appendLine('ChatPanelProvider: Event listeners registered');
    }
    
    /**
     * Open an existing panel or create a new one.
     */
    openOrCreate(): void {
        if (this.panel) {
            // If panel exists, just reveal it
            this.panel.reveal(vscode.ViewColumn.One);
            return;
        }
        
        // Create a new webview panel in an editor tab
        this.panel = vscode.window.createWebviewPanel(
            'matrixcodeChat',  // Unique identifier
            'MatrixCode Chat', // Title displayed in the tab
            vscode.ViewColumn.One,  // Editor column to show in
            {
                enableScripts: true,
                retainContextWhenHidden: true,  // Keep state when hidden
                localResourceRoots: [this.extensionUri]
            }
        );
        
        // Set the HTML content
        this.panel.webview.html = this.getHtmlContent(this.panel.webview);
        
        // Handle messages from the webview
        this.panel.webview.onDidReceiveMessage(
            async (data) => {
                await this.handleWebviewMessage(data);
            },
            null,
            this.disposables
        );
        
        // Handle panel close
        this.panel.onDidDispose(
            () => {
                this.panel = undefined;
            },
            null,
            this.disposables
        );
    }
    
    /**
     * Handle messages from the webview.
     */
    private async handleWebviewMessage(data: any): Promise<void> {
        switch (data.type) {
            case 'sendMessage':
                await this.handleUserMessage(data.content);
                break;
            case 'clearHistory':
                this.clearHistory();
                break;
            case 'newSession':
                this.clearHistory();
                break;
            case 'getHistory':
                this.postMessage({ type: 'history', messages: this.messages });
                break;
            case 'applyCode':
                await this.applyCode(data.code, data.filename);
                break;
            case 'copyCode':
                vscode.env.clipboard.writeText(data.code);
                this.postMessage({ type: 'copied', success: true });
                break;
        }
    }
    
    /**
     * Apply code to an editor.
     */
    private async applyCode(code: string, filename?: string): Promise<void> {
        try {
            if (filename) {
                // Open the file first
                const doc = await vscode.workspace.openTextDocument(filename);
                const editor = await vscode.window.showTextDocument(doc);
                
                await editor.edit(editBuilder => {
                    const fullRange = new vscode.Range(
                        doc.positionAt(0),
                        doc.positionAt(doc.getText().length)
                    );
                    editBuilder.replace(fullRange, code);
                });
            } else {
                // Apply to active editor
                const editor = vscode.window.activeTextEditor;
                if (editor) {
                    await editor.edit(editBuilder => {
                        const selection = editor.selection;
                        if (selection.isEmpty) {
                            editBuilder.insert(selection.start, code);
                        } else {
                            editBuilder.replace(selection, code);
                        }
                    });
                }
            }
            
            this.postMessage({ type: 'applied', success: true });
            vscode.window.showInformationMessage('Code applied successfully');
        } catch (error) {
            const errMsg = error instanceof Error ? error.message : String(error);
            vscode.window.showErrorMessage(`Failed to apply code: ${errMsg}`);
            this.postMessage({ type: 'applied', success: false, error: errMsg });
        }
    }
    
    /**
     * Handle user message submission.
     */
    private async handleUserMessage(content: string): Promise<void> {
        const userMessage: ChatMessage = {
            id: this.generateId(),
            role: 'user',
            content,
            timestamp: new Date()
        };
        
        this.messages.push(userMessage);
        this.postMessage({ type: 'newMessage', message: userMessage });
        
        // Build context if enabled
        const context: RequestContext | undefined = this.configManager.getAutoContext()
            ? this.buildContext()
            : undefined;
        
        // Create assistant message placeholder
        this.currentAssistantMessage = {
            id: this.generateId(),
            role: 'assistant',
            content: '',
            timestamp: new Date(),
            thinking: [],
            toolUses: [],
            isStreaming: true
        };
        this.messages.push(this.currentAssistantMessage);
        this.postMessage({ type: 'newMessage', message: this.currentAssistantMessage });
        
        // Ensure daemon is running before sending request
        if (!this.client.isRunning()) {
            try {
                await this.client.startDaemon();
            } catch (err) {
                this.handleError(err instanceof Error ? err : new Error(String(err)));
                return;
            }
        }
        
        // Send request to CLI
        try {
            await this.client.chat(content, context);
        } catch (err) {
            this.handleError(err instanceof Error ? err : new Error(String(err)));
        }
    }
    
    /**
     * Handle stream events from the CLI.
     */
    private handleStreamEvent(event: StreamEvent): void {
        console.log('[ChatPanel] Received event:', JSON.stringify(event));
        this.outputChannel.appendLine(`ChatPanel: Received event type=${event.type}, content=${event.content?.substring(0, 50)}`);
        
        if (!this.currentAssistantMessage) {
            this.currentAssistantMessage = {
                id: this.generateId(),
                role: 'assistant',
                content: '',
                timestamp: new Date(),
                thinking: [],
                toolUses: [],
                isStreaming: true
            };
            this.messages.push(this.currentAssistantMessage);
            this.postMessage({ type: 'newMessage', message: this.currentAssistantMessage });
        }
        
        switch (event.type) {
            case 'text':
                this.currentAssistantMessage.content += event.content || '';
                this.postMessage({
                    type: 'updateMessage',
                    messageId: this.currentAssistantMessage.id,
                    field: 'content',
                    value: this.currentAssistantMessage.content
                });
                break;
                
            case 'thinking':
                if (this.configManager.getShowThinking()) {
                    this.currentAssistantMessage.thinking?.push(event.content || '');
                    this.postMessage({
                        type: 'updateMessage',
                        messageId: this.currentAssistantMessage.id,
                        field: 'thinking',
                        value: this.currentAssistantMessage.thinking
                    });
                }
                break;
                
            case 'tool_use':
                const toolUse: ToolUse = {
                    id: event.id || '',
                    name: event.name || '',
                    input: event.input,
                    status: 'running'
                };
                this.currentAssistantMessage.toolUses?.push(toolUse);
                this.postMessage({
                    type: 'updateMessage',
                    messageId: this.currentAssistantMessage.id,
                    field: 'toolUses',
                    value: this.currentAssistantMessage.toolUses
                });
                break;
                
            case 'tool_result':
                const lastToolUse = this.currentAssistantMessage.toolUses?.[
                    this.currentAssistantMessage.toolUses?.length - 1 || 0
                ];
                if (lastToolUse) {
                    lastToolUse.result = event.content;
                    lastToolUse.status = 'done';
                    this.postMessage({
                        type: 'updateMessage',
                        messageId: this.currentAssistantMessage.id,
                        field: 'toolUses',
                        value: this.currentAssistantMessage.toolUses
                    });
                }
                break;
                
            case 'done':
                if (this.currentAssistantMessage) {
                    this.currentAssistantMessage.isStreaming = false;
                    this.postMessage({
                        type: 'updateMessage',
                        messageId: this.currentAssistantMessage.id,
                        field: 'isStreaming',
                        value: false
                    });
                    this.currentAssistantMessage = null;
                }
                break;
                
            case 'error':
                this.postMessage({
                    type: 'error',
                    message: event.message || event.content || 'An error occurred'
                });
                break;
                
            case 'session_started':
                console.log('[ChatPanel] Session started:', event.content);
                break;
        }
    }
    
    /**
     * Handle errors from the CLI.
     */
    private handleError(error: Error): void {
        this.postMessage({
            type: 'error',
            message: error.message
        });
        
        if (this.currentAssistantMessage) {
            this.currentAssistantMessage.isStreaming = false;
            this.postMessage({
                type: 'updateMessage',
                messageId: this.currentAssistantMessage.id,
                field: 'isStreaming',
                value: false
            });
            this.currentAssistantMessage = null;
        }
    }
    
    /**
     * Send a quick action request.
     */
    async sendQuickAction(
        action: string,
        code: string,
        context: EditorContext,
        instructions?: string
    ): Promise<void> {
        // Build a pre-formatted message for the quick action
        const actionPrompts: Record<string, string> = {
            'explain': `请解释这段代码的功能和逻辑：\n\n${code}`,
            'fix': `请修复这段代码中的问题：\n\n${code}`,
            'generateTests': `请为这段代码生成单元测试：\n\n${code}`,
            'refactor': instructions 
                ? `请重构这段代码，要求：${instructions}\n\n${code}`
                : `请重构这段代码，改进其结构和可读性：\n\n${code}`
        };
        
        const prompt = actionPrompts[action] || `${action}: ${code}`;
        
        // Send as regular chat message
        await this.handleUserMessage(prompt);
    }
    
    /**
     * Build request context from current editor.
     */
    private buildContext(): RequestContext {
        const editor = vscode.window.activeTextEditor;
        if (!editor) return {};
        
        return {
            workspace: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
            file: editor.document.uri.fsPath,
            language: editor.document.languageId,
            selection: editor.selection.isEmpty ? undefined : {
                start: { line: editor.selection.start.line, character: editor.selection.start.character },
                end: { line: editor.selection.end.line, character: editor.selection.end.character }
            }
        };
    }
    
    /**
     * Clear chat history.
     */
    clearHistory(): void {
        this.messages = [];
        this.postMessage({ type: 'clearHistory' });
    }
    
    /**
     * Post message to the webview.
     */
    private postMessage(message: any): void {
        if (this.panel) {
            this.panel.webview.postMessage(message);
        }
    }
    
    /**
     * Generate unique ID for messages.
     */

    /**
     * Save current session.
     */
    private async saveCurrentSession(): Promise<void> {
        if (!this.currentSession) {
            this.currentSession = this.sessionManager.createSession(this.messages);
        }
        this.currentSession.messages = this.messages;
        this.currentSession.updatedAt = Date.now();
        const name = await this.sessionManager.promptSessionName(this.currentSession.name);
        if (name) {
            this.currentSession.name = name;
        }
        await this.sessionManager.saveSession(this.currentSession);
        vscode.window.showInformationMessage('Session saved: ' + this.currentSession.name);
    }
    
    private async loadSession(): Promise<void> {
        const session = await this.sessionManager.showSessionPicker();
        if (session) {
            this.currentSession = session;
            this.messages = session.messages;
            this.postMessage({ type: 'clearMessages' });
            for (const msg of this.messages) {
                this.postMessage({ type: 'newMessage', message: msg });
            }
            vscode.window.showInformationMessage('Session loaded: ' + session.name);
        }
    }
    
    private async newSession(): Promise<void> {
        if (this.messages.length > 0) {
            const save = await vscode.window.showQuickPick(['Save and start new', 'Discard and start new', 'Cancel'], { placeHolder: 'Current session has messages' });
            if (save === 'Cancel') return;
            if (save === 'Save and start new') await this.saveCurrentSession();
        }
        this.currentSession = this.sessionManager.createSession();
        this.messages = [];
        this.postMessage({ type: 'clearMessages' });
        vscode.window.showInformationMessage('New session started');
    }
    
    private async deleteSession(): Promise<void> {
        const session = await this.sessionManager.showSessionPicker();
        if (session) {
            const confirm = await vscode.window.showQuickPick(['Delete', 'Cancel'], { placeHolder: 'Delete session?' });
            if (confirm === 'Delete') {
                await this.sessionManager.deleteSession(session.id);
                vscode.window.showInformationMessage('Session deleted');
            }
        }
    }
    
    private generateId(): string {
        return Math.random().toString(36).substring(2, 9);
    }
    
    /**
     * Dispose the provider.
     */
    dispose(): void {
        if (this.panel) {
            this.panel.dispose();
            this.panel = undefined;
        }
        
        for (const disposable of this.disposables) {
            disposable.dispose();
        }
        this.disposables = [];
    }
    
    /**
     * Generate HTML content for the webview.
     */
    private getHtmlContent(webview: vscode.Webview): string {
        return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta name="color-scheme" content="light dark">
    <title>MatrixCode Chat</title>
    <style>
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        
        :root {
            --font-family: var(--vscode-font-family);
            --font-size: var(--vscode-font-size);
            --background: var(--vscode-editor-background);
            --foreground: var(--vscode-editor-foreground);
            --input-bg: var(--vscode-input-background);
            --input-border: var(--vscode-input-border);
            --input-fg: var(--vscode-input-foreground);
            --button-bg: var(--vscode-button-background);
            --button-fg: var(--vscode-button-foreground);
            --button-hover: var(--vscode-button-hoverBackground);
            --link-fg: var(--vscode-textLink-foreground);
            --border: var(--vscode-panel-border);
            --scrollbar-bg: var(--vscode-editor-scrollbarSlider-background);
            --scrollbar-hover: var(--vscode-editor-scrollbarSlider-hoverBackground);
            --success-bg: #28a745;
            --error-bg: #dc3545;
            --warning-bg: #ffc107;
        }
        
        body {
            font-family: var(--font-family);
            font-size: var(--font-size);
            background: var(--background);
            color: var(--foreground);
            height: 100vh;
            display: flex;
            flex-direction: column;
            overflow: hidden;
        }
        
        /* Header */
        .header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 12px 16px;
            border-bottom: 1px solid var(--border);
            background: var(--background);
        }
        
        .header-title {
            font-size: 16px;
            font-weight: 600;
            display: flex;
            align-items: center;
            gap: 10px;
        }
        
        .header-logo {
            font-size: 20px;
        }
        
        .header-actions {
            display: flex;
            gap: 12px;
        }
        
        .header-btn {
            background: transparent;
            border: 1px solid var(--input-border);
            color: var(--foreground);
            cursor: pointer;
            padding: 6px 12px;
            border-radius: 4px;
            font-size: 13px;
            display: flex;
            align-items: center;
            gap: 6px;
        }
        
        .header-btn:hover {
            background: var(--button-hover);
        }
        
        /* Messages Container */
        .messages-container {
            flex: 1;
            overflow-y: auto;
            padding: 20px;
            display: flex;
            flex-direction: column;
            gap: 20px;
        }
        
        /* Message */
        .message {
            display: flex;
            flex-direction: column;
            gap: 10px;
            max-width: 100%;
        }
        
        .message-header {
            display: flex;
            align-items: center;
            gap: 10px;
            font-size: 13px;
            opacity: 0.8;
        }
        
        .message-avatar {
            width: 28px;
            height: 28px;
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 16px;
            font-weight: 600;
        }
        
        .message-avatar.user {
            background: #0078d4;
            color: white;
        }
        
        .message-avatar.assistant {
            background: #f0f0f0;
            border: 1px solid var(--border);
        }
        
        .message-role {
            font-weight: 600;
        }
        
        .message-time {
            opacity: 0.6;
        }
        
        .message-content {
            padding: 16px;
            border-radius: 8px;
            line-height: 1.7;
            white-space: pre-wrap;
            word-wrap: break-word;
            font-size: 14px;
        }
        
        .message.user .message-content {
            background: rgba(0, 120, 212, 0.1);
            border: 1px solid rgba(0, 120, 212, 0.2);
        }
        
        .message.assistant .message-content {
            background: rgba(255, 255, 255, 0.05);
            border: 1px solid var(--border);
        }
        
        /* Thinking Block */
        .thinking-block {
            padding: 8px 12px;
            background: rgba(100, 100, 100, 0.1);
            border: 1px solid rgba(100, 100, 100, 0.2);
            border-radius: 6px;
            font-size: 12px;
            opacity: 0.7;
            margin-top: 8px;
            cursor: pointer;
            user-select: none;
        }
        
        .thinking-block.collapsed .thinking-content {
            display: none;
        }
        
        .thinking-block.collapsed .thinking-toggle {
            content: '▶';
        }
        
        .thinking-content {
            margin-top: 6px;
            line-height: 1.4;
            color: var(--foreground);
            opacity: 0.6;
        }
        
        .thinking-header {
            display: flex;
            align-items: center;
            gap: 6px;
            font-weight: 500;
            color: var(--foreground);
        }
        
        .thinking-toggle {
            font-size: 10px;
            opacity: 0.5;
        }
        
        /* Tool Use Card */
        .tool-use-card {
            background: rgba(255, 255, 255, 0.03);
            border: 1px solid var(--border);
            border-radius: 8px;
            margin-top: 12px;
            overflow: hidden;
        }
        
        .tool-use-header {
            display: flex;
            align-items: center;
            gap: 10px;
            padding: 10px 14px;
            background: rgba(255, 255, 255, 0.05);
            font-size: 13px;
        }
        
        .tool-use-icon {
            font-size: 16px;
        }
        
        .tool-use-name {
            font-weight: 600;
        }
        
        .tool-use-status {
            font-size: 11px;
            padding: 3px 8px;
            border-radius: 4px;
            font-weight: 600;
        }
        
        .tool-use-status.running {
            background: rgba(255, 193, 7, 0.2);
            color: #856404;
        }
        
        .tool-use-status.done {
            background: rgba(40, 167, 69, 0.2);
            color: #155724;
        }
        
        .tool-use-status.error {
            background: rgba(220, 53, 69, 0.2);
            color: #721c24;
        }
        
        .tool-use-body {
            padding: 12px 14px;
            font-size: 12px;
            font-family: monospace;
        }
        
        .tool-use-input {
            opacity: 0.9;
            max-height: 120px;
            overflow-y: auto;
        }
        
        .tool-use-result {
            margin-top: 10px;
            padding-top: 10px;
            border-top: 1px solid var(--border);
            max-height: 200px;
            overflow-y: auto;
        }
        
        /* Code Block */
        .code-block {
            background: var(--vscode-textCodeBlock-background);
            border-radius: 8px;
            margin-top: 12px;
            border: 1px solid var(--border);
        }
        
        .code-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 8px 14px;
            background: rgba(255, 255, 255, 0.05);
            font-size: 12px;
        }
        
        .code-language {
            font-weight: 600;
            opacity: 0.8;
        }
        
        .code-actions {
            display: flex;
            gap: 8px;
        }
        
        .code-btn {
            background: transparent;
            border: 1px solid var(--input-border);
            color: var(--foreground);
            cursor: pointer;
            padding: 4px 10px;
            border-radius: 4px;
            font-size: 12px;
        }
        
        .code-btn:hover {
            background: var(--button-hover);
        }
        
        .code-content {
            padding: 14px;
            font-family: var(--vscode-editor-font-family);
            font-size: var(--vscode-editor-font-size);
            line-height: 1.6;
            overflow-x: auto;
            white-space: pre;
        }
        
        /* Input Area */
        .input-area {
            padding: 16px;
            border-top: 1px solid var(--border);
            background: var(--background);
        }
        
        .input-wrapper {
            display: flex;
            gap: 12px;
            align-items: flex-end;
        }
        
        .input-field {
            flex: 1;
            padding: 12px;
            border: 1px solid var(--input-border);
            border-radius: 8px;
            background: var(--input-bg);
            color: var(--input-fg);
            font-size: 14px;
            font-family: var(--font-family);
            resize: none;
            min-height: 60px;
            max-height: 200px;
            outline: none;
        }
        
        .input-field:focus {
            border-color: var(--link-fg);
        }
        
        .input-btn {
            background: var(--button-bg);
            border: none;
            color: var(--button-fg);
            cursor: pointer;
            padding: 12px 20px;
            border-radius: 8px;
            font-size: 14px;
            font-weight: 600;
            height: 44px;
            min-width: 80px;
        }
        
        .input-btn:hover {
            background: var(--button-hover);
        }
        
        .input-btn:disabled {
            opacity: 0.5;
            cursor: not-allowed;
        }
        
        /* Spinner */
        .spinner {
            display: inline-block;
            width: 14px;
            height: 14px;
            border: 2px solid var(--foreground);
            border-radius: 50%;
            border-top-color: transparent;
            animation: spin 1s linear infinite;
        }
        
        @keyframes spin {
            to { transform: rotate(360deg); }
        }
        
        /* Empty State */
        .empty-state {
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            height: 100%;
            gap: 20px;
            opacity: 0.6;
        }
        
        .empty-icon {
            font-size: 60px;
        }
        
        .empty-title {
            font-size: 20px;
            font-weight: 600;
        }
        
        .empty-hint {
            font-size: 14px;
            text-align: center;
            max-width: 400px;
        }
        
        .empty-features {
            display: flex;
            flex-direction: column;
            gap: 10px;
            margin-top: 20px;
        }
        
        .empty-feature {
            font-size: 13px;
            display: flex;
            align-items: center;
            gap: 8px;
        }
        
        /* Scrollbar */
        ::-webkit-scrollbar {
            width: 10px;
            height: 10px;
        }
        
        ::-webkit-scrollbar-track {
            background: transparent;
        }
        
        ::-webkit-scrollbar-thumb {
            background: var(--scrollbar-bg);
            border-radius: 5px;
        }
        
        ::-webkit-scrollbar-thumb:hover {
            background: var(--scrollbar-hover);
        }
        
        /* Status bar */
        .status-bar {
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 8px 16px;
            font-size: 12px;
            opacity: 0.7;
            border-top: 1px solid var(--border);
        }
        
        .status-info {
            display: flex;
            align-items: center;
            gap: 10px;
        }
        
        .status-spinner {
            display: none;
        }
        
        .status-spinner.active {
            display: inline-block;
        }
    </style>
</head>
<body>
    <div class="header">
        <div class="header-title">
            <span class="header-logo">🤖</span>
            <span>MatrixCode</span>
        </div>
        <div class="header-actions">
            <button class="header-btn" onclick="newSession()">New Session</button>
            <button class="header-btn" onclick="clearHistory()">Clear</button>
        </div>
    </div>
    
    <div class="messages-container" id="messages">
        <div class="empty-state" id="empty">
            <div class="empty-icon">🤖</div>
            <div class="empty-title">MatrixCode AI Assistant</div>
            <div class="empty-hint">Ask questions about your code, request explanations, fixes, tests, or refactorings.</div>
            <div class="empty-features">
                <div class="empty-feature">💡 Explain code functionality</div>
                <div class="empty-feature">🔧 Fix bugs and errors</div>
                <div class="empty-feature">🧪 Generate unit tests</div>
                <div class="empty-feature">✨ Refactor and improve code</div>
            </div>
        </div>
    </div>
    
    <div class="input-area">
        <div class="input-wrapper">
            <textarea 
                class="input-field" 
                id="input" 
                placeholder="Ask MatrixCode anything... (Enter to send, Shift+Enter for new line)"
                rows="3"
            ></textarea>
            <button class="input-btn" id="sendBtn" onclick="sendMessage()">
                Send
            </button>
        </div>
    </div>
    
    <div class="status-bar">
        <div class="status-info">
            <span class="status-spinner" id="spinner"></span>
            <span id="status-text">Ready</span>
        </div>
        <div class="status-info">
            <span id="model-info"></span>
        </div>
    </div>
    
    <script>
        const vscode = acquireVsCodeApi();
        const messagesDiv = document.getElementById('messages');
        const inputField = document.getElementById('input');
        const sendBtn = document.getElementById('sendBtn');
        const emptyState = document.getElementById('empty');
        const spinner = document.getElementById('spinner');
        const statusText = document.getElementById('status-text');
        const modelInfo = document.getElementById('model-info');
        
        const toolIcons = {
            read: '📖',
            write: '📝',
            edit: '✏️',
            bash: '⚡',
            search: '🔍',
            glob: '📁',
            ls: '📂',
            ask: '❓',
            websearch: '🌐',
            webfetch: '🔗',
            skill: '🔧',
            todo_write: '📋'
        };
        
        let currentMessageId = null;
        
        // Auto-resize textarea
        inputField.addEventListener('input', function() {
            this.style.height = 'auto';
            this.style.height = Math.min(this.scrollHeight, 200) + 'px';
        });
        
        // Handle keyboard
        inputField.addEventListener('keydown', function(e) {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                sendMessage();
            }
        });
        
        function sendMessage() {
            const content = inputField.value.trim();
            if (!content) return;
            
            vscode.postMessage({ type: 'sendMessage', content: content });
            inputField.value = '';
            inputField.style.height = '60px';
            sendBtn.disabled = true;
            setStatus('Thinking...', true);
        }
        
        function newSession() {
            vscode.postMessage({ type: 'newSession' });
        }
        
        function clearHistory() {
            vscode.postMessage({ type: 'clearHistory' });
        }
        
        function applyCode(code, filename) {
            vscode.postMessage({ type: 'applyCode', code: code, filename: filename });
        }
        
        function copyCode(code) {
            vscode.postMessage({ type: 'copyCode', code: code });
        }
        
        function setStatus(text, isLoading) {
            statusText.textContent = text;
            spinner.classList.toggle('active', isLoading);
        }
        
        function addMessage(message) {
            emptyState.style.display = 'none';
            
            const msgDiv = document.createElement('div');
            msgDiv.className = 'message ' + message.role;
            msgDiv.id = 'msg-' + message.id;
            
            // Header
            const headerDiv = document.createElement('div');
            headerDiv.className = 'message-header';
            
            const avatarDiv = document.createElement('div');
            avatarDiv.className = 'message-avatar ' + message.role;
            avatarDiv.textContent = message.role === 'user' ? '👤' : '🤖';
            
            const roleDiv = document.createElement('div');
            roleDiv.className = 'message-role';
            roleDiv.textContent = message.role === 'user' ? 'You' : 'MatrixCode';
            
            const timeDiv = document.createElement('div');
            timeDiv.className = 'message-time';
            timeDiv.textContent = formatTime(message.timestamp);
            
            headerDiv.appendChild(avatarDiv);
            headerDiv.appendChild(roleDiv);
            headerDiv.appendChild(timeDiv);
            
            // Content
            const contentDiv = document.createElement('div');
            contentDiv.className = 'message-content';
            contentDiv.id = 'content-' + message.id;
            contentDiv.innerHTML = formatContent(message.content);
            
            msgDiv.appendChild(headerDiv);
            msgDiv.appendChild(contentDiv);
            
            // Thinking block
            if (message.thinking && message.thinking.length > 0) {
                const thinkingDiv = document.createElement('div');
                thinkingDiv.className = 'thinking-block';
                thinkingDiv.id = 'thinking-' + message.id;
                thinkingDiv.innerHTML = '<div class="thinking-header">💭 Thinking</div>' +
                    message.thinking.map(t => escapeHtml(t)).join(' ');
                msgDiv.appendChild(thinkingDiv);
            }
            
            // Tool uses
            if (message.toolUses && message.toolUses.length > 0) {
                const toolDiv = document.createElement('div');
                toolDiv.id = 'tools-' + message.id;
                toolDiv.innerHTML = renderToolUses(message.toolUses);
                msgDiv.appendChild(toolDiv);
            }
            
            messagesDiv.appendChild(msgDiv);
            scrollToBottom();
            
            if (message.isStreaming) {
                currentMessageId = message.id;
            }
        }
        
        function updateMessage(messageId, field, value) {
            if (field === 'content') {
                const contentDiv = document.getElementById('content-' + messageId);
                if (contentDiv) {
                    contentDiv.innerHTML = formatContent(value);
                    scrollToBottom();
                }
            } else if (field === 'thinking') {
                const thinkingDiv = document.getElementById('thinking-' + messageId);
                if (thinkingDiv && value && value.length > 0) {
                    thinkingDiv.innerHTML = '<div class="thinking-header">💭 Thinking</div>' +
                        value.map(t => escapeHtml(t)).join(' ');
                } else if (!thinkingDiv && value && value.length > 0) {
                    // Create thinking div
                    const msgDiv = document.getElementById('msg-' + messageId);
                    if (msgDiv) {
                        const newThinkingDiv = document.createElement('div');
                        newThinkingDiv.className = 'thinking-block';
                        newThinkingDiv.id = 'thinking-' + messageId;
                        newThinkingDiv.innerHTML = '<div class="thinking-header">💭 Thinking</div>' +
                            value.map(t => escapeHtml(t)).join('<br>');
                        msgDiv.appendChild(newThinkingDiv);
                    }
                }
            } else if (field === 'toolUses') {
                const toolDiv = document.getElementById('tools-' + messageId);
                if (toolDiv) {
                    toolDiv.innerHTML = renderToolUses(value);
                }
            } else if (field === 'isStreaming') {
                sendBtn.disabled = value;
                if (!value) {
                    currentMessageId = null;
                    setStatus('Ready', false);
                    sendBtn.disabled = false;
                }
            }
        }
        
        function formatContent(content) {
            if (!content) return '';
            
            // Process content: escape HTML, detect code blocks
            let result = '';
            let inCodeBlock = false;
            let codeLang = '';
            let codeContent = '';
            let lines = content.split('\\n');
            
            for (let i = 0; i < lines.length; i++) {
                let line = lines[i];
                
                if (!inCodeBlock && line.startsWith('\`\`\`')) {
                    inCodeBlock = true;
                    codeLang = line.substring(3).trim() || 'text';
                    codeContent = '';
                    continue;
                }
                
                if (inCodeBlock && line.startsWith('\`\`\`')) {
                    inCodeBlock = false;
                    result += renderCodeBlock(codeLang, codeContent);
                    continue;
                }
                
                if (inCodeBlock) {
                    codeContent += line + '\\n';
                } else {
                    result += processInlineMarkdown(escapeHtml(line)) + '<br>';
                }
            }
            
            // Handle unclosed code block
            if (inCodeBlock) {
                result += renderCodeBlock(codeLang, codeContent);
            }
            
            return result;
        }
        
        function renderCodeBlock(lang, code) {
            const escapedCode = escapeHtml(code.trim());
            const displayLang = lang || 'text';
            const codeId = 'code-' + Math.random().toString(36).substr(2, 9);
            
            return '<div class="code-block">' +
                '<div class="code-header">' +
                '<span class="code-language">' + displayLang + '</span>' +
                '<div class="code-actions">' +
                '<button class="code-btn" onclick="copyCodeById(\\'' + codeId + '\\')">Copy</button>' +
                '<button class="code-btn" onclick="applyCodeById(\\'' + codeId + '\\')">Apply</button>' +
                '</div>' +
                '</div>' +
                '<div class="code-content" id="' + codeId + '">' + escapedCode + '</div>' +
                '</div>';
        }
        
        function copyCodeById(id) {
            const codeDiv = document.getElementById(id);
            if (codeDiv) {
                copyCode(codeDiv.textContent);
            }
        }
        
        function applyCodeById(id) {
            const codeDiv = document.getElementById(id);
            if (codeDiv) {
                applyCode(codeDiv.textContent);
            }
        }
        
        function escapeHtml(text) {
            return text
                .replace(/&/g, '&amp;')
                .replace(/</g, '&lt;')
                .replace(/>/g, '&gt;');
        }
        
        function renderToolUses(toolUses) {
            if (!toolUses || toolUses.length === 0) return '';
            
            let html = '';
            for (const tool of toolUses) {
                const icon = toolIcons[tool.name] || '🔧';
                const statusClass = tool.status || 'running';
                const statusText = tool.status === 'running' ? '⏳ Running' : 
                                   tool.status === 'done' ? '✓ Done' : '✗ Error';
                
                html += '<div class="tool-use-card">' +
                    '<div class="tool-use-header">' +
                    '<span class="tool-use-icon">' + icon + '</span>' +
                    '<span class="tool-use-name">' + tool.name + '</span>' +
                    '<span class="tool-use-status ' + statusClass + '">' + statusText + '</span>' +
                    '</div>' +
                    '<div class="tool-use-body">' +
                    '<div class="tool-use-input">' + escapeHtml(JSON.stringify(tool.input, null, 2)) + '</div>' +
                    (tool.result ? '<div class="tool-use-result">' + escapeHtml(truncate(tool.result, 300)) + '</div>' : '') +
                    '</div>' +
                    '</div>';
            }
            return html;
        }
        
        function truncate(str, max) {
            if (str.length > max) {
                return str.substring(0, max) + '...';
            }
            return str;
        }
        
        function formatTime(date) {
            if (typeof date === 'string') date = new Date(date);
            return date.toLocaleTimeString();
        }
        
        function scrollToBottom() {
            messagesDiv.scrollTop = messagesDiv.scrollHeight;
        }
        
        function clearMessages() {
            messagesDiv.innerHTML = '<div class="empty-state" id="empty">' +
                '<div class="empty-icon">🤖</div>' +
                '<div class="empty-title">MatrixCode AI Assistant</div>' +
                '<div class="empty-hint">Ask questions about your code, request explanations, fixes, tests, or refactorings.</div>' +
                '<div class="empty-features">' +
                '<div class="empty-feature">💡 Explain code functionality</div>' +
                '<div class="empty-feature">🔧 Fix bugs and errors</div>' +
                '<div class="empty-feature">🧪 Generate unit tests</div>' +
                '<div class="empty-feature">✨ Refactor and improve code</div>' +
                '</div>' +
                '</div>';
        }
        
        // Handle messages from extension
        window.addEventListener('message', event => {
            const message = event.data;
            
            switch (message.type) {
                case 'newMessage':
                    addMessage(message.message);
                    break;
                case 'updateMessage':
                    updateMessage(message.messageId, message.field, message.value);
                    break;
                case 'history':
                    clearMessages();
                    message.messages.forEach(addMessage);
                    break;
                case 'clearHistory':
                    clearMessages();
                    setStatus('Ready', false);
                    sendBtn.disabled = false;
                    break;
                case 'error':
                    setStatus('Error: ' + message.message, false);
                    sendBtn.disabled = false;
                    break;
                case 'copied':
                    setStatus('Code copied!', false);
                    setTimeout(() => setStatus('Ready', false), 2000);
                    break;
                case 'applied':
                    if (message.success) {
                        setStatus('Code applied!', false);
                    } else {
                        setStatus('Failed: ' + message.error, false);
                    }
                    setTimeout(() => setStatus('Ready', false), 3000);
                    break;
            }
        });
        
        // Request history on load
        vscode.postMessage({ type: 'getHistory' });
        
        // Focus input on load
        inputField.focus();
    </script>
</body>
</html>`;
    }
}

interface EditorContext {
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
        range: { start: { line: number; character: number }; end: { line: number; character: number } };
    }>;
}