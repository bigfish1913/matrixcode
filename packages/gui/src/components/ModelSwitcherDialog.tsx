import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useConfigStore } from '../stores/configStore';

interface ModelInfo {
  id: string;
  name: string;
  provider: string;
  tier?: 'fast' | 'standard' | 'advanced';
  pricing?: {
    input: number;
    output: number;
  };
  max_tokens?: number;
  supports_vision?: boolean;
  supports_tools?: boolean;
}

interface ModelSwitcherDialogProps {
  onClose: () => void;
}

// Model tiers with colors (matching Claude models)
const MODEL_TIERS: Record<string, { label: string; color: string; desc: string }> = {
  fast: { label: 'Fast', color: 'text-blue-500', desc: '快速响应，适合简单任务' },
  standard: { label: 'Standard', color: 'text-green-500', desc: '平衡性能与成本' },
  advanced: { label: 'Advanced', color: 'text-purple-500', desc: '最强能力，复杂任务' },
};

// Provider icons
const PROVIDER_ICONS: Record<string, string> = {
  anthropic: '🤖',
  openai: '🔷',
  google: '🔶',
  local: '💻',
};

// Popular models list (hardcoded as fallback)
const POPULAR_MODELS: ModelInfo[] = [
  // Anthropic Claude models
  { id: 'claude-sonnet-4-20250514', name: 'Claude Sonnet 4', provider: 'anthropic', tier: 'standard', supports_vision: true, supports_tools: true },
  { id: 'claude-opus-4-8', name: 'Claude Opus 4.8', provider: 'anthropic', tier: 'advanced', supports_vision: true, supports_tools: true },
  { id: 'claude-haiku-4-5-20251001', name: 'Claude Haiku 4.5', provider: 'anthropic', tier: 'fast', supports_vision: true, supports_tools: true },
  { id: 'claude-fable-5', name: 'Claude Fable 5', provider: 'anthropic', tier: 'advanced', supports_vision: true, supports_tools: true },
  // OpenAI models
  { id: 'gpt-4o', name: 'GPT-4o', provider: 'openai', tier: 'advanced', supports_vision: true, supports_tools: true },
  { id: 'gpt-4-turbo', name: 'GPT-4 Turbo', provider: 'openai', tier: 'standard', supports_vision: true, supports_tools: true },
  { id: 'gpt-3.5-turbo', name: 'GPT-3.5 Turbo', provider: 'openai', tier: 'fast', supports_tools: true },
  // Google models
  { id: 'gemini-1.5-pro', name: 'Gemini 1.5 Pro', provider: 'google', tier: 'advanced', supports_vision: true, supports_tools: true },
  { id: 'gemini-1.5-flash', name: 'Gemini 1.5 Flash', provider: 'google', tier: 'fast', supports_vision: true, supports_tools: true },
];

export function ModelSwitcherDialog({ onClose }: ModelSwitcherDialogProps) {
  const [models, setModels] = useState<ModelInfo[]>(POPULAR_MODELS);
  const [loading, setLoading] = useState(false);
  const [selectedModel, setSelectedModel] = useState<string | null>(null);
  const [filter, setFilter] = useState<string>('all');

  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const currentModel = config?.model || '';

  // Load available models from backend (optional)
  useEffect(() => {
    const loadModels = async () => {
      try {
        setLoading(true);
        const backendModels = await invoke<ModelInfo[]>('get_available_models');
        if (backendModels && backendModels.length > 0) {
          setModels(backendModels);
        }
      } catch (e) {
        // Use hardcoded models as fallback
        console.log('Using fallback model list');
      } finally {
        setLoading(false);
      }
    };
    loadModels();
  }, []);

  // Handle model selection
  const handleSelectModel = async (modelId: string) => {
    setSelectedModel(modelId);

    try {
      await updateConfig({ model: modelId });
      console.log(`Model changed to: ${modelId}`);
      onClose();
    } catch (e) {
      console.error('Failed to update model:', e);
    }
  };

  // Filter models
  const filteredModels = models.filter(m => {
    if (filter === 'all') return true;
    if (filter === 'vision') return m.supports_vision;
    if (filter === 'tools') return m.supports_tools;
    return m.provider === filter;
  });

  // Group by provider
  const groupedModels = filteredModels.reduce((acc, model) => {
    const provider = model.provider;
    if (!acc[provider]) acc[provider] = [];
    acc[provider].push(model);
    return acc;
  }, {} as Record<string, ModelInfo[]>);

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-card border shadow-lg rounded-lg max-w-lg w-full max-h-[80vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="p-4 border-b bg-muted/30">
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-semibold flex items-center gap-2">
              <span>🤖</span>
              <span>模型切换</span>
            </h3>
            <button
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent transition-colors"
            >
              ✕
            </button>
          </div>
          <p className="text-sm text-muted-foreground mt-1">
            选择 AI 模型进行对话
          </p>
        </div>

        {/* Current model indicator */}
        <div className="px-4 py-2 bg-muted/20 border-b flex items-center gap-2">
          <span className="text-xs text-muted-foreground">当前模型:</span>
          <span className="px-2 py-0.5 bg-primary/10 text-primary rounded font-mono text-sm">
            {currentModel}
          </span>
        </div>

        {/* Filter tabs */}
        <div className="px-4 py-2 border-b flex gap-2 text-xs">
          <button
            onClick={() => setFilter('all')}
            className={`px-2 py-1 rounded ${filter === 'all' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-accent'}`}
          >
            全部
          </button>
          <button
            onClick={() => setFilter('anthropic')}
            className={`px-2 py-1 rounded ${filter === 'anthropic' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-accent'}`}
          >
            Anthropic
          </button>
          <button
            onClick={() => setFilter('openai')}
            className={`px-2 py-1 rounded ${filter === 'openai' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-accent'}`}
          >
            OpenAI
          </button>
          <button
            onClick={() => setFilter('vision')}
            className={`px-2 py-1 rounded ${filter === 'vision' ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-accent'}`}
          >
            🖼 Vision
          </button>
        </div>

        {/* Models list */}
        <div className="flex-1 overflow-y-auto p-4">
          {loading ? (
            <div className="text-center text-muted-foreground py-8">
              <div className="animate-spin text-2xl mb-2">⏳</div>
              <span className="text-sm">加载模型列表...</span>
            </div>
          ) : (
            Object.entries(groupedModels).map(([provider, providerModels]) => (
              <div key={provider} className="mb-4">
                {/* Provider header */}
                <div className="flex items-center gap-2 mb-2 text-sm font-medium text-muted-foreground">
                  <span>{PROVIDER_ICONS[provider] || '🔧'}</span>
                  <span className="capitalize">{provider}</span>
                  <span className="text-xs">({providerModels.length} models)</span>
                </div>

                {/* Models */}
                <div className="space-y-2">
                  {providerModels.map((model) => (
                    <button
                      key={model.id}
                      onClick={() => handleSelectModel(model.id)}
                      disabled={selectedModel !== null}
                      className={`w-full p-3 rounded-lg border transition-all ${
                        currentModel === model.id
                          ? 'border-primary bg-primary/10'
                          : 'border-border hover:border-primary/50 hover:bg-accent/30'
                      } ${selectedModel === model.id ? 'animate-pulse' : ''}`}
                    >
                      <div className="flex items-start gap-3">
                        {/* Tier indicator */}
                        {model.tier && (
                          <span className={`px-1.5 py-0.5 rounded text-xs ${MODEL_TIERS[model.tier]?.color}`}>
                            {MODEL_TIERS[model.tier]?.label}
                          </span>
                        )}

                        {/* Model info */}
                        <div className="flex-1">
                          <div className="flex items-center gap-2">
                            <span className="font-medium">{model.name}</span>
                            {currentModel === model.id && (
                              <span className="px-1.5 py-0.5 bg-primary text-primary-foreground text-xs rounded">
                                当前
                              </span>
                            )}
                          </div>

                          {/* Model ID */}
                          <span className="text-xs text-muted-foreground font-mono block mt-1">
                            {model.id}
                          </span>

                          {/* Capabilities */}
                          <div className="flex gap-2 mt-2">
                            {model.supports_vision && (
                              <span className="text-xs text-blue-500 flex items-center gap-0.5">
                                <span>🖼</span>
                                <span>Vision</span>
                              </span>
                            )}
                            {model.supports_tools && (
                              <span className="text-xs text-green-500 flex items-center gap-0.5">
                                <span>🔧</span>
                                <span>Tools</span>
                              </span>
                            )}
                            {model.max_tokens && (
                              <span className="text-xs text-muted-foreground">
                                {model.max_tokens > 100000 ? `${Math.floor(model.max_tokens / 1000)}k` : model.max_tokens} tokens
                              </span>
                            )}
                          </div>

                          {/* Tier description */}
                          {model.tier && (
                            <p className="text-xs text-muted-foreground mt-1">
                              {MODEL_TIERS[model.tier]?.desc}
                            </p>
                          )}
                        </div>
                      </div>
                    </button>
                  ))}
                </div>
              </div>
            ))
          )}
        </div>

        {/* Footer */}
        <div className="p-4 border-t bg-muted/30">
          <div className="flex gap-2">
            <button
              onClick={onClose}
              className="flex-1 px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors"
            >
              取消
            </button>
            <div className="flex-1 flex items-center justify-center text-xs text-muted-foreground">
              <kbd className="px-1.5 py-0.5 bg-muted rounded">/model</kbd>
              <span className="ml-1">快捷命令</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}