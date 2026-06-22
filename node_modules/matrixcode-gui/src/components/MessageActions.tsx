import React, { useState } from 'react';
import type { ChatMessage } from '../stores/chatStore';

interface MessageActionsProps {
  message: ChatMessage;
  onEdit?: (newContent: string) => void;
  onDelete?: () => void;
  onRetry?: () => void;
  onCopy?: (content: string) => void;
  onRegenerate?: () => void;  // For assistant messages
}

export function MessageActions({
  message,
  onEdit,
  onDelete,
  onRetry,
  onCopy,
  onRegenerate,
}: MessageActionsProps) {
  const [showEditDialog, setShowEditDialog] = useState(false);
  const [editContent, setEditContent] = useState(message.content);

  // Action buttons based on message type
  const isUser = message.role === 'user';
  const isAssistant = message.role === 'assistant';
  const isError = message.isError;
  const isTool = message.role === 'tool';

  // Handle copy with history
  const handleCopy = () => {
    navigator.clipboard.writeText(message.content);
    onCopy?.(message.content);
  };

  // Handle edit submit
  const handleEditSubmit = () => {
    if (editContent.trim() !== message.content.trim()) {
      onEdit?.(editContent.trim());
    }
    setShowEditDialog(false);
  };

  // Don't show actions for streaming messages
  if (message.isStreaming) {
    return null;
  }

  return (
    <>
      {/* Action buttons */}
      <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
        {/* Copy button (always available) */}
        <button
          onClick={handleCopy}
          className="p-1.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
          title="复制内容"
        >
          <span className="text-xs">📋</span>
        </button>

        {/* Edit button (user messages only) */}
        {isUser && onEdit && (
          <button
            onClick={() => setShowEditDialog(true)}
            className="p-1.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
            title="编辑消息"
          >
            <span className="text-xs">✏️</span>
          </button>
        )}

        {/* Retry button (error messages) */}
        {isError && onRetry && (
          <button
            onClick={onRetry}
            className="p-1.5 rounded hover:bg-red-500/20 text-red-500 transition-colors"
            title="重试"
          >
            <span className="text-xs">🔄</span>
          </button>
        )}

        {/* Regenerate button (assistant messages) */}
        {isAssistant && onRegenerate && (
          <button
            onClick={onRegenerate}
            className="p-1.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
            title="重新生成"
          >
            <span className="text-xs">🔄</span>
          </button>
        )}

        {/* Delete button (user messages only) */}
        {isUser && onDelete && (
          <button
            onClick={onDelete}
            className="p-1.5 rounded hover:bg-red-500/20 text-red-500 transition-colors"
            title="删除消息"
          >
            <span className="text-xs">🗑️</span>
          </button>
        )}

        {/* Copy tool input/result */}
        {isTool && (message.toolInput || message.content) && (
          <button
            onClick={() => handleCopy()}
            className="p-1.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
            title="复制工具数据"
          >
            <span className="text-xs">📋</span>
          </button>
        )}
      </div>

      {/* Edit dialog */}
      {showEditDialog && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
          <div className="bg-card border shadow-lg rounded-lg max-w-md w-full overflow-hidden">
            {/* Header */}
            <div className="p-4 border-b bg-muted/30">
              <h3 className="text-lg font-semibold flex items-center gap-2">
                <span>✏️</span>
                <span>Edit Message</span>
              </h3>
              <p className="text-sm text-muted-foreground mt-1">
                修改消息内容后重新发送
              </p>
            </div>

            {/* Edit textarea */}
            <div className="p-4">
              <textarea
                value={editContent}
                onChange={(e) => setEditContent(e.target.value)}
                className="w-full h-[200px] p-3 border rounded-lg text-sm resize-none focus:outline-none focus:ring-2 focus:ring-primary"
              />
              <div className="text-xs text-muted-foreground mt-2">
                {editContent.length} characters
              </div>
            </div>

            {/* Actions */}
            <div className="p-4 border-t bg-muted/30 flex gap-2">
              <button
                onClick={() => setShowEditDialog(false)}
                className="flex-1 px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleEditSubmit}
                disabled={editContent.trim() === ''}
                className="flex-1 px-4 py-2 bg-primary text-primary-foreground rounded-lg text-sm hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                保存并重新发送
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

// Quick actions menu for assistant messages
export function QuickActionsMenu({
  onRegenerate,
  onCopyResponse,
  onContinue,
}: {
  onRegenerate?: () => void;
  onCopyResponse?: () => void;
  onContinue?: () => void;
}) {
  return (
    <div className="flex items-center gap-2 mt-2 pt-2 border-t border-transparent">
      {onRegenerate && (
        <button
          onClick={onRegenerate}
          className="px-2 py-1 rounded text-xs bg-muted/50 text-muted-foreground hover:bg-muted transition-colors"
        >
          🔄 重新生成
        </button>
      )}
      {onCopyResponse && (
        <button
          onClick={onCopyResponse}
          className="px-2 py-1 rounded text-xs bg-muted/50 text-muted-foreground hover:bg-muted transition-colors"
        >
          📋 复制响应
        </button>
      )}
      {onContinue && (
        <button
          onClick={onContinue}
          className="px-2 py-1 rounded text-xs bg-muted/50 text-muted-foreground hover:bg-muted transition-colors"
        >
          ➡️ 继续生成
        </button>
      )}
    </div>
  );
}