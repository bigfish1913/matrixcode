import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';

// Global state context for server status (matching TUI mcp_servers, lsp_servers, codegraph_status)
interface ServerStatusState {
  mcp: {
    servers: Array<{ name: string; status: string; tools?: string[] }>;
    connected: boolean;
  };
  lsp: {
    servers: Array<{ name: string; status: string; language?: string }>;
    connected: boolean;
  };
  codegraph: {
    initialized: boolean;
    indexing: boolean;
    filesIndexed: number;
    symbolsIndexed: number;
    pendingFiles: number;
  };
}

interface ServerStatusContextValue {
  status: ServerStatusState;
  refreshStatus: () => Promise<void>;
  isLoading: boolean;
}

const ServerStatusContext = createContext<ServerStatusContextValue | null>(null);

// Hook to access server status - returns full context value
export function useServerStatus(): ServerStatusContextValue {
  const context = useContext(ServerStatusContext);
  if (!context) {
    // Return default value if context is not available
    return {
      status: {
        mcp: { servers: [], connected: false },
        lsp: { servers: [], connected: false },
        codegraph: {
          initialized: false,
          indexing: false,
          filesIndexed: 0,
          symbolsIndexed: 0,
          pendingFiles: 0,
        },
      },
      refreshStatus: async () => {},
      isLoading: false,
    };
  }
  return context;
}

// Individual status hooks for convenience
export function useMcpStatus() {
  const { status } = useServerStatus();
  return status.mcp;
}

export function useLspStatus() {
  const { status } = useServerStatus();
  return status.lsp;
}

export function useCodeGraphStatus() {
  const { status } = useServerStatus();
  return status.codegraph;
}

// Status badge component for individual servers
export function ServerStatusBadge({
  type,
  name,
  status: serverStatus
}: {
  type: 'mcp' | 'lsp' | 'codegraph';
  name: string;
  status: string;
}) {
  const statusColors = {
    connected: 'text-green-500',
    running: 'text-green-500',
    initialized: 'text-green-500',
    disconnected: 'text-gray-400',
    stopped: 'text-gray-400',
    initializing: 'text-yellow-500 animate-pulse',
    indexing: 'text-yellow-500 animate-pulse',
    error: 'text-red-500',
  };

  const statusIcons = {
    connected: '●',
    running: '●',
    initialized: '●',
    disconnected: '○',
    stopped: '○',
    initializing: '◐',
    indexing: '◐',
    error: '✗',
  };

  const color = statusColors[serverStatus as keyof typeof statusColors] || 'text-gray-400';
  const icon = statusIcons[serverStatus as keyof typeof statusIcons] || '?';

  return (
    <span className={`flex items-center gap-1 text-xs ${color}`}>
      <span>{icon}</span>
      <span className="font-medium">{name}</span>
    </span>
  );
}

// Provider component that manages server status state
export function ServerStatusProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<ServerStatusState>({
    mcp: { servers: [], connected: false },
    lsp: { servers: [], connected: false },
    codegraph: {
      initialized: false,
      indexing: false,
      filesIndexed: 0,
      symbolsIndexed: 0,
      pendingFiles: 0,
    },
  });
  const [isLoading, setIsLoading] = useState(false);

  // Refresh server status from backend
  const refreshStatus = async () => {
    setIsLoading(true);
    try {
      // Connect to actual Tauri backend API
      const { invoke } = await import('@tauri-apps/api/core');

      // Fetch LSP status
      const lspServers = await invoke<Array<{name: string; status: string; language?: string; error?: string}>>('get_lsp_status');

      // Fetch CodeGraph status
      const codegraphData = await invoke<{
        initialized: boolean;
        indexing: boolean;
        files_indexed: number;
        symbols_indexed: number;
        edges_indexed: number;
        pending_files: string[];
        last_sync: string;
        error?: string;
      } | null>('get_codegraph_status');

      // MCP status - temporarily use mock data until backend command is added
      const mcpData = {
        servers: [
          { name: 'filesystem', status: 'connected', tools: ['read_file', 'write_file'] },
          { name: 'git', status: 'connected', tools: ['status', 'diff'] },
        ],
        connected: true,
      };

      setStatus({
        mcp: mcpData,
        lsp: {
          servers: lspServers.map(s => ({
            name: s.name,
            status: s.status,
            language: s.language,
          })),
          connected: lspServers.length > 0,
        },
        codegraph: codegraphData ? {
          initialized: codegraphData.initialized,
          indexing: codegraphData.indexing,
          filesIndexed: codegraphData.files_indexed,
          symbolsIndexed: codegraphData.symbols_indexed,
          pendingFiles: codegraphData.pending_files.length,
        } : {
          initialized: false,
          indexing: false,
          filesIndexed: 0,
          symbolsIndexed: 0,
          pendingFiles: 0,
        },
      });
    } catch (error) {
      console.error('Failed to refresh server status:', error);
      // Fallback to disconnected state on error
      setStatus({
        mcp: { servers: [], connected: false },
        lsp: { servers: [], connected: false },
        codegraph: {
          initialized: false,
          indexing: false,
          filesIndexed: 0,
          symbolsIndexed: 0,
          pendingFiles: 0,
        },
      });
    } finally {
      setIsLoading(false);
    }
  };

  // Auto-refresh on mount and every 30 seconds (matching TUI behavior)
  useEffect(() => {
    refreshStatus();
    const interval = setInterval(refreshStatus, 30000);
    return () => clearInterval(interval);
  }, []);

  return (
    <ServerStatusContext.Provider value={{ status, refreshStatus, isLoading }}>
      {children}
    </ServerStatusContext.Provider>
  );
}