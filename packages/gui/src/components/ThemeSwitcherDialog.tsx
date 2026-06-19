import React, { useState, useEffect } from 'react';

// Theme configuration
interface ThemeConfig {
  id: string;
  name: string;
  mode: 'light' | 'dark' | 'system';
  colors: {
    primary: string;
    background: string;
    text: string;
    muted: string;
    accent: string;
  };
}

// Available themes matching TUI color schemes
const AVAILABLE_THEMES: ThemeConfig[] = [
  {
    id: 'dark',
    name: 'Dark (Default)',
    mode: 'dark',
    colors: {
      primary: '#3b82f6',
      background: '#0f172a',
      text: '#e2e8f0',
      muted: '#64748b',
      accent: '#1e293b',
    },
  },
  {
    id: 'matrix',
    name: 'Matrix Green',
    mode: 'dark',
    colors: {
      primary: '#22c55e',
      background: '#0a0a0a',
      text: '#4ade80',
      muted: '#166534',
      accent: '#14532d',
    },
  },
  {
    id: 'ocean',
    name: 'Ocean Blue',
    mode: 'dark',
    colors: {
      primary: '#0ea5e9',
      background: '#0c4a6e',
      text: '#e0f2fe',
      muted: '#0369a1',
      accent: '#075985',
    },
  },
  {
    id: 'sunset',
    name: 'Sunset Orange',
    mode: 'dark',
    colors: {
      primary: '#f97316',
      background: '#1c1917',
      text: '#fed7aa',
      muted: '#78350f',
      accent: '#442407',
    },
  },
  {
    id: 'light',
    name: 'Light',
    mode: 'light',
    colors: {
      primary: '#3b82f6',
      background: '#ffffff',
      text: '#1f2937',
      muted: '#9ca3af',
      accent: '#f3f4f6',
    },
  },
  {
    id: 'system',
    name: 'System',
    mode: 'system',
    colors: {
      primary: '',
      background: '',
      text: '',
      muted: '',
      accent: '',
    },
  },
];

// Get current system preference
function getSystemTheme(): 'light' | 'dark' {
  if (typeof window === 'undefined') return 'dark';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

// Apply theme to document
function applyTheme(theme: ThemeConfig) {
  const root = document.documentElement;

  // Set mode class
  root.classList.remove('light', 'dark');
  const effectiveMode = theme.mode === 'system' ? getSystemTheme() : theme.mode;
  root.classList.add(effectiveMode);

  // Apply custom colors (if not system)
  if (theme.mode !== 'system') {
    root.style.setProperty('--primary', theme.colors.primary);
    root.style.setProperty('--background', theme.colors.background);
    root.style.setProperty('--foreground', theme.colors.text);
    root.style.setProperty('--muted', theme.colors.muted);
    root.style.setProperty('--accent', theme.colors.accent);
  } else {
    // Reset to default
    root.style.removeProperty('--primary');
    root.style.removeProperty('--background');
    root.style.removeProperty('--foreground');
    root.style.removeProperty('--muted');
    root.style.removeProperty('--accent');
  }

  // Store preference
  localStorage.setItem('matrixcode-theme', theme.id);
}

// Get stored theme
function getStoredTheme(): string {
  return localStorage.getItem('matrixcode-theme') || 'dark';
}

interface ThemeSwitcherDialogProps {
  onClose: () => void;
}

export function ThemeSwitcherDialog({ onClose }: ThemeSwitcherDialogProps) {
  const [currentTheme, setCurrentTheme] = useState(getStoredTheme());
  const [previewTheme, setPreviewTheme] = useState<string | null>(null);

  // Load and apply stored theme on mount
  useEffect(() => {
    const stored = getStoredTheme();
    const theme = AVAILABLE_THEMES.find(t => t.id === stored) || AVAILABLE_THEMES[0];
    applyTheme(theme);
  }, []);

  // Preview theme on hover
  const handlePreview = (themeId: string) => {
    setPreviewTheme(themeId);
    const theme = AVAILABLE_THEMES.find(t => t.id === themeId);
    if (theme) applyTheme(theme);
  };

  // Reset preview on mouse leave
  const handleResetPreview = () => {
    setPreviewTheme(null);
    const theme = AVAILABLE_THEMES.find(t => t.id === currentTheme);
    if (theme) applyTheme(theme);
  };

  // Apply theme permanently
  const handleSelect = (themeId: string) => {
    setCurrentTheme(themeId);
    const theme = AVAILABLE_THEMES.find(t => t.id === themeId);
    if (theme) {
      applyTheme(theme);
      onClose();
    }
  };

  // Listen for system theme changes
  useEffect(() => {
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleChange = () => {
      if (currentTheme === 'system') {
        const theme = AVAILABLE_THEMES.find(t => t.id === 'system');
        if (theme) applyTheme(theme);
      }
    };
    mediaQuery.addEventListener('change', handleChange);
    return () => mediaQuery.removeEventListener('change', handleChange);
  }, [currentTheme]);

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-card border shadow-lg rounded-lg max-w-md w-full overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b bg-muted/30">
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-semibold flex items-center gap-2">
              <span>🎨</span>
              <span>Theme Switcher</span>
            </h3>
            <button
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent transition-colors"
            >
              ✕
            </button>
          </div>
          <p className="text-sm text-muted-foreground mt-1">
            选择界面颜色方案
          </p>
        </div>

        {/* Current theme indicator */}
        <div className="px-4 py-2 bg-muted/20 border-b">
          <span className="text-xs text-muted-foreground">
            当前主题: {AVAILABLE_THEMES.find(t => t.id === currentTheme)?.name}
          </span>
        </div>

        {/* Theme grid */}
        <div className="p-4 grid grid-cols-2 gap-3">
          {AVAILABLE_THEMES.map((theme) => (
            <div
              key={theme.id}
              onClick={() => handleSelect(theme.id)}
              onMouseEnter={() => handlePreview(theme.id)}
              onMouseLeave={handleResetPreview}
              className={`p-4 rounded-lg border cursor-pointer transition-all ${
                currentTheme === theme.id
                  ? 'border-primary bg-primary/10'
                  : 'border-border hover:border-primary/50'
              } ${previewTheme === theme.id ? 'ring-2 ring-primary' : ''}`}
            >
              {/* Theme name */}
              <div className="font-medium text-sm mb-2">{theme.name}</div>

              {/* Color preview */}
              {theme.mode !== 'system' && (
                <div className="flex gap-1">
                  <div
                    className="w-4 h-4 rounded"
                    style={{ backgroundColor: theme.colors.primary }}
                    title="Primary"
                  />
                  <div
                    className="w-4 h-4 rounded"
                    style={{ backgroundColor: theme.colors.background }}
                    title="Background"
                  />
                  <div
                    className="w-4 h-4 rounded border"
                    style={{ backgroundColor: theme.colors.text }}
                    title="Text"
                  />
                  <div
                    className="w-4 h-4 rounded"
                    style={{ backgroundColor: theme.colors.muted }}
                    title="Muted"
                  />
                </div>
              )}

              {/* System theme indicator */}
              {theme.mode === 'system' && (
                <div className="flex gap-1">
                  <div className="w-4 h-4 rounded bg-white border" title="Light" />
                  <div className="w-4 h-4 rounded bg-black" title="Dark" />
                  <span className="text-xs text-muted-foreground ml-1">自动</span>
                </div>
              )}

              {/* Current indicator */}
              {currentTheme === theme.id && (
                <span className="text-xs text-primary mt-2">
                  ✓ 当前选择
                </span>
              )}
            </div>
          ))}
        </div>

        {/* Footer */}
        <div className="p-4 border-t bg-muted/30">
          <button
            onClick={onClose}
            className="w-full px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}

// Hook for accessing current theme
export function useTheme() {
  const [theme, setTheme] = useState(getStoredTheme());

  useEffect(() => {
    const handleChange = () => {
      setTheme(getStoredTheme());
    };
    window.addEventListener('storage', handleChange);
    return () => window.removeEventListener('storage', handleChange);
  }, []);

  const changeTheme = (themeId: string) => {
    const themeConfig = AVAILABLE_THEMES.find(t => t.id === themeId);
    if (themeConfig) {
      applyTheme(themeConfig);
      setTheme(themeId);
    }
  };

  return { theme, changeTheme, themes: AVAILABLE_THEMES };
}