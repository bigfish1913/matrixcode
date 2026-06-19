import React from 'react';
import { useChatStore } from '../stores/chatStore';

interface TokenStatsPanelProps {
  onClose?: () => void;
}

// Format token count
function formatTokens(count: number): string {
  if (count >= 1000000) return `${(count / 1000000).toFixed(2)}M`;
  if (count >= 1000) return `${(count / 1000).toFixed(1)}k`;
  return String(count);
}

// Calculate costs (approximate pricing)
function calculateCost(input: number, output: number, model?: string): { inputCost: number; outputCost: number; totalCost: number } {
  // Approximate pricing per 1M tokens (Claude Sonnet 4)
  const pricing = {
    'claude-sonnet-4-20250514': { input: 3.00, output: 15.00 },
    'claude-opus-4-8': { input: 15.00, output: 75.00 },
    'claude-haiku-4-5-20251001': { input: 0.80, output: 4.00 },
    'claude-fable-5': { input: 3.00, output: 15.00 },
    'gpt-4o': { input: 2.50, output: 10.00 },
    'gpt-4-turbo': { input: 10.00, output: 30.00 },
    'gpt-3.5-turbo': { input: 0.50, output: 1.50 },
    'gemini-1.5-pro': { input: 1.25, output: 5.00 },
    'gemini-1.5-flash': { input: 0.075, output: 0.30 },
  };

  // Default pricing (Claude Sonnet)
  const defaultPricing = { input: 3.00, output: 15.00 };
  const p = pricing[model as keyof typeof pricing] || defaultPricing;

  const inputCost = (input / 1000000) * p.input;
  const outputCost = (output / 1000000) * p.output;
  const totalCost = inputCost + outputCost;

  return { inputCost, outputCost, totalCost };
}

// Token usage bar with segments
function TokenUsageBar({
  input,
  output,
  cacheRead,
  cacheCreated,
  maxContext = 200000,
}: {
  input: number;
  output: number;
  cacheRead?: number;
  cacheCreated?: number;
  maxContext?: number;
}) {
  const total = input + output;
  const ratio = Math.min(total / maxContext, 1);

  // Calculate percentages for segments
  const inputPct = input / total;
  const outputPct = output / total;
  const cacheReadPct = cacheRead ? cacheRead / (cacheRead + input) : 0;

  return (
    <div className="space-y-2">
      {/* Main usage bar */}
      <div className="h-4 bg-muted rounded-full overflow-hidden relative">
        {/* Input segment */}
        <div
          className="absolute left-0 h-full bg-blue-500"
          style={{ width: `${ratio * inputPct * 100}%` }}
        />
        {/* Output segment */}
        <div
          className="absolute h-full bg-green-500"
          style={{ left: `${ratio * inputPct * 100}%`, width: `${ratio * outputPct * 100}%` }}
        />
        {/* Cache read indicator */}
        {cacheRead && cacheRead > 0 && (
          <div
            className="absolute top-0 h-full bg-yellow-500/50"
            style={{ width: `${cacheReadPct * 100}%` }}
          />
        )}
      </div>

      {/* Legend */}
      <div className="flex gap-4 text-xs">
        <span className="flex items-center gap-1">
          <span className="w-2 h-2 rounded bg-blue-500" />
          <span>Input</span>
        </span>
        <span className="flex items-center gap-1">
          <span className="w-2 h-2 rounded bg-green-500" />
          <span>Output</span>
        </span>
        {cacheRead && cacheRead > 0 && (
          <span className="flex items-center gap-1">
            <span className="w-2 h-2 rounded bg-yellow-500" />
            <span>Cache</span>
          </span>
        )}
        <span className="ml-auto text-muted-foreground">
          {Math.round(ratio * 100)}% of {formatTokens(maxContext)} context
        </span>
      </div>
    </div>
  );
}

export function TokenStatsPanel({ onClose }: TokenStatsPanelProps) {
  const inputTokens = useChatStore((s) => s.inputTokens);
  const outputTokens = useChatStore((s) => s.outputTokens);
  const cacheReadTokens = useChatStore((s) => s.cacheReadTokens);
  const cacheCreationTokens = useChatStore((s) => s.cacheCreationTokens);
  const apiCalls = useChatStore((s) => s.apiCalls);
  const toolCalls = useChatStore((s) => s.toolCalls);
  const compressions = useChatStore((s) => s.compressions);
  const messages = useChatStore((s) => s.messages);

  // Calculate statistics
  const totalInput = inputTokens;
  const totalOutput = outputTokens;
  const totalTokens = totalInput + totalOutput;
  const totalCacheRead = cacheReadTokens;
  const totalCacheCreated = cacheCreationTokens;

  // Message statistics
  const userMessages = messages.filter(m => m.role === 'user').length;
  const assistantMessages = messages.filter(m => m.role === 'assistant').length;
  const toolMessages = messages.filter(m => m.role === 'tool').length;
  const errorMessages = messages.filter(m => m.role === 'error').length;

  // Efficiency metrics
  const avgTokensPerMessage = messages.length > 0 ? Math.round(totalTokens / messages.length) : 0;
  const avgInputPerCall = apiCalls > 0 ? Math.round(totalInput / apiCalls) : 0;
  const avgOutputPerCall = apiCalls > 0 ? Math.round(totalOutput / apiCalls) : 0;

  // Cache efficiency
  const cacheEfficiency = totalInput > 0 ? Math.round((totalCacheRead / totalInput) * 100) : 0;

  // Calculate costs (using default model for now)
  const costs = calculateCost(totalInput, totalOutput);

  return (
    <div className="bg-card border rounded-lg p-4 mb-3">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h3 className="font-semibold text-sm flex items-center gap-2">
          <span>📊</span>
          <span>Token Statistics</span>
        </h3>
        {onClose && (
          <button
            onClick={onClose}
            className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent transition-colors"
          >
            ✕
          </button>
        )}
      </div>

      {/* Usage bar */}
      <TokenUsageBar
        input={totalInput}
        output={totalOutput}
        cacheRead={totalCacheRead}
        cacheCreated={totalCacheCreated}
      />

      {/* Token counts */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mt-4">
        <div className="p-3 bg-muted/30 rounded">
          <div className="text-xs text-muted-foreground">Input Tokens</div>
          <div className="text-xl font-bold text-blue-500">{formatTokens(totalInput)}</div>
          {totalCacheRead > 0 && (
            <div className="text-xs text-yellow-500 mt-1">
              ⚡ {formatTokens(totalCacheRead)} cached
            </div>
          )}
        </div>

        <div className="p-3 bg-muted/30 rounded">
          <div className="text-xs text-muted-foreground">Output Tokens</div>
          <div className="text-xl font-bold text-green-500">{formatTokens(totalOutput)}</div>
        </div>

        <div className="p-3 bg-muted/30 rounded">
          <div className="text-xs text-muted-foreground">Total Tokens</div>
          <div className="text-xl font-bold">{formatTokens(totalTokens)}</div>
        </div>

        <div className="p-3 bg-muted/30 rounded">
          <div className="text-xs text-muted-foreground">Cache Created</div>
          <div className="text-xl font-bold text-purple-500">{formatTokens(totalCacheCreated)}</div>
        </div>
      </div>

      {/* Efficiency metrics */}
      <div className="mt-4 space-y-2">
        <div className="text-xs font-medium text-muted-foreground">效率指标</div>
        <div className="grid grid-cols-2 md:grid-cols-3 gap-2 text-xs">
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground">平均/消息:</span>
            <span className="font-mono">{avgTokensPerMessage} tokens</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground">平均输入/调用:</span>
            <span className="font-mono">{avgInputPerCall} tokens</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground">平均输出/调用:</span>
            <span className="font-mono">{avgOutputPerCall} tokens</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground">缓存效率:</span>
            <span className={`font-mono ${cacheEfficiency > 50 ? 'text-green-500' : ''}`}>
              {cacheEfficiency}%
            </span>
          </div>
        </div>
      </div>

      {/* Call statistics */}
      <div className="mt-4 space-y-2">
        <div className="text-xs font-medium text-muted-foreground">调用统计</div>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-2 text-xs">
          <div className="flex items-center gap-2">
            <span>📡</span>
            <span className="text-muted-foreground">API Calls:</span>
            <span className="font-mono">{apiCalls}</span>
          </div>
          <div className="flex items-center gap-2">
            <span>🔧</span>
            <span className="text-muted-foreground">Tool Calls:</span>
            <span className="font-mono">{toolCalls}</span>
          </div>
          <div className="flex items-center gap-2">
            <span>📦</span>
            <span className="text-muted-foreground">Compressions:</span>
            <span className="font-mono">{compressions}</span>
          </div>
        </div>
      </div>

      {/* Message statistics */}
      <div className="mt-4 space-y-2">
        <div className="text-xs font-medium text-muted-foreground">消息统计</div>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-2 text-xs">
          <div className="flex items-center gap-2">
            <span>👤</span>
            <span className="text-muted-foreground">User:</span>
            <span className="font-mono">{userMessages}</span>
          </div>
          <div className="flex items-center gap-2">
            <span>🤖</span>
            <span className="text-muted-foreground">Assistant:</span>
            <span className="font-mono">{assistantMessages}</span>
          </div>
          <div className="flex items-center gap-2">
            <span>🔧</span>
            <span className="text-muted-foreground">Tool:</span>
            <span className="font-mono">{toolMessages}</span>
          </div>
          {errorMessages > 0 && (
            <div className="flex items-center gap-2">
              <span>❌</span>
              <span className="text-muted-foreground">Error:</span>
              <span className="font-mono text-red-500">{errorMessages}</span>
            </div>
          )}
        </div>
      </div>

      {/* Cost estimate */}
      <div className="mt-4 p-3 bg-muted/30 rounded">
        <div className="text-xs font-medium text-muted-foreground mb-2">成本估算 (近似)</div>
        <div className="grid grid-cols-3 gap-2 text-xs">
          <div>
            <span className="text-muted-foreground">输入成本:</span>
            <span className="font-mono ml-1">${costs.inputCost.toFixed(4)}</span>
          </div>
          <div>
            <span className="text-muted-foreground">输出成本:</span>
            <span className="font-mono ml-1">${costs.outputCost.toFixed(4)}</span>
          </div>
          <div>
            <span className="text-muted-foreground">总成本:</span>
            <span className="font-mono ml-1 font-bold">${costs.totalCost.toFixed(4)}</span>
          </div>
        </div>
        <div className="text-xs text-muted-foreground mt-2">
          基于 Claude Sonnet 4 定价 ($3/$15 per 1M tokens)
        </div>
      </div>
    </div>
  );
}