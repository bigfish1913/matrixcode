import React, { useState } from 'react';
import { useChatStore, type ChatMessage } from '../stores/chatStore';

// Export formats
type ExportFormat = 'markdown' | 'json' | 'html' | 'txt' | 'pdf' | 'csv';

// Export options
interface ExportOptions {
  format: ExportFormat;
  includeThinking: boolean;
  includeToolCalls: boolean;
  includeTimestamps: boolean;
  includeMetadata: boolean;
  range?: {
    start: number;
    end: number;
  };
}

// Export dialog
interface ExportDialogProps {
  onClose: () => void;
}

export function ExportDialog({ onClose }: ExportDialogProps) {
  const messages = useChatStore((s) => s.messages);
  const [format, setFormat] = useState<ExportFormat>('markdown');
  const [options, setOptions] = useState<ExportOptions>({
    format: 'markdown',
    includeThinking: true,
    includeToolCalls: true,
    includeTimestamps: true,
    includeMetadata: false,
  });
  const [exporting, setExporting] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);

  // Format descriptions
  const FORMAT_INFO: Record<ExportFormat, { label: string; description: string; icon: string }> = {
    markdown: { label: 'Markdown', description: '适合阅读和编辑', icon: '📝' },
    json: { label: 'JSON', description: '结构化数据，适合程序处理', icon: '📊' },
    html: { label: 'HTML', description: '可在浏览器中查看', icon: '🌐' },
    txt: { label: 'Plain Text', description: '纯文本，通用格式', icon: '📄' },
    pdf: { label: 'PDF', description: '适合打印和分享', icon: '📑' },
    csv: { label: 'CSV', description: '表格数据', icon: '📈' },
  };

  // Generate export content
  const generateExportContent = (): string => {
    const selectedMessages = options.range
      ? messages.slice(options.range.start, options.range.end)
      : messages;

    switch (format) {
      case 'markdown':
        return generateMarkdown(selectedMessages, options);
      case 'json':
        return generateJSON(selectedMessages, options);
      case 'html':
        return generateHTML(selectedMessages, options);
      case 'txt':
        return generateTXT(selectedMessages, options);
      case 'csv':
        return generateCSV(selectedMessages, options);
      case 'pdf':
        return generatePDFPreview(selectedMessages, options);
      default:
        return '';
    }
  };

  // Markdown format
  const generateMarkdown = (msgs: ChatMessage[], opts: ExportOptions): string => {
    let content = '# MatrixCode Conversation Export\n\n';
    content += `Exported: ${new Date().toLocaleString()}\n\n`;
    content += '---\n\n';

    msgs.forEach((msg, idx) => {
      // Role header
      content += `## ${msg.role.toUpperCase()}\n\n`;

      // Timestamp
      if (opts.includeTimestamps && msg.timestamp) {
        content += `**Time:** ${new Date(msg.timestamp).toLocaleString()}\n\n`;
      }

      // Thinking
      if (opts.includeThinking && msg.thinking) {
        content += '### Thinking\n\n';
        content += '```thinking\n';
        content += msg.thinking;
        content += '\n```\n\n';
      }

      // Main content
      content += msg.content + '\n\n';

      // Tool calls
      if (opts.includeToolCalls && msg.toolName) {
        content += `### Tool: ${msg.toolName}\n\n`;
        if (msg.toolInput) {
          content += '**Input:**\n```json\n';
          content += JSON.stringify(msg.toolInput, null, 2);
          content += '\n```\n\n';
        }
        if (msg.isToolResult) {
          content += '**Result:**\n```\n';
          content += msg.content;
          content += '\n```\n\n';
        }
      }

      // Metadata
      if (opts.includeMetadata) {
        content += `> Message ID: ${msg.id}\n`;
        content += `> Index: ${idx}\n\n`;
      }

      content += '---\n\n';
    });

    return content;
  };

  // JSON format
  const generateJSON = (msgs: ChatMessage[], opts: ExportOptions): string => {
    const exportData = {
      version: '1.0',
      exported: new Date().toISOString(),
      messages: msgs.map((msg, idx) => ({
        id: msg.id,
        role: msg.role,
        content: msg.content,
        thinking: opts.includeThinking ? msg.thinking : undefined,
        toolName: opts.includeToolCalls ? msg.toolName : undefined,
        toolInput: opts.includeToolCalls ? msg.toolInput : undefined,
        isToolResult: opts.includeToolCalls ? msg.isToolResult : undefined,
        isError: msg.isError,
        timestamp: opts.includeTimestamps ? msg.timestamp : undefined,
        index: opts.includeMetadata ? idx : undefined,
      })),
    };

    return JSON.stringify(exportData, null, 2);
  };

  // HTML format
  const generateHTML = (msgs: ChatMessage[], opts: ExportOptions): string => {
    let html = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>MatrixCode Conversation Export</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }
    .message { margin: 20px 0; padding: 15px; border-radius: 8px; }
    .user { background: #e3f2fd; }
    .assistant { background: #f5f5f5; }
    .tool { background: #fff3e0; }
    .error { background: #ffebee; }
    .thinking { background: #f3e5f5; padding: 10px; border-radius: 4px; margin: 10px 0; }
    .timestamp { color: #666; font-size: 0.9em; }
    pre { background: #f5f5f5; padding: 10px; border-radius: 4px; overflow-x: auto; }
    code { font-family: 'Courier New', monospace; }
  </style>
</head>
<body>
  <h1>MatrixCode Conversation Export</h1>
  <p>Exported: ${new Date().toLocaleString()}</p>
  <hr>
`;

    msgs.forEach((msg) => {
      html += `<div class="message ${msg.role}">\n`;
      html += `<h3>${msg.role.toUpperCase()}</h3>\n`;

      if (opts.includeTimestamps && msg.timestamp) {
        html += `<p class="timestamp">${new Date(msg.timestamp).toLocaleString()}</p>\n`;
      }

      if (opts.includeThinking && msg.thinking) {
        html += `<div class="thinking">\n`;
        html += `<strong>Thinking:</strong>\n`;
        html += `<pre>${msg.thinking}</pre>\n`;
        html += `</div>\n`;
      }

      html += `<p>${msg.content.replace(/\n/g, '<br>')}</p>\n`;

      if (opts.includeToolCalls && msg.toolName) {
        html += `<p><strong>Tool: ${msg.toolName}</strong></p>\n`;
      }

      html += `</div>\n`;
    });

    html += '</body></html>';
    return html;
  };

  // Plain text format
  const generateTXT = (msgs: ChatMessage[], opts: ExportOptions): string => {
    let text = 'MatrixCode Conversation Export\n';
    text += `Exported: ${new Date().toLocaleString()}\n`;
    text += '='.repeat(50) + '\n\n';

    msgs.forEach((msg) => {
      text += `[${msg.role.toUpperCase()}]\n`;

      if (opts.includeTimestamps && msg.timestamp) {
        text += `Time: ${new Date(msg.timestamp).toLocaleString()}\n`;
      }

      if (opts.includeThinking && msg.thinking) {
        text += `\nThinking:\n${msg.thinking}\n`;
      }

      text += `\n${msg.content}\n`;

      if (opts.includeToolCalls && msg.toolName) {
        text += `\nTool: ${msg.toolName}\n`;
      }

      text += '\n' + '-'.repeat(50) + '\n\n';
    });

    return text;
  };

  // CSV format
  const generateCSV = (msgs: ChatMessage[], opts: ExportOptions): string => {
    const headers = ['Role', 'Content'];
    if (opts.includeTimestamps) headers.push('Timestamp');
    if (opts.includeThinking) headers.push('Thinking');
    if (opts.includeToolCalls) headers.push('Tool Name');

    let csv = headers.join(',') + '\n';

    msgs.forEach((msg) => {
      const row = [
        msg.role,
        `"${msg.content.replace(/"/g, '""')}"`,
      ];

      if (opts.includeTimestamps) {
        row.push(msg.timestamp ? new Date(msg.timestamp).toLocaleString() : '');
      }

      if (opts.includeThinking) {
        row.push(`"${(msg.thinking || '').replace(/"/g, '""')}"`);
      }

      if (opts.includeToolCalls) {
        row.push(msg.toolName || '');
      }

      csv += row.join(',') + '\n';
    });

    return csv;
  };

  // PDF preview (text only, actual PDF needs backend)
  const generatePDFPreview = (msgs: ChatMessage[], opts: ExportOptions): string => {
    return `PDF export preview\n\n${generateTXT(msgs, opts)}\n\nNote: PDF export requires backend support. This is a text preview.`;
  };

  // Preview content
  const handlePreview = () => {
    const content = generateExportContent();
    setPreview(content);
  };

  // Export content
  const handleExport = async () => {
    setExporting(true);
    try {
      const content = generateExportContent();
      const filename = `matrixcode-export-${new Date().toISOString().slice(0, 10)}.${getExtension(format)}`;
      const mimeType = getMimeType(format);

      // Create download
      const blob = new Blob([content], { type: mimeType });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = filename;
      a.click();
      URL.revokeObjectURL(url);

      onClose();
    } finally {
      setExporting(false);
    }
  };

  // Get file extension
  const getExtension = (fmt: ExportFormat): string => {
    const ext: Record<ExportFormat, string> = {
      markdown: 'md',
      json: 'json',
      html: 'html',
      txt: 'txt',
      pdf: 'pdf',
      csv: 'csv',
    };
    return ext[fmt];
  };

  // Get MIME type
  const getMimeType = (fmt: ExportFormat): string => {
    const mime: Record<ExportFormat, string> = {
      markdown: 'text/markdown',
      json: 'application/json',
      html: 'text/html',
      txt: 'text/plain',
      pdf: 'application/pdf',
      csv: 'text/csv',
    };
    return mime[fmt];
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-card border shadow-lg rounded-lg max-w-md w-full overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b bg-muted/30">
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-semibold flex items-center gap-2">
              <span>📤</span>
              <span>Export Conversation</span>
            </h3>
            <button
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent transition-colors"
            >
              ✕
            </button>
          </div>
          <p className="text-sm text-muted-foreground mt-1">
            Export {messages.length} messages
          </p>
        </div>

        {/* Format selection */}
        <div className="p-4">
          <div className="text-sm font-medium mb-2">Format</div>
          <div className="grid grid-cols-3 gap-2">
            {(Object.keys(FORMAT_INFO) as ExportFormat[]).map((fmt) => {
              const info = FORMAT_INFO[fmt];
              return (
                <button
                  key={fmt}
                  onClick={() => {
                    setFormat(fmt);
                    setOptions({ ...options, format: fmt });
                  }}
                  className={`p-3 rounded-lg border text-left ${
                    format === fmt ? 'border-primary bg-primary/10' : 'border-border hover:bg-accent/30'
                  }`}
                >
                  <div className="text-xl">{info.icon}</div>
                  <div className="font-medium text-sm">{info.label}</div>
                  <div className="text-xs text-muted-foreground">{info.description}</div>
                </button>
              );
            })}
          </div>
        </div>

        {/* Options */}
        <div className="px-4 py-2 border-t">
          <div className="text-sm font-medium mb-2">Options</div>
          <div className="space-y-2">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={options.includeThinking}
                onChange={(e) => setOptions({ ...options, includeThinking: e.target.checked })}
              />
              <span className="text-sm">Include Thinking</span>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={options.includeToolCalls}
                onChange={(e) => setOptions({ ...options, includeToolCalls: e.target.checked })}
              />
              <span className="text-sm">Include Tool Calls</span>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={options.includeTimestamps}
                onChange={(e) => setOptions({ ...options, includeTimestamps: e.target.checked })}
              />
              <span className="text-sm">Include Timestamps</span>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={options.includeMetadata}
                onChange={(e) => setOptions({ ...options, includeMetadata: e.target.checked })}
              />
              <span className="text-sm">Include Metadata</span>
            </label>
          </div>
        </div>

        {/* Preview */}
        {preview && (
          <div className="px-4 py-2 border-t">
            <div className="bg-muted/30 rounded p-3 max-h-[150px] overflow-auto">
              <pre className="text-xs">{preview.slice(0, 500)}</pre>
            </div>
          </div>
        )}

        {/* Actions */}
        <div className="p-4 border-t bg-muted/30">
          <div className="flex gap-2">
            <button
              onClick={handlePreview}
              className="px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors"
            >
              Preview
            </button>
            <button
              onClick={handleExport}
              disabled={exporting}
              className="flex-1 px-4 py-2 bg-primary text-primary-foreground rounded-lg text-sm hover:bg-primary/90 disabled:opacity-50 transition-colors"
            >
              {exporting ? 'Exporting...' : 'Export'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}