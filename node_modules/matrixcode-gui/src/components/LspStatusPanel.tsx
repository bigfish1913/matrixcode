import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useToastContext } from '../contexts/ToastContext';

interface LspServerInfo {
  name: string;
  language: string;
  status: LspServerStatus;
}

// Backend returns enum LspServerStatus
type LspServerStatus = 'NotStarted' | 'Starting' | 'Connected' | { Error: string } | string;

// Convert backend status to frontend display status
const toDisplayStatus = (status: LspServerStatus): 'running' | 'stopped' | 'starting' | 'error' => {
  if (typeof status === 'string') {
    switch (status) {
      case 'Connected': return 'running';
      case 'NotStarted': return 'stopped';
      case 'Starting': return 'starting';
      default: return 'stopped';
    }
  }
  // Error variant is { Error: "message" }
  return 'error';
};

// Get error message from status
const getErrorMessage = (status: LspServerStatus): string | undefined => {
  if (typeof status === 'object' && 'Error' in status) {
    return status.Error;
  }
  return undefined;
};

export function LspStatusPanel({ onClose }: { onClose: () => void }) {
  const [lspServers, setLspServers] = useState<LspServerInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [operating, setOperating] = useState<string | null>(null);
  const toast = useToastContext();

  useEffect(() => {
    loadLspStatus();
  }, []);

  const loadLspStatus = async () => {
    try {
      const servers = await invoke<LspServerInfo[]>('get_lsp_status');
      setLspServers(servers || []);
    } catch (e) {
      console.error('Failed to load LSP status:', e);
      setLspServers([]);
    } finally {
      setLoading(false);
    }
  };

  const getStatusColor = (status: LspServerStatus) => {
    const displayStatus = toDisplayStatus(status);
    switch (displayStatus) {
      case 'running':
        return 'text-green-500';
      case 'starting':
        return 'text-yellow-500';
      case 'stopped':
        return 'text-gray-500';
      case 'error':
        return 'text-red-500';
      default:
        return 'text-muted-foreground';
    }
  };

  const getStatusIcon = (status: LspServerStatus) => {
    const displayStatus = toDisplayStatus(status);
    switch (displayStatus) {
      case 'running':
        return '●';
      case 'starting':
        return '◐';
      case 'stopped':
        return '○';
      case 'error':
        return '✗';
      default:
        return '?';
    }
  };

  // LSP lifecycle management
  const handleStartServer = async (serverName: string) => {
    try {
      setOperating(serverName);
      await invoke('start_lsp_server', { language: serverName });
      toast.addToast({ type: 'success', message: `LSP 服务器 ${serverName} 已启动` });
      await loadLspStatus();
    } catch (e) {
      toast.addToast({
        type: 'error',
        message: `启动失败: ${e instanceof Error ? e.message : '未知错误'}`
      });
    } finally {
      setOperating(null);
    }
  };

  const handleStopServer = async (serverName: string) => {
    try {
      setOperating(serverName);
      await invoke('stop_lsp_server', { language: serverName });
      toast.addToast({ type: 'success', message: `LSP 服务器 ${serverName} 已停止` });
      await loadLspStatus();
    } catch (e) {
      toast.addToast({
        type: 'error',
        message: `停止失败: ${e instanceof Error ? e.message : '未知错误'}`
      });
    } finally {
      setOperating(null);
    }
  };

  const handleRestartServer = async (serverName: string) => {
    try {
      setOperating(serverName);
      await invoke('restart_lsp_server', { language: serverName });
      toast.addToast({ type: 'success', message: `LSP 服务器 ${serverName} 已重启` });
      await loadLspStatus();
    } catch (e) {
      toast.addToast({
        type: 'error',
        message: `重启失败: ${e instanceof Error ? e.message : '未知错误'}`
      });
    } finally {
      setOperating(null);
    }
  };

  return (
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="lsp-dialog-title"
      onClick={(e) => {
        // Close on background click
        if (e.target === e.currentTarget) {
          onClose();
        }
      }}>
      <div className="bg-card border shadow-lg rounded-lg max-w-2xl w-full">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b">
          <h2 id="lsp-dialog-title" className="text-lg font-semibold">LSP 服务器状态</h2>
          <button
            onClick={onClose}
            className="p-1 hover:bg-accent rounded text-muted-foreground hover:text-foreground transition-colors"
            aria-label="关闭对话框"
          >
            ✕
          </button>
        </div>

        {/* Content */}
        <div className="p-4">
          {loading ? (
            <div className="text-center py-8 text-muted-foreground">
              加载 LSP 状态...
            </div>
          ) : lspServers.length === 0 ? (
            <div className="text-center py-8 text-muted-foreground">
              <p className="mb-2">No LSP servers configured</p>
              <p className="text-sm">
                Enable LSP in settings to get language server diagnostics
              </p>
            </div>
          ) : (
            <div className="space-y-3">
              {lspServers.map((server, idx) => {
                const displayStatus = toDisplayStatus(server.status);
                const errorMessage = getErrorMessage(server.status);

                return (
                  <div
                    key={idx}
                    className="border rounded-lg p-3 bg-background"
                  >
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center gap-2">
                        <span className={getStatusColor(server.status)}>
                          {getStatusIcon(server.status)}
                        </span>
                        <span className="font-medium">{server.name}</span>
                        <span className="text-sm text-muted-foreground px-2 py-0.5 bg-accent rounded">
                          {server.language}
                        </span>
                      </div>
                      <span className={`text-sm ${getStatusColor(server.status)}`}>
                        {displayStatus}
                      </span>
                    </div>

                    {errorMessage && (
                      <div className="text-sm text-red-500 mt-2 p-2 bg-red-500/10 rounded">
                        <span className="font-medium">Error: </span>
                        {errorMessage}
                      </div>
                    )}

                    {/* Lifecycle controls */}
                    <div className="mt-3 flex gap-2">
                      {(displayStatus === 'stopped' || displayStatus === 'starting') && (
                        <button
                          onClick={() => handleStartServer(server.language)}
                          disabled={operating === server.language}
                          className="px-3 py-1.5 text-xs bg-green-500/10 text-green-500 hover:bg-green-500/20 rounded transition-colors disabled:opacity-50"
                        >
                          {operating === server.language ? '启动中...' : '启动'}
                        </button>
                      )}
                      {displayStatus === 'running' && (
                        <button
                          onClick={() => handleStopServer(server.language)}
                          disabled={operating === server.language}
                          className="px-3 py-1.5 text-xs bg-red-500/10 text-red-500 hover:bg-red-500/20 rounded transition-colors disabled:opacity-50"
                        >
                          {operating === server.language ? '停止中...' : '停止'}
                        </button>
                      )}
                      {displayStatus === 'error' && (
                        <button
                          onClick={() => handleRestartServer(server.language)}
                          disabled={operating === server.language}
                          className="px-3 py-1.5 text-xs bg-yellow-500/10 text-yellow-500 hover:bg-yellow-500/20 rounded transition-colors disabled:opacity-50"
                        >
                          {operating === server.language ? '重启中...' : '重启'}
                        </button>
                      )}
                      {displayStatus === 'running' && (
                        <button
                          onClick={() => handleRestartServer(server.language)}
                          disabled={operating === server.language}
                          className="px-3 py-1.5 text-xs bg-yellow-500/10 text-yellow-500 hover:bg-yellow-500/20 rounded transition-colors disabled:opacity-50"
                        >
                          {operating === server.language ? '重启中...' : '重启'}
                        </button>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="border-t p-4 flex justify-between items-center">
          <button
            onClick={loadLspStatus}
            className="px-3 py-1.5 text-sm hover:bg-accent rounded transition-colors"
          >
            Refresh
          </button>
          <div className="text-xs text-muted-foreground">
            Press <kbd className="px-1 bg-accent rounded">Alt+L</kbd> to toggle
          </div>
        </div>
      </div>
    </div>
  );
}