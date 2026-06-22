import React, { useState, useEffect, useRef } from 'react';
import { useSearchStore, highlightQueryParts, getRoleIcon, getRoleColor, truncateContent, type SearchFilters } from '../stores/searchStore';

interface SearchPanelProps {
  onClose: () => void;
  onSelectMessage?: (messageId: string) => void;
}

/** Render highlighted text from parts */
function renderHighlightedText(parts: Array<{ type: 'text' | 'match'; content: string }>): React.ReactNode {
  return parts.map((part, idx) => {
    if (part.type === 'match') {
      return (
        <span key={idx} className="bg-yellow-500/30 text-yellow-600 px-0.5 rounded font-medium">
          {part.content}
        </span>
      );
    }
    return part.content;
  });
}

export function SearchPanel({ onClose, onSelectMessage }: SearchPanelProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);
  const prevFocusRef = useRef<HTMLElement | null>(null);

  // Store state
  const {
    query,
    results,
    filters,
    currentResultIndex,
    searchMode,
    loading,
    selectedResult,
    search,
    searchInContent,
    searchInThinking,
    searchInTools,
    navigateNext,
    navigatePrev,
    setCurrentIndex,
    setFilters,
    clearSearch,
    setSelectedResult,
  } = useSearchStore();

  // Focus input on mount and trap focus
  useEffect(() => {
    // Store the previously focused element
    prevFocusRef.current = document.activeElement as HTMLElement;
    inputRef.current?.focus();

    // Focus trap handler
    const handleTab = (e: KeyboardEvent) => {
      if (e.key !== 'Tab') return;

      const modal = modalRef.current;
      if (!modal) return;

      // Get all focusable elements within the modal
      const focusableElements = modal.querySelectorAll<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      const firstElement = focusableElements[0];
      const lastElement = focusableElements[focusableElements.length - 1];

      if (e.shiftKey) {
        // Shift+Tab: if on first element, move to last
        if (document.activeElement === firstElement) {
          e.preventDefault();
          lastElement?.focus();
        }
      } else {
        // Tab: if on last element, move to first
        if (document.activeElement === lastElement) {
          e.preventDefault();
          firstElement?.focus();
        }
      }
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        navigateNext();
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        navigatePrev();
      } else if (e.key === 'Enter' && selectedResult) {
        e.preventDefault();
        onSelectMessage?.(selectedResult.messageId);
        onClose();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      } else if (e.key === 'Tab') {
        handleTab(e);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      // Restore focus to the previous element on close
      prevFocusRef.current?.focus();
    };
  }, [selectedResult, navigateNext, navigatePrev, onSelectMessage, onClose]);

  // Handle query change
  const handleQueryChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    search(e.target.value);
  };

  // Handle filter change
  const handleFilterChange = (newFilters: Partial<SearchFilters>) => {
    setFilters(newFilters);
  };

  // Format timestamp - handle undefined/null/invalid values
  const formatTime = (timestamp: number | undefined | null): string => {
    if (timestamp === undefined || timestamp === null || timestamp === 0) {
      return '未知时间';
    }
    try {
      return new Date(timestamp).toLocaleString('zh-CN');
    } catch {
      return '未知时间';
    }
  };

  // Render result item
  const renderResultItem = (result: typeof results[0], index: number) => {
    const isSelected = index === currentResultIndex;
    const roleIcon = getRoleIcon(result.role);
    const roleColor = getRoleColor(result.role);

    return (
      <div
        key={result.messageId}
        onClick={() => {
          setCurrentIndex(index);
          onSelectMessage?.(result.messageId);
          onClose();
        }}
        className={`px-4 py-3 cursor-pointer border-b transition-colors ${
          isSelected ? 'bg-primary/10' : 'hover:bg-accent/30'
        }`}
        aria-label={`搜索结果 ${index + 1}: ${result.role}`}
      >
        <div className="flex items-start gap-3">
          {/* Role icon */}
          <span className={`text-lg ${roleColor}`} aria-hidden="true">
            {roleIcon}
          </span>

          {/* Content preview */}
          <div className="flex-1">
            {/* Highlighted content */}
            <div className="text-sm">
              {query.trim() !== ''
                ? renderHighlightedText(highlightQueryParts(result.context, query))
                : truncateContent(result.content)}
            </div>

            {/* Thinking indicator */}
            {result.hasThinking && (
              <div className="text-xs text-purple-500 mt-1 flex items-center gap-1">
                <span aria-hidden="true">💭</span>
                <span>包含思考内容</span>
              </div>
            )}

            {/* Code indicator */}
            {result.hasCode && (
              <div className="text-xs text-blue-500 mt-1 flex items-center gap-1">
                <span aria-hidden="true">💻</span>
                <span>包含代码</span>
              </div>
            )}

            {/* Tool name */}
            {result.toolName && (
              <div className="text-xs text-amber-500 mt-1 flex items-center gap-1">
                <span aria-hidden="true">🔧</span>
                <span>{result.toolName}</span>
              </div>
            )}

            {/* Metadata */}
            <div className="flex items-center gap-3 mt-2 text-xs text-muted-foreground">
              <span>{formatTime(result.timestamp)}</span>
              <span>{result.content.length} 字符</span>
              {result.role === 'tool' && (
                <span className="text-amber-500">
                  {result.toolName ? '工具调用' : '工具结果'}
                </span>
              )}
            </div>
          </div>

          {/* Selected indicator */}
          {isSelected && (
            <span className="text-primary text-xs" aria-hidden="true">●</span>
          )}
        </div>
      </div>
    );
  };

  // Render search mode buttons
  const renderSearchModeButtons = () => (
    <div className="flex gap-2">
      <button
        onClick={() => searchInContent(query)}
        className={`px-3 py-1.5 rounded text-xs transition-colors ${
          searchMode === 'content' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-accent'
        }`}
        aria-label="搜索消息内容"
      >
        消息内容
      </button>
      <button
        onClick={() => searchInThinking(query)}
        className={`px-3 py-1.5 rounded text-xs transition-colors ${
          searchMode === 'thinking' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-accent'
        }`}
        aria-label="搜索思考内容"
      >
        思考内容
      </button>
      <button
        onClick={() => searchInTools(query)}
        className={`px-3 py-1.5 rounded text-xs transition-colors ${
          searchMode === 'tool' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-accent'
        }`}
        aria-label="搜索工具调用"
      >
        工具调用
      </button>
    </div>
  );

  // Render filter buttons
  const renderFilterButtons = () => (
    <div className="flex flex-wrap gap-2">
      {/* Role filter */}
      <select
        value={filters.role}
        onChange={(e) => handleFilterChange({ role: e.target.value as any })}
        className="px-3 py-1.5 bg-muted rounded text-xs outline-none"
        aria-label="按角色筛选"
      >
        <option value="all">全部角色</option>
        <option value="user">👤 用户</option>
        <option value="assistant">🤖 助手</option>
        <option value="tool">🔧 工具</option>
        <option value="error">❌ 错误</option>
      </select>

      {/* Date range filter */}
      <select
        value={filters.dateRange}
        onChange={(e) => handleFilterChange({ dateRange: e.target.value as any })}
        className="px-3 py-1.5 bg-muted rounded text-xs outline-none"
        aria-label="按时间筛选"
      >
        <option value="all">全部时间</option>
        <option value="today">今天</option>
        <option value="week">本周</option>
        <option value="month">本月</option>
      </select>

      {/* Has code filter */}
      <button
        onClick={() => handleFilterChange({ hasCode: !filters.hasCode })}
        className={`px-3 py-1.5 rounded text-xs transition-colors ${
          filters.hasCode ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-accent'
        }`}
        aria-label={filters.hasCode ? '取消代码筛选' : '筛选包含代码的消息'}
      >
        💻 有代码
      </button>

      {/* Has thinking filter */}
      <button
        onClick={() => handleFilterChange({ hasThinking: !filters.hasThinking })}
        className={`px-3 py-1.5 rounded text-xs transition-colors ${
          filters.hasThinking ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-accent'
        }`}
        aria-label={filters.hasThinking ? '取消思考筛选' : '筛选包含思考的消息'}
      >
        💭 有思考
      </button>

      {/* Clear filters */}
      {(filters.role !== 'all' ||
        filters.dateRange !== 'all' ||
        filters.hasCode ||
        filters.hasThinking) && (
        <button
          onClick={() => handleFilterChange({
            role: 'all',
            dateRange: 'all',
            hasCode: false,
            hasThinking: false,
          })}
          className="px-3 py-1.5 bg-muted rounded text-xs hover:bg-accent transition-colors"
          aria-label="清除筛选"
        >
          清除筛选
        </button>
      )}
    </div>
  );

  return (
    <div
      ref={modalRef}
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="search-panel-title"
    >
      <div className="bg-card border shadow-lg rounded-lg max-w-2xl w-full max-h-[80vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="p-4 border-b bg-muted/30">
          <div className="flex items-center justify-between">
            <h3 id="search-panel-title" className="text-lg font-semibold flex items-center gap-2">
              <span aria-hidden="true">🔍</span>
              <span>消息搜索</span>
            </h3>
            <button
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent transition-colors"
              aria-label="关闭"
            >
              ✕
            </button>
          </div>
        </div>

        {/* Search input and filters */}
        <div className="p-4 border-b space-y-3">
          {/* Query input */}
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={handleQueryChange}
            placeholder="搜索消息内容、思考内容、工具名称..."
            className="w-full bg-muted rounded-lg px-4 py-2 text-sm outline-none focus:ring-2 focus:ring-primary"
            aria-label="搜索输入"
          />

          {/* Search mode buttons */}
          {renderSearchModeButtons()}

          {/* Filter buttons */}
          {renderFilterButtons()}
        </div>

        {/* Results */}
        <div className="flex-1 overflow-y-auto">
          {/* Results count */}
          <div className="px-4 py-2 text-xs text-muted-foreground bg-muted/20">
            找到 {results.length} 条消息
            {query.trim() !== '' && ` (搜索: "${query}")`}
          </div>

          {loading ? (
            <div className="text-center text-muted-foreground py-8">
              <div className="animate-spin text-2xl mb-2" aria-hidden="true">⏳</div>
              <span className="text-sm">搜索中...</span>
            </div>
          ) : results.length === 0 ? (
            <div className="text-center text-muted-foreground py-8">
              <div className="text-4xl mb-2" aria-hidden="true">🔍</div>
              <span className="text-sm">没有找到匹配的消息</span>
              <p className="text-xs mt-1">
                尝试修改搜索条件
              </p>
            </div>
          ) : (
            results.map((result, idx) => renderResultItem(result, idx))
          )}
        </div>

        {/* Navigation and Footer */}
        <div className="p-4 border-t bg-muted/30">
          <div className="flex justify-between items-center mb-2">
            <div className="flex gap-2 text-xs text-muted-foreground">
              <span><kbd className="px-1.5 py-0.5 bg-muted rounded">↑↓</kbd> 导航</span>
              <span><kbd className="px-1.5 py-0.5 bg-muted rounded">Enter</kbd> 选择</span>
              <span><kbd className="px-1.5 py-0.5 bg-muted rounded">Esc</kbd> 关闭</span>
            </div>
            {results.length > 0 && (
              <span className="text-xs text-muted-foreground">
                {currentResultIndex + 1} / {results.length}
              </span>
            )}
          </div>
          <div className="flex gap-2">
            <button
              onClick={navigatePrev}
              disabled={results.length === 0}
              className="flex-1 px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors disabled:opacity-50"
              aria-label="上一个结果"
            >
              上一个
            </button>
            <button
              onClick={navigateNext}
              disabled={results.length === 0}
              className="flex-1 px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors disabled:opacity-50"
              aria-label="下一个结果"
            >
              下一个
            </button>
            <button
              onClick={clearSearch}
              className="px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors"
              aria-label="清除搜索"
            >
              清除
            </button>
            <button
              onClick={onClose}
              className="px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors"
              aria-label="关闭"
            >
              关闭
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}