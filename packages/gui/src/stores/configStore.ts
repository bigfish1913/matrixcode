import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

// SafeConfig from backend - API key is masked (api_key_set indicates if key is configured)
export interface MatrixConfig {
  provider: string | null;
  api_key_set: boolean;  // Changed: backend now returns boolean instead of actual key
  base_url: string | null;
  model: string | null;
  think: boolean;
  markdown: boolean;
  max_tokens: number;
  context_size: number | null;
  multi_model: boolean | null;
  plan_model: string | null;
  compress_model: string | null;
  fast_model: string | null;
  approve_mode: string | null;
  enable_lsp: boolean;
  verify_strategy: string | null;
  cli_path: string | null;  // CLI binary path (matching VSCode extension matrixcode.cliPath)
}

interface ConfigState {
  config: MatrixConfig | null;
  projectPath: string | null;
  loading: boolean;

  loadConfig: () => Promise<void>;
  updateConfig: (updates: Record<string, unknown>) => Promise<void>;
  setProjectPath: (path: string) => Promise<void>;
}

export const useConfigStore = create<ConfigState>((set, get) => ({
  config: null,
  projectPath: null,
  loading: false,

  loadConfig: async () => {
    set({ loading: true });
    try {
      const config = await invoke<MatrixConfig>('get_config');
      set({ config });
    } finally {
      set({ loading: false });
    }
  },

  updateConfig: async (updates: Record<string, unknown>) => {
    await invoke('update_config', { updates });
    // Refresh the config from backend after update
    const config = await invoke<MatrixConfig>('get_config');
    set({ config });
  },

  setProjectPath: async (path: string) => {
    await invoke('set_project_path', { path });
    set({ projectPath: path });
  },
}));