/**
 * MatrixCode Chat Panel Script
 * Uses marked.js for Markdown rendering and highlight.js for code syntax highlighting
 */

// VSCode API
const vscode = acquireVsCodeApi();

// DOM Elements
const messagesDiv = document.getElementById('messages');
const inputField = document.getElementById('input');
const sendBtn = document.getElementById('sendBtn');
const emptyState = document.getElementById('empty');
const spinner = document.getElementById('spinner');
const statusText = document.getElementById('status-text');
const modelInfo = document.getElementById('model-info');

// Configure marked
if (typeof marked !== 'undefined') {
    marked.setOptions({
        highlight: function(code, lang) {
            if (lang && hljs.getLanguage(lang)) {
                try {
                    return hljs.highlight(code, { language: lang }).value;
                } catch (e) {}
            }
            return hljs.highlightAuto(code).value;
        },
        breaks: true,
        gfm: true
    });
}

// Tool icons mapping
const toolIcons = {
    read: '📖',
    write: '📝',
    edit: '✏️',
    multi_edit: '📝',
    bash: '⚡',
    search: '🔍',
    glob: '📁',
    ls: '📂',
    ask: '❓',
    websearch: '🌐',
    webfetch: '🔗',
    skill: '🔧',
    todo_write: '📋',
    check: '✅'
};

// State
let currentMessageId = null;

// Auto-resize textarea
inputField.addEventListener('input', function() {
    this.style.height = 'auto';
    this.style.height = Math.min(this.scrollHeight, 200) + 'px';
});

// Handle keyboard shortcuts
inputField.addEventListener('keydown', function(e) {
    if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        sendMessage();
    }
});

/**
 * Send a message to the extension
 */
function sendMessage() {
    const content = inputField.value.trim();
    if (!content) return;
    
    vscode.postMessage({ type: 'sendMessage', content: content });
    inputField.value = '';
    inputField.style.height = '60px';
    sendBtn.disabled = true;
    setStatus('Thinking...', true);
}

/**
 * Start a new session
 */
function newSession() {
    vscode.postMessage({ type: 'newSession' });
}

/**
 * Clear chat history
 */
function clearHistory() {
    vscode.postMessage({ type: 'clearHistory' });
}

/**
 * Apply code to editor
 */
function applyCode(code, filename) {
    vscode.postMessage({ type: 'applyCode', code: code, filename: filename });
}

/**
 * Copy code to clipboard
 */
function copyCode(code) {
    vscode.postMessage({ type: 'copyCode', code: code });
}

/**
 * Set status text
 */
function setStatus(text, isLoading) {
    statusText.textContent = text;
    spinner.classList.toggle('active', isLoading);
}

/**
 * Add a message to the chat
 */
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
    contentDiv.className = 'message-content markdown-content';
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
            '<div class="thinking-content">' + message.thinking.map(t => escapeHtml(t)).join('<br>') + '</div>';
        thinkingDiv.onclick = () => thinkingDiv.classList.toggle('collapsed');
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

/**
 * Update an existing message
 */
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
                '<div class="thinking-content">' + value.map(t => escapeHtml(t)).join('<br>') + '</div>';
        } else if (!thinkingDiv && value && value.length > 0) {
            // Create thinking div
            const msgDiv = document.getElementById('msg-' + messageId);
            if (msgDiv) {
                const newThinkingDiv = document.createElement('div');
                newThinkingDiv.className = 'thinking-block';
                newThinkingDiv.id = 'thinking-' + messageId;
                newThinkingDiv.innerHTML = '<div class="thinking-header">💭 Thinking</div>' +
                    '<div class="thinking-content">' + value.map(t => escapeHtml(t)).join('<br>') + '</div>';
                newThinkingDiv.onclick = () => newThinkingDiv.classList.toggle('collapsed');
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

/**
 * Format message content using marked.js for full Markdown support
 */
function formatContent(content) {
    if (!content) return '';
    
    // Use marked for full Markdown rendering
    if (typeof marked !== 'undefined') {
        try {
            let html = marked.parse(content);
            // Post-process: add copy/apply buttons to code blocks
            html = enhanceCodeBlocks(html);
            return html;
        } catch (e) {
            console.error('Markdown parsing error:', e);
            return escapeHtml(content);
        }
    }
    
    // Fallback: basic formatting
    return fallbackFormat(content);
}

/**
 * Enhance code blocks with copy/apply buttons
 */
function enhanceCodeBlocks(html) {
    // Add buttons to <pre><code> blocks
    return html.replace(/<pre><code(?: class="language-(\w+)")?>/g, (match, lang) => {
        const codeId = 'code-' + Math.random().toString(36).substr(2, 9);
        const displayLang = lang || 'text';
        return `<div class="code-block">
            <div class="code-header">
                <span class="code-language">${displayLang}</span>
                <div class="code-actions">
                    <button class="code-btn" onclick="copyCodeById('${codeId}')">Copy</button>
                    <button class="code-btn" onclick="applyCodeById('${codeId}')">Apply</button>
                </div>
            </div>
            <pre class="code-content" id="${codeId}"><code>`;
    }).replace(/<\/code></pre>/g, '</code></pre></div>');
}

/**
 * Fallback formatting when marked is not available
 */
function fallbackFormat(content) {
    let result = '';
    let inCodeBlock = false;
    let codeLang = '';
    let codeContent = '';
    let lines = content.split('\n');
    
    for (let i = 0; i < lines.length; i++) {
        let line = lines[i];
        
        if (!inCodeBlock && line.startsWith('```')) {
            inCodeBlock = true;
            codeLang = line.substring(3).trim() || 'text';
            codeContent = '';
            continue;
        }
        
        if (inCodeBlock && line.startsWith('```')) {
            inCodeBlock = false;
            result += renderCodeBlock(codeLang, codeContent);
            continue;
        }
        
        if (inCodeBlock) {
            codeContent += line + '\n';
        } else {
            result += processInlineMarkdown(escapeHtml(line)) + '<br>';
        }
    }
    
    if (inCodeBlock) {
        result += renderCodeBlock(codeLang, codeContent);
    }
    
    return result;
}

/**
 * Render a code block (fallback)
 */
function renderCodeBlock(lang, code) {
    const escapedCode = escapeHtml(code.trim());
    const displayLang = lang || 'text';
    const codeId = 'code-' + Math.random().toString(36).substr(2, 9);
    
    return '<div class="code-block">' +
        '<div class="code-header">' +
        '<span class="code-language">' + displayLang + '</span>' +
        '<div class="code-actions">' +
        '<button class="code-btn" onclick="copyCodeById(\'' + codeId + '\')">Copy</button>' +
        '<button class="code-btn" onclick="applyCodeById(\'' + codeId + '\')">Apply</button>' +
        '</div>' +
        '</div>' +
        '<div class="code-content" id="' + codeId + '"><pre><code>' + escapedCode + '</code></pre></div>' +
        '</div>';
}

/**
 * Copy code by element ID
 */
function copyCodeById(id) {
    const codeDiv = document.getElementById(id);
    if (codeDiv) {
        // Get text content, stripping HTML
        const code = codeDiv.textContent || codeDiv.innerText;
        copyCode(code);
    }
}

/**
 * Apply code by element ID
 */
function applyCodeById(id) {
    const codeDiv = document.getElementById(id);
    if (codeDiv) {
        const code = codeDiv.textContent || codeDiv.innerText;
        applyCode(code);
    }
}

/**
 * Escape HTML special characters
 */
function escapeHtml(text) {
    return text
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;');
}

/**
 * Process inline Markdown (bold, italic, links, code)
 */
function processInlineMarkdown(text) {
    // Bold
    text = text.replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>');
    // Italic
    text = text.replace(/\*(.*?)\*/g, '<em>$1</em>');
    // Inline code
    text = text.replace(/`([^`]+)`/g, '<code>$1</code>');
    // Links
    text = text.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');
    return text;
}

/**
 * Render tool uses section
 */
function renderToolUses(toolUses) {
    if (!toolUses || toolUses.length === 0) return '';
    
    let html = '';
    for (const tool of toolUses) {
        const icon = toolIcons[tool.name] || '🔧';
        const statusClass = tool.status || 'running';
        const statusLabel = tool.status === 'running' ? '⏳ Running' : 
                           tool.status === 'done' ? '✓ Done' : '✗ Error';
        
        // Format input for display
        let inputDisplay = '';
        try {
            inputDisplay = JSON.stringify(tool.input, null, 2);
        } catch (e) {
            inputDisplay = String(tool.input);
        }
        
        html += '<div class="tool-use-card">' +
            '<div class="tool-use-header">' +
            '<span class="tool-use-icon">' + icon + '</span>' +
            '<span class="tool-use-name">' + tool.name + '</span>' +
            '<span class="tool-use-status ' + statusClass + '">' + statusLabel + '</span>' +
            '</div>' +
            '<div class="tool-use-body">' +
            '<div class="tool-use-input"><pre>' + escapeHtml(inputDisplay) + '</pre></div>' +
            (tool.result ? '<div class="tool-use-result"><pre>' + escapeHtml(truncate(tool.result, 500)) + '</pre></div>' : '') +
            '</div>' +
            '</div>';
    }
    return html;
}

/**
 * Truncate string to max length
 */
function truncate(str, max) {
    if (!str) return '';
    if (str.length > max) {
        return str.substring(0, max) + '...';
    }
    return str;
}

/**
 * Format timestamp
 */
function formatTime(date) {
    if (typeof date === 'string') date = new Date(date);
    return date.toLocaleTimeString();
}

/**
 * Scroll to bottom of messages
 */
function scrollToBottom() {
    messagesDiv.scrollTop = messagesDiv.scrollHeight;
}

/**
 * Clear all messages
 */
function clearMessages() {
    messagesDiv.innerHTML = 
        '<div class="empty-state" id="empty">' +
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
        case 'modelInfo':
            modelInfo.textContent = message.model;
            break;
    }
});

// Handle link clicks in markdown content - send to VS Code to open files
document.addEventListener('click', function(e) {
    const target = e.target;
    if (target.tagName === 'A' && target.closest('.markdown-content')) {
        e.preventDefault();
        const href = target.getAttribute('href');
        if (href) {
            // Parse link format: filename.ts or filename.ts:42 or filename.ts:42-51
            vscode.postMessage({ type: 'openFile', path: href });
        }
    }
});

// Request history on load
vscode.postMessage({ type: 'getHistory' });

// Focus input on load
inputField.focus();