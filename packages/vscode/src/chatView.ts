/**
 * Chat View Provider
 * Provides the sidebar webview for chat interface
 */

import * as vscode from 'vscode';
import { MatrixCodeClient, StreamEvent, RequestContext } from './matrixcodeClient';
import { ConfigManager } from './configManager';

interface ChatMessage {
    id: string;
    role: 'user' | 'assistant' | 'system';
    content: string;
    timestamp: Date;
    thinking?: string[];
    toolUses?: ToolUse[];
    isStreaming?: boolean;
}

interface ToolUse {
    id: string;
    name: string;
    input: unknown;
    result?: string;
}

export class ChatViewProvider implements vscode.WebviewViewProvider {
    private view?: vscode.WebviewView;
    private client: MatrixCodeClient;
    private configManager: ConfigManager;
    private messages: ChatMessage[] = [];
    private currentAssistantMessage: ChatMessage | null = null;
    private extensionUri: vscode.Uri;
    
    constructor(
        extensionUri: vscode.Uri,
        client: MatrixCodeClient,
        configManager: ConfigManager
    ) {
        this.extensionUri = extensionUri;
        this.client = client;
        this.configManager = configManager;
        
        // Listen for events from client
        this.client.onEvent(this.handleStreamEvent.bind(this));
        this.client.onError(this.handleError.bind(this));
    }
    
    resolveWebviewView(
        webviewView: vscode.WebviewView,
        context: vscode.WebviewViewResolveContext,
        _token: vscode.CancellationToken
    ): void {
        this.view = webviewView;
        
        webviewView.webview.options = {
            enableScripts: true,
            localResourceRoots: [this.extensionUri]
        };
        
        webviewView.webview.html = this.getHtmlContent(webviewView.webview);
        
        // Handle messages from webview
        webviewView.webview.onDidReceiveMessage(async (data) => {
            switch (data.type) {
                case 'sendMessage':
                    await this.handleUserMessage(data.content);
                    break;
                case 'clearHistory':
                    this.clearHistory();
                    break;
                case 'newSession':
                    await this.client.newSession();
                    this.clearHistory();
                    break;
                case 'getHistory':
                    this.postMessage({ type: 'history', messages: this.messages });
                    break;
            }
        });
    }
    
    private handleUserMessage(content: string): void {
        const userMessage: ChatMessage = {
            id: this.generateId(),
            role: 'user',
            content,
            timestamp: new Date()
        };
        
        this.messages.push(userMessage);
        this.postMessage({ type: 'newMessage', message: userMessage });
        
        // Prepare context
        const context: RequestContext | undefined = this.configManager.getAutoContext()
            ? this.buildContext()
            : undefined;
        
        // Send to client
        this.client.chat(content, context).catch(err => {
            this.handleError(err);
        });
        
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
    }
    
    private handleStreamEvent(event: StreamEvent): void {
        if (!this.currentAssistantMessage) {
            // Create one if not exists
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
                if (this.configManager.getShowToolUse()) {
                    const toolUse: ToolUse = {
                        id: event.id || '',
                        name: event.name || '',
                        input: event.input
                    };
                    this.currentAssistantMessage.toolUses?.push(toolUse);
                    this.postMessage({
                        type: 'updateMessage',
                        messageId: this.currentAssistantMessage.id,
                        field: 'toolUses',
                        value: this.currentAssistantMessage.toolUses
                    });
                }
                break;
                
            case 'tool_result':
                if (this.configManager.getShowToolUse() && event.tool_use_id) {
                    const toolUse = this.currentAssistantMessage.toolUses?.find(t => t.id === event.tool_use_id);
                    if (toolUse) {
                        toolUse.result = event.content;
                        this.postMessage({
                            type: 'updateMessage',
                            messageId: this.currentAssistantMessage.id,
                            field: 'toolUses',
                            value: this.currentAssistantMessage.toolUses
                        });
                    }
                }
                break;
                
            case 'done':
                this.currentAssistantMessage.isStreaming = false;
                this.postMessage({
                    type: 'updateMessage',
                    messageId: this.currentAssistantMessage.id,
                    field: 'isStreaming',
                    value: false
                });
                this.currentAssistantMessage = null;
                break;
                
            case 'error':
                this.handleError(new Error(event.content || 'Unknown error'));
                break;
        }
    }
    
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
    
    async sendQuickAction(
        action: string, 
        code: string, 
        context: RequestContext,
        instructions?: string
    ): Promise<void> {
        // Ensure view is visible
        if (this.view) {
            this.view.show(true);
        }
        
        // Create user message showing the action
        const actionLabels: Record<string, string> = {
            explain: 'Explain this code',
            fix: 'Fix this code',
            generateTests: 'Generate tests for this code',
            refactor: 'Refactor this code'
        };
        
        const userMessage: ChatMessage = {
            id: this.generateId(),
            role: 'user',
            content: `${actionLabels[action] || action}${instructions ? ` (${instructions})` : ''}\n\n\`\`\`${context.language || ''}\n${code}\n\`\`\``,
            timestamp: new Date()
        };
        
        this.messages.push(userMessage);
        this.postMessage({ type: 'newMessage', message: userMessage });
        
        // Send to client
        this.client.quickAction(action, code, context, instructions).catch(err => {
            this.handleError(err);
        });
        
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
    }
    
    clearHistory(): void {
        this.messages = [];
        this.currentAssistantMessage = null;
        this.postMessage({ type: 'clearHistory' });
    }
    
    private postMessage(message: unknown): void {
        if (this.view) {
            this.view.webview.postMessage(message);
        }
    }
    
    private buildContext(): RequestContext | undefined {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            return undefined;
        }
        
        const document = editor.document;
        const selection = editor.selection;
        
        return {
            workspace: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
            file: document.uri.fsPath,
            language: document.languageId,
            selection: {
                start: { line: selection.start.line, character: selection.start.character },
                end: { line: selection.end.line, character: selection.end.character }
            }
        };
    }
    
    private generateId(): string {
        return Math.random().toString(36).substring(2, 15);
    }
    
    private getHtmlContent(webview: vscode.Webview): string {
        return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>MatrixCode Chat</title>
    <style>
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        
        body {
            font-family: var(--vscode-font-family);
            color: var(--vscode-foreground);
            background: var(--vscode-sideBar-background);
            height: 100vh;
            display: flex;
            flex-direction: column;
        }
        
        #messages {
            flex: 1;
            overflow-y: auto;
            padding: 10px;
        }
        
        .message {
            margin-bottom: 12px;
            padding: 8px 12px;
            border-radius: 8px;
            max-width: 95%;
        }
        
        .message.user {
            background: var(--vscode-input-background);
            margin-left: auto;
            text-align: left;
        }
        
        .message.assistant {
            background: var(--vscode-editor-background);
            border: 1px solid var(--vscode-panel-border);
        }
        
        .message.system {
            background: var(--vscode-editorInfo-background, #4a4a4a);
            color: var(--vscode-editorInfo-foreground, white);
            font-style: italic;
            text-align: center;
        }
        
        .message-time {
            font-size: 0.75em;
            color: var(--vscode-descriptionForeground);
            margin-top: 4px;
        }
        
        .thinking-section, .tool-section {
            background: var(--vscode-editor-background);
            border: 1px solid var(--vscode-inputValidation-infoBorder, #6a6a6a);
            border-radius: 4px;
            padding: 6px 8px;
            margin-top: 8px;
            font-size: 0.85em;
        }
        
        .thinking-section {
            color: var(--vscode-descriptionForeground);
            font-style: italic;
        }
        
        .tool-section {
            display: flex;
            flex-direction: column;
        }
        
        .tool-name {
            font-weight: bold;
            color: var(--vscode-textLink-foreground);
        }
        
        .tool-result {
            margin-top: 4px;
            color: var(--vscode-descriptionForeground);
            font-size: 0.9em;
        }
        
        #input-area {
            padding: 10px;
            border-top: 1px solid var(--vscode-panel-border);
            display: flex;
            flex-direction: column;
            gap: 8px;
        }
        
        #message-input {
            width: 100%;
            min-height: 60px;
            max-height: 200px;
            resize: vertical;
            background: var(--vscode-input-background);
            color: var(--vscode-input-foreground);
            border: 1px solid var(--vscode-input-border);
            border-radius: 4px;
            padding: 8px;
            font-family: var(--vscode-font-family);
        }
        
        #message-input:focus {
            outline: 1px solid var(--vscode-focusBorder);
        }
        
        .button-row {
            display: flex;
            gap: 8px;
        }
        
        button {
            background: var(--vscode-button-background);
            color: var(--vscode-button-foreground);
            border: none;
            border-radius: 4px;
            padding: 6px 12px;
            cursor: pointer;
            font-family: var(--vscode-font-family);
        }
        
        button:hover {
            background: var(--vscode-button-hoverBackground);
        }
        
        button.secondary {
            background: var(--vscode-button-secondaryBackground);
            color: var(--vscode-button-secondaryForeground);
        }
        
        button.secondary:hover {
            background: var(--vscode-button-secondaryHoverBackground);
        }
        
        .streaming-indicator {
            display: inline-block;
            width: 8px;
            height: 8px;
            background: var(--vscode-progressBar-background);
            border-radius: 50%;
            animation: pulse 1.5s infinite;
            margin-left: 8px;
        }
        
        @keyframes pulse {
            0%, 100% { opacity: 0.4; }
            50% { opacity: 1; }
        }
        
        .error-message {
            color: var(--vscode-errorForeground);
            background: var(--vscode-inputValidation-errorBackground);
            border: 1px solid var(--vscode-inputValidation-errorBorder);
            padding: 8px;
            border-radius: 4px;
            margin: 8px;
        }
        
        pre {
            background: var(--vscode-textCodeBlock-background);
            padding: 8px;
            border-radius: 4px;
            overflow-x: auto;
            font-family: var(--vscode-editor-font-family);
            font-size: var(--vscode-editor-font-size);
        }
        
        code {
            font-family: var(--vscode-editor-font-family);
        }
    </style>
</head>
<body>
    <div id="messages"></div>
    
    <div id="input-area">
        <textarea id="message-input" placeholder="Ask MatrixCode..."></textarea>
        <div class="button-row">
            <button id="send-btn">Send</button>
            <button id="clear-btn" class="secondary">Clear</button>
            <button id="new-session-btn" class="secondary">New Session</button>
        </div>
    </div>
    
    <script>
        const vscode = acquireVsCodeApi();
        const messagesDiv = document.getElementById('messages');
        const messageInput = document.getElementById('message-input');
        const sendBtn = document.getElementById('send-btn');
        const clearBtn = document.getElementById('clear-btn');
        const newSessionBtn = document.getElementById('new-session-btn');
        
        // Send message
        sendBtn.addEventListener('click', sendMessage);
        messageInput.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                sendMessage();
            }
        });
        
        function sendMessage() {
            const content = messageInput.value.trim();
            if (content) {
                vscode.postMessage({ type: 'sendMessage', content });
                messageInput.value = '';
            }
        }
        
        clearBtn.addEventListener('click', () => {
            vscode.postMessage({ type: 'clearHistory' });
        });
        
        newSessionBtn.addEventListener('click', () => {
            vscode.postMessage({ type: 'newSession' });
        });
        
        // Handle messages from extension
        window.addEventListener('message', event => {
            const data = event.data;
            
            switch (data.type) {
                case 'newMessage':
                    appendMessage(data.message);
                    break;
                case 'updateMessage':
                    updateMessage(data.messageId, data.field, data.value);
                    break;
                case 'clearHistory':
                    messagesDiv.innerHTML = '';
                    break;
                case 'history':
                    messagesDiv.innerHTML = '';
                    data.messages.forEach(appendMessage);
                    break;
                case 'error':
                    showError(data.message);
                    break;
            }
        });
        
        function appendMessage(msg) {
            const div = document.createElement('div');
            div.className = 'message ' + msg.role;
            div.id = 'msg-' + msg.id;
            
            let html = formatContent(msg.content);
            if (msg.isStreaming) {
                html += '<span class="streaming-indicator"></span>';
            }
            
            if (msg.thinking && msg.thinking.length > 0) {
                html += '<div class="thinking-section">💭 ' + msg.thinking.join('') + '</div>';
            }
            
            if (msg.toolUses && msg.toolUses.length > 0) {
                msg.toolUses.forEach(tool => {
                    html += '<div class="tool-section">';
                    html += '<span class="tool-name">🔧 ' + tool.name + '</span>';
                    if (tool.result) {
                        html += '<span class="tool-result">' + truncate(tool.result, 200) + '</span>';
                    }
                    html += '</div>';
                });
            }
            
            html += '<div class="message-time">' + formatTime(msg.timestamp) + '</div>';
            
            div.innerHTML = html;
            messagesDiv.appendChild(div);
            scrollToBottom();
        }
        
        function updateMessage(id, field, value) {
            const msgDiv = document.getElementById('msg-' + id);
            if (!msgDiv) return;
            
            if (field === 'content') {
                const indicator = msgDiv.querySelector('.streaming-indicator');
                msgDiv.innerHTML = formatContent(value);
                if (indicator) msgDiv.appendChild(indicator);
            } else if (field === 'thinking') {
                // Update thinking section
            } else if (field === 'toolUses') {
                // Update tool sections
            } else if (field === 'isStreaming') {
                const indicator = msgDiv.querySelector('.streaming-indicator');
                if (indicator) indicator.remove();
            }
            
            scrollToBottom();
        }
        
        function showError(message) {
            const div = document.createElement('div');
            div.className = 'error-message';
            div.textContent = message;
            messagesDiv.appendChild(div);
            scrollToBottom();
            
            setTimeout(() => div.remove(), 5000);
        }
        
        function formatContent(content) {
            // Simple markdown-like formatting
            if (!content) return '';
            
            // Escape HTML
            content = content.replace(/</g, '&lt;').replace(/>/g, '&gt;');
            
            // Code blocks
            content = content.replace(/\\`\\`\\`([\\w]*)\\n([^\\`]+)\\`\\`\\`/g, '<pre><code>$2</code></pre>');
            
            // Inline code
            content = content.replace(/\\`([^\\`]+)\\`/g, '<code>$1</code>');
            
            // Bold
            content = content.replace(/\\*\\*([^\\*]+)\\*\\*/g, '<strong>$1</strong>');
            
            // Links
            content = content.replace(/\\[([^\\]]+)\\]\\(([^\\)]+)\\)/g, '<a href="$2">$1</a>');
            
            return content;
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
        
        // Request history on load
        vscode.postMessage({ type: 'getHistory' });
    </script>
</body>
</html>`;
    }
}