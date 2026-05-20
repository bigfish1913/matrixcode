/**
 * Chat Panel Provider - Editor Tab Style (like Claude Code)
 * 
 * Provides the chat interface in a dedicated editor tab,
 * instead of a sidebar webview.
 */

import * as vscode from 'vscode';
import { MatrixCodeClient, RequestContext } from './matrixcodeClient';
import { SessionManager, Session } from './sessionManager';
import { ConfigManager } from './configManager';
import { ChatMessage, ToolUse, CodeBlock, DiffInfo, EditorContext, StreamEvent } from './types';

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
        outputChannel: vscode.OutputChannel,
        sessionManager: SessionManager
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
            case 'openFile':
                await this.openFile(data.path);
                break;
        }
    }

    /**
     * Open a file from a link click in webview.
     * Supports formats: filename.ts, filename.ts:42, filename.ts:42-51
     */
    private async openFile(linkPath: string): Promise<void> {
        try {
            // Parse the link path - extract filename and line numbers
            // Format: path/to/file.ts:42 or path/to/file.ts:42-51
            const match = linkPath.match(/^(.+?)(?::(\d+)(?:-(\d+))?)?$/);

            if (!match) {
                vscode.window.showWarningMessage(`Invalid file path: ${linkPath}`);
                return;
            }

            const filePath = match[1];
            const startLine = match[2] ? parseInt(match[2], 10) - 1 : 0; // Convert to 0-indexed
            const endLine = match[3] ? parseInt(match[3], 10) - 1 : startLine;

            // Resolve file path relative to workspace
            const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
            let absolutePath = filePath;

            if (workspaceRoot && !filePath.startsWith(workspaceRoot)) {
                absolutePath = vscode.Uri.joinPath(vscode.Uri.file(workspaceRoot), filePath).fsPath;
            }

            // Open the file
            const doc = await vscode.workspace.openTextDocument(absolutePath);
            const editor = await vscode.window.showTextDocument(doc, {
                preview: false,
                selection: new vscode.Range(
                    new vscode.Position(startLine, 0),
                    new vscode.Position(endLine, 0)
                )
            });
        } catch (error) {
            const errMsg = error instanceof Error ? error.message : String(error);
            vscode.window.showErrorMessage(`Failed to open file: ${errMsg}`);
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
     * Generate HTML content for the webview by loading external files.
     */
    private getHtmlContent(webview: vscode.Webview): string {
        // Get URIs for webview resources
        const webviewPath = vscode.Uri.joinPath(this.extensionUri, 'src', 'webview');
        const toolkitPath = vscode.Uri.joinPath(this.extensionUri, 'node_modules', '@vscode', 'webview-ui-toolkit', 'dist');
        const markedPath = vscode.Uri.joinPath(this.extensionUri, 'node_modules', 'marked', 'lib');
        const hljsPath = vscode.Uri.joinPath(this.extensionUri, 'node_modules', 'highlight.js');
        const cssUri = webview.asWebviewUri(vscode.Uri.joinPath(webviewPath, 'styles.css'));
        const jsUri = webview.asWebviewUri(vscode.Uri.joinPath(webviewPath, 'main.js'));
        const toolkitUri = webview.asWebviewUri(vscode.Uri.joinPath(toolkitPath, 'toolkit.min.js'));
        const markedUri = webview.asWebviewUri(vscode.Uri.joinPath(markedPath, 'marked.umd.js'));
        const hljsCssUri = webview.asWebviewUri(vscode.Uri.joinPath(hljsPath, 'styles', 'vs2015.min.css'));
        
        return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta name="color-scheme" content="light dark">
    <title>MatrixCode Chat</title>
    <link rel="stylesheet" href="${cssUri}">
    <link rel="stylesheet" href="${hljsCssUri}">
    <script src="${toolkitUri}"></script>
    <script src="${markedUri}"></script>
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
    
    <script src="${jsUri}"></script>
</body>
</html>`;
    }
}
