import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface LspServerInfo {
  name: string;
  status: 'running' | 'stopped' | 'error';
  language?: string;
  command?: string;
  error?: string;
}

export function LspStatusPanel({ onClose }: { onClose: () => void }) {
  const [lspServers, setLspServers] = useState<LspServerInfo[]>([]);
  const [loading, setLoading] = useState(true);

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

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'running':
        return 'text-green-500';
      case 'stopped':
        return 'text-yellow-500';
      case 'error':
        return 'text-red-500';
      default:
        return 'text-muted-foreground';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'running':
        return '●';
      case 'stopped':
        return '○';
      case 'error':
        return '✗';
      default:
        return '?';
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4" onClick={(e) => {
      // Close on background click
      if (e.target === e.currentTarget) {
        onClose();
      }
    }}>
      <div className="bg-card border shadow-lg rounded-lg max-w-2xl w-full">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b">
          <h2 className="text-lg font-semibold">LSP Server Status</h2>
          <button
            onClick={onClose}
            className="p-1 hover:bg-accent rounded text-muted-foreground hover:text-foreground transition-colors"
          >
            ✕
          </button>
        </div>

        {/* Content */}
        <div className="p-4">
          {loading ? (
            <div className="text-center py-8 text-muted-foreground">
              Loading LSP status...
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
              {lspServers.map((server, idx) => (
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
                      {server.language && (
                        <span className="text-sm text-muted-foreground px-2 py-0.5 bg-accent rounded">
                          {server.language}
                        </span>
                      )}
                    </div>
                    <span className={`text-sm ${getStatusColor(server.status)}`}>
                      {server.status}
                    </span>
                  </div>
                  {server.command && (
                    <div className="text-sm text-muted-foreground mb-1">
                      <span className="font-medium">Command: </span>
                      <code className="bg-accent px-1 rounded text-xs">
                        {server.command}
                      </code>
                    </div>
                  )}
                  {server.error && (
                    <div className="text-sm text-red-500 mt-2 p-2 bg-red-500/10 rounded">
                      <span className="font-medium">Error: </span>
                      {server.error}
                    </div>
                  )}
                </div>
              ))}
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