#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

console.log('开始修改 chatPanel.ts...');

const filePath = path.join(__dirname, 'src', 'chatPanel.ts');
let content = fs.readFileSync(filePath, 'utf8');

// ===== 1. 添加 SessionManager 导入 =====
content = content.replace(
    "import { MatrixCodeClient, StreamEvent, RequestContext } from './matrixcodeClient';",
    "import { MatrixCodeClient, StreamEvent, RequestContext } from './matrixcodeClient';\nimport { SessionManager, Session } from './sessionManager';"
);

// ===== 2. 在类中添加 sessionManager 字段 =====
content = content.replace(
    'private outputChannel: vscode.OutputChannel;',
    'private outputChannel: vscode.OutputChannel;\n    private sessionManager: SessionManager;\n    private currentSession: Session | null = null;'
);

// ===== 3. 修改构造函数 =====
content = content.replace(
    'constructor(\n        extensionUri: vscode.Uri,\n        client: MatrixCodeClient,\n        configManager: ConfigManager,\n        outputChannel: vscode.OutputChannel\n    )',
    'constructor(\n        extensionUri: vscode.Uri,\n        client: MatrixCodeClient,\n        configManager: ConfigManager,\n        sessionManager: SessionManager,\n        outputChannel: vscode.OutputChannel\n    )'
);

content = content.replace(
    'this.outputChannel = outputChannel;',
    'this.outputChannel = outputChannel;\n        this.sessionManager = sessionManager;\n        this.currentSession = sessionManager.createSession();'
);

// ===== 4. 在 generateId 方法前添加 Session 管理方法 =====
const sessionMethods = `
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
    
    /**
     * Load a session.
     */
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
    
    /**
     * New session.
     */
    private async newSession(): Promise<void> {
        if (this.messages.length > 0) {
            const save = await vscode.window.showQuickPick(
                ['Save and start new', 'Discard and start new', 'Cancel'],
                { placeHolder: 'Current session has messages. What to do?' }
            );
            
            if (save === 'Cancel') return;
            if (save === 'Save and start new') {
                await this.saveCurrentSession();
            }
        }
        
        this.currentSession = this.sessionManager.createSession();
        this.messages = [];
        this.postMessage({ type: 'clearMessages' });
        vscode.window.showInformationMessage('New session started');
    }
    
    /**
     * Delete a session.
     */
    private async deleteSession(): Promise<void> {
        const session = await this.sessionManager.showSessionPicker();
        if (session) {
            const confirm = await vscode.window.showQuickPick(
                ['Delete', 'Cancel'],
                { placeHolder: 'Delete session: ' + session.name + '?' }
            );
            
            if (confirm === 'Delete') {
                await this.sessionManager.deleteSession(session.id);
                vscode.window.showInformationMessage('Session deleted: ' + session.name);
            }
        }
    }
    
`;

const markerPos = content.indexOf('    private generateId(): string {');
if (markerPos > 0) {
    content = content.substring(0, markerPos) + sessionMethods + content.substring(markerPos);
}

// ===== 5. 修改 handleWebviewMessage =====
content = content.replace(
    "case 'newSession':\n                this.clearHistory();\n                break;",
    "case 'newSession':\n                await this.newSession();\n                break;\n            case 'saveSession':\n                await this.saveCurrentSession();\n                break;\n            case 'loadSession':\n                await this.loadSession();\n                break;\n            case 'deleteSession':\n                await this.deleteSession();\n                break;"
);

// ===== 6. 修改 UI 按钮 =====
content = content.replace(
    '<div class="header-actions">\n            <button class="header-btn" onclick="newSession()">New Session</button>\n            <button class="header-btn" onclick="clearHistory()">Clear</button>\n        </div>',
    '<div class="header-actions">\n            <button class="header-btn" onclick="newSession()">📝 New</button>\n            <button class="header-btn" onclick="saveSession()">💾 Save</button>\n            <button class="header-btn" onclick="loadSession()">📂 Load</button>\n            <button class="header-btn" onclick="clearHistory()">🗑️ Clear</button>\n        </div>'
);

// ===== 7. 添加 webview 函数 =====
const webviewFuncs = `function newSession() {
            vscode.postMessage({ type: 'newSession' });
        }
        
        function saveSession() {
            vscode.postMessage({ type: 'saveSession' });
        }
        
        function loadSession() {
            vscode.postMessage({ type: 'loadSession' });
        }
        
        function deleteSession() {
            vscode.postMessage({ type: 'deleteSession' });
        }
        `;

content = content.replace(
    'function newSession() {\n            vscode.postMessage({ type: \'newSession\' });\n        }',
    webviewFuncs
);

// ===== 8. 修复 Thinking 显示 =====
content = content.replace(
    "message.thinking.map(t => escapeHtml(t)).join('<br>');",
    "message.thinking.map(t => escapeHtml(t)).join(' ');"
);

content = content.replace(
    "value.map(t => escapeHtml(t)).join('<br>');",
    "value.map(t => escapeHtml(t)).join(' ');"
);

// ===== 9. 添加 Markdown 支持 =====
// 找到 formatContent 函数中的 "result += escapeHtml(line) + '<br>';" 这行
content = content.replace(
    "result += escapeHtml(line) + '<br>';",
    "result += processInlineMarkdown(escapeHtml(line)) + '<br>';"
);

// 在 formatContent 函数结束后添加 processInlineMarkdown 函数
const formatContentEnd = "return result;\n        }\n        \n        function renderCodeBlock";
const processInlineFunc = `return result;
        }
        
        // Process inline markdown (bold, italic, code, links)
        function processInlineMarkdown(text) {
            // Inline code: \\x60code\\x60 (backtick)
            text = text.replace(/\\x60([^\\x60]+)\\x60/g, '<code style="background:rgba(100,100,100,0.15);padding:1px 3px;border-radius:2px;">$1</code>');
            
            // Bold: **text**
            text = text.replace(/\\*\\*([^*]+)\\*\\*/g, '<strong style="font-weight:600;">$1</strong>');
            
            // Italic: *text*
            text = text.replace(/\\*([^*]+)\\*/g, '<em style="font-style:italic;">$1</em>');
            
            // Links: [text](url)
            text = text.replace(/\\[([^\\]]+)\\]\\(([^)]+)\\)/g, '<a href="$2" style="color:#0af;text-decoration:none;" target="_blank">$1</a>');
            
            return text;
        }
        
        function renderCodeBlock`;

content = content.replace(formatContentEnd, processInlineFunc);

// ===== 写入文件 =====
fs.writeFileSync(filePath, content, 'utf8');
console.log('✅ chatPanel.ts 已成功修改');