import React, { useState, useEffect } from 'react';
import { useConfigStore } from '../stores/configStore';

// Configuration categories matching TUI settings
interface ConfigCategory {
  id: string;
  name: string;
  icon: string;
  description: string;
}

const CONFIG_CATEGORIES: ConfigCategory[] = [
  { id: 'provider', name: 'API Provider', icon: '📡', description: 'API 配置和认证' },
  { id: 'model', name: 'Model Settings', icon: '🤖', description: '模型选择和行为' },
  { id: 'behavior', name: 'Behavior', icon: '⚙️', description: 'Agent 行为设置' },
  { id: 'ui', name: 'UI Settings', icon: '🎨', description: '界面和显示设置' },
  { id: 'shortcuts', name: 'Shortcuts', icon: '⌨️', description: '快捷键配置' },
  { id: 'advanced', name: 'Advanced', icon: '🔧', description: '高级设置' },
];

// Configuration settings structure
interface ConfigSetting {
  id: string;
  category: string;
  name: string;
  type: 'string' | 'number' | 'boolean' | 'select' | 'array';
  value: any;
  defaultValue: any;
  options?: { value: string; label: string }[];
  description?: string;
  placeholder?: string;
  min?: number;
  max?: number;
  sensitive?: boolean;  // e.g., API key - hide by default
  experimental?: boolean;
}

// Load settings from localStorage
function loadSettings(): Record<string, any> {
  try {
    const stored = localStorage.getItem('matrixcode-settings');
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (e) {
    console.error('Failed to load settings:', e);
  }
  return {};
}

// Save settings to localStorage
function saveSettings(settings: Record<string, any>): void {
  try {
    localStorage.setItem('matrixcode-settings', JSON.stringify(settings));
  } catch (e) {
    console.error('Failed to save settings:', e);
  }
}

// Settings center dialog
interface SettingsCenterDialogProps {
  onClose: () => void;
}

export function SettingsCenterDialog({ onClose }: SettingsCenterDialogProps) {
  const [activeCategory, setActiveCategory] = useState<string>('provider');
  const [settings, setSettings] = useState<Record<string, any>>(loadSettings());
  const [searchQuery, setSearchQuery] = useState('');
  const [hasChanges, setHasChanges] = useState(false);
  const [showSensitive, setShowSensitive] = useState(false);

  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);

  // Default settings based on current config
  const defaultSettings: ConfigSetting[] = [
    // Provider settings
    { id: 'provider', category: 'provider', name: 'Provider', type: 'select', value: config?.provider || 'anthropic', defaultValue: 'anthropic', options: [
      { value: 'anthropic', label: 'Anthropic (Claude)' },
      { value: 'openai', label: 'OpenAI (GPT)' },
      { value: 'google', label: 'Google (Gemini)' },
    ]},
    { id: 'api_key', category: 'provider', name: 'API Key', type: 'string', value: '', defaultValue: '', sensitive: true, placeholder: 'Enter your API key' },
    { id: 'base_url', category: 'provider', name: 'Base URL', type: 'string', value: config?.base_url || '', defaultValue: '', placeholder: 'Custom API endpoint' },

    // Model settings
    { id: 'model', category: 'model', name: 'Model', type: 'select', value: config?.model || 'claude-sonnet-4-20250514', defaultValue: 'claude-sonnet-4-20250514', options: [
      { value: 'claude-sonnet-4-20250514', label: 'Claude Sonnet 4' },
      { value: 'claude-opus-4-8', label: 'Claude Opus 4.8' },
      { value: 'claude-haiku-4-5-20251001', label: 'Claude Haiku 4.5' },
      { value: 'claude-fable-5', label: 'Claude Fable 5' },
      { value: 'gpt-4o', label: 'GPT-4o' },
      { value: 'gpt-4-turbo', label: 'GPT-4 Turbo' },
      { value: 'gemini-1.5-pro', label: 'Gemini 1.5 Pro' },
    ]},
    { id: 'max_tokens', category: 'model', name: 'Max Tokens', type: 'number', value: config?.max_tokens || 4096, defaultValue: 4096, min: 256, max: 8192, description: 'Maximum response length' },
    { id: 'temperature', category: 'model', name: 'Temperature', type: 'number', value: 1.0, defaultValue: 1.0, min: 0, max: 2, description: 'Response randomness (0-2)' },
    { id: 'think', category: 'model', name: 'Extended Thinking', type: 'boolean', value: config?.think ?? true, defaultValue: true, description: 'Enable extended thinking mode' },

    // Behavior settings
    { id: 'approve_mode', category: 'behavior', name: 'Approve Mode', type: 'select', value: config?.approve_mode || 'auto', defaultValue: 'auto', options: [
      { value: 'ask', label: 'Ask - 每次操作确认' },
      { value: 'auto', label: 'Auto - 自动执行' },
      { value: 'strict', label: 'Strict - 严格确认' },
    ]},
    { id: 'enable_lsp', category: 'behavior', name: 'Enable LSP', type: 'boolean', value: config?.enable_lsp ?? false, defaultValue: false, description: 'Enable Language Server Protocol' },
    { id: 'auto_save', category: 'behavior', name: 'Auto Save', type: 'boolean', value: true, defaultValue: true, description: 'Automatically save sessions' },
    { id: 'auto_compact', category: 'behavior', name: 'Auto Compact', type: 'boolean', value: true, defaultValue: true, description: 'Auto compact context when too long' },

    // UI settings
    { id: 'theme', category: 'ui', name: 'Theme', type: 'select', value: 'dark', defaultValue: 'dark', options: [
      { value: 'dark', label: 'Dark (Default)' },
      { value: 'matrix', label: 'Matrix Green' },
      { value: 'ocean', label: 'Ocean Blue' },
      { value: 'sunset', label: 'Sunset Orange' },
      { value: 'light', label: 'Light' },
      { value: 'system', label: 'System' },
    ]},
    { id: 'font_size', category: 'ui', name: 'Font Size', type: 'select', value: '14', defaultValue: '14', options: [
      { value: '12', label: 'Small (12px)' },
      { value: '14', label: 'Medium (14px)' },
      { value: '16', label: 'Large (16px)' },
      { value: '18', label: 'Extra Large (18px)' },
    ]},
    { id: 'show_timestamps', category: 'ui', name: 'Show Timestamps', type: 'boolean', value: true, defaultValue: true, description: 'Show message timestamps' },
    { id: 'show_tokens', category: 'ui', name: 'Show Token Count', type: 'boolean', value: true, defaultValue: true, description: 'Show token usage in status bar' },
    { id: 'compact_mode', category: 'ui', name: 'Compact Mode', type: 'boolean', value: false, defaultValue: false, description: 'Use compact UI layout' },

    // Shortcuts settings (placeholder - handled by KeybindingEditor)
    { id: 'shortcuts_enabled', category: 'shortcuts', name: 'Enable Custom Shortcuts', type: 'boolean', value: true, defaultValue: true, description: 'Use custom keyboard shortcuts' },

    // Advanced settings
    { id: 'debug_mode', category: 'advanced', name: 'Debug Mode', type: 'boolean', value: false, defaultValue: false, description: 'Show debug information' },
    { id: 'experimental_features', category: 'advanced', name: 'Experimental Features', type: 'boolean', value: false, defaultValue: false, description: 'Enable experimental features', experimental: true },
    { id: 'context_window', category: 'advanced', name: 'Context Window', type: 'number', value: 200000, defaultValue: 200000, min: 10000, max: 500000, description: 'Maximum context window size' },
    { id: 'cache_enabled', category: 'advanced', name: 'Enable Caching', type: 'boolean', value: true, defaultValue: true, description: 'Cache responses for efficiency' },
  ];

  // Filter settings by category and search
  const filteredSettings = defaultSettings.filter(setting => {
    if (setting.category !== activeCategory) return false;
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      return setting.name.toLowerCase().includes(query) ||
             (setting.description?.toLowerCase().includes(query));
    }
    return true;
  });

  // Update setting value
  const updateSetting = (id: string, value: any) => {
    const newSettings = { ...settings, [id]: value };
    setSettings(newSettings);
    setHasChanges(true);
    saveSettings(newSettings);
  };

  // Apply changes to config
  const applyChanges = async () => {
    // Map settings to config
    const configUpdates: Record<string, any> = {};

    if (settings.provider) configUpdates.provider = settings.provider;
    if (settings.model) configUpdates.model = settings.model;
    if (settings.max_tokens) configUpdates.max_tokens = settings.max_tokens;
    if (settings.think !== undefined) configUpdates.think = settings.think;
    if (settings.approve_mode) configUpdates.approve_mode = settings.approve_mode;
    if (settings.enable_lsp !== undefined) configUpdates.enable_lsp = settings.enable_lsp;

    try {
      await updateConfig(configUpdates);
      setHasChanges(false);
      // Show success notification
      console.log('Settings saved successfully');
    } catch (e) {
      console.error('Failed to apply settings:', e);
    }
  };

  // Reset to defaults
  const resetToDefaults = () => {
    const defaults: Record<string, any> = {};
    defaultSettings.forEach(setting => {
      defaults[setting.id] = setting.defaultValue;
    });
    setSettings(defaults);
    setHasChanges(true);
    saveSettings(defaults);
  };

  // Setting input component
  const SettingInput = ({ setting }: { setting: ConfigSetting }) => {
    const value = settings[setting.id] ?? setting.value;

    // Sensitive field (hide by default)
    if (setting.sensitive && !showSensitive) {
      return (
        <div className="flex items-center gap-2">
          <span className="text-muted-foreground">••••••••</span>
          <button
            onClick={() => setShowSensitive(true)}
            className="text-xs text-primary hover:underline"
          >
            Show
          </button>
        </div>
      );
    }

    switch (setting.type) {
      case 'boolean':
        return (
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={value}
              onChange={(e) => updateSetting(setting.id, e.target.checked)}
              className="w-4 h-4"
            />
            <span className={`text-sm ${value ? 'text-primary' : 'text-muted-foreground'}`}>
              {value ? 'Enabled' : 'Disabled'}
            </span>
          </div>
        );

      case 'select':
        return (
          <select
            value={value}
            onChange={(e) => updateSetting(setting.id, e.target.value)}
            className="w-full px-3 py-2 bg-muted rounded text-sm outline-none focus:ring-2 focus:ring-primary"
          >
            {setting.options?.map(opt => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        );

      case 'number':
        return (
          <input
            type="number"
            value={value}
            onChange={(e) => updateSetting(setting.id, parseInt(e.target.value))}
            min={setting.min}
            max={setting.max}
            className="w-full px-3 py-2 bg-muted rounded text-sm outline-none focus:ring-2 focus:ring-primary"
          />
        );

      case 'string':
        return (
          <input
            type={setting.sensitive ? 'password' : 'text'}
            value={value}
            onChange={(e) => updateSetting(setting.id, e.target.value)}
            placeholder={setting.placeholder}
            className="w-full px-3 py-2 bg-muted rounded text-sm outline-none focus:ring-2 focus:ring-primary"
          />
        );

      default:
        return <span>{value}</span>;
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-card border shadow-lg rounded-lg max-w-2xl w-full max-h-[80vh] overflow-hidden flex">
        {/* Sidebar - Categories */}
        <div className="w-48 bg-muted/30 border-r">
          <div className="p-3 border-b">
            <h3 className="font-semibold text-sm">Settings</h3>
          </div>
          <div className="p-2">
            {CONFIG_CATEGORIES.map(cat => (
              <button
                key={cat.id}
                onClick={() => setActiveCategory(cat.id)}
                className={`w-full p-2 rounded text-left flex items-center gap-2 transition-colors ${
                  activeCategory === cat.id ? 'bg-primary/10 text-primary' : 'hover:bg-accent/30'
                }`}
              >
                <span>{cat.icon}</span>
                <span className="text-sm">{cat.name}</span>
              </button>
            ))}
          </div>
        </div>

        {/* Main content */}
        <div className="flex-1 overflow-hidden flex flex-col">
          {/* Header */}
          <div className="p-4 border-b">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <span className="text-xl">
                  {CONFIG_CATEGORIES.find(c => c.id === activeCategory)?.icon}
                </span>
                <h3 className="font-semibold">
                  {CONFIG_CATEGORIES.find(c => c.id === activeCategory)?.name}
                </h3>
              </div>
              <button
                onClick={onClose}
                className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent transition-colors"
              >
                ✕
              </button>
            </div>
            <p className="text-sm text-muted-foreground mt-1">
              {CONFIG_CATEGORIES.find(c => c.id === activeCategory)?.description}
            </p>
          </div>

          {/* Search */}
          <div className="px-4 py-2 border-b">
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="搜索设置..."
              className="w-full bg-muted rounded px-3 py-1.5 text-sm outline-none focus:ring-2 focus:ring-primary"
            />
          </div>

          {/* Settings list */}
          <div className="flex-1 overflow-y-auto p-4">
            <div className="space-y-4">
              {filteredSettings.map(setting => (
                <div
                  key={setting.id}
                  className="p-3 bg-muted/30 rounded-lg"
                >
                  {/* Setting name */}
                  <div className="flex items-center gap-2 mb-2">
                    <span className="font-medium text-sm">{setting.name}</span>
                    {setting.experimental && (
                      <span className="px-1.5 py-0.5 bg-purple-500/20 text-purple-500 rounded text-xs">
                        Experimental
                      </span>
                    )}
                  </div>

                  {/* Setting input */}
                  <SettingInput setting={setting} />

                  {/* Description */}
                  {setting.description && (
                    <p className="text-xs text-muted-foreground mt-2">
                      {setting.description}
                    </p>
                  )}
                </div>
              ))}
            </div>
          </div>

          {/* Footer */}
          <div className="p-4 border-t bg-muted/30">
            <div className="flex gap-2">
              {hasChanges && (
                <button
                  onClick={applyChanges}
                  className="px-4 py-2 bg-primary text-primary-foreground rounded-lg text-sm hover:bg-primary/90 transition-colors"
                >
                  Apply Changes
                </button>
              )}
              <button
                onClick={resetToDefaults}
                className="px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors"
              >
                Reset to Defaults
              </button>
              <button
                onClick={onClose}
                className="flex-1 px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors"
              >
                Close
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}