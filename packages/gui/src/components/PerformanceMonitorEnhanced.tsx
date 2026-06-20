/**
 * Performance monitoring component
 * Tracks and displays real-time performance metrics
 */

import React, { useState, useEffect } from 'react';

interface PerformanceMetrics {
  messageCount: number;
  renderTime: number;
  memoryUsage?: number;
  componentCount: number;
}

interface PerformanceMonitorProps {
  minMessages?: number; // Minimum messages to show monitor (default: 20)
}

export function PerformanceMonitor({ minMessages = 20 }: PerformanceMonitorProps) {
  const [metrics, setMetrics] = useState<PerformanceMetrics>({
    messageCount: 0,
    renderTime: 0,
    componentCount: 0,
  });
  const [showDetails, setShowDetails] = useState(false);

  useEffect(() => {
    // Update metrics every 2 seconds
    const interval = setInterval(() => {
      // Get performance metrics
      const perfEntries = performance.getEntriesByType('measure');
      const lastRender = perfEntries.length > 0
        ? perfEntries[perfEntries.length - 1].duration
        : 0;

      // Estimate component count (React doesn't provide this directly)
      const estimatedComponents = document.querySelectorAll('[data-reactroot], [class*="react"]').length;

      setMetrics({
        messageCount: 0, // Will be updated by parent
        renderTime: lastRender,
        memoryUsage: (performance as any).memory?.usedJSHeapSize,
        componentCount: estimatedComponents,
      });
    }, 2000);

    return () => clearInterval(interval);
  }, []);

  const formatBytes = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatTime = (ms: number): string => {
    if (ms < 1) return `${(ms * 1000).toFixed(0)}μs`;
    if (ms < 1000) return `${ms.toFixed(1)}ms`;
    return `${(ms / 1000).toFixed(2)}s`;
  };

  // Don't show if metrics are below threshold
  if (metrics.messageCount < minMessages) return null;

  return (
    <div className="bg-card border rounded-lg p-3 shadow-lg">
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2">
          <span className="text-lg">📊</span>
          <span className="font-semibold text-sm">Performance</span>
        </div>
        <button
          onClick={() => setShowDetails(!showDetails)}
          className="text-xs text-muted-foreground hover:text-foreground"
          aria-label={showDetails ? 'Hide details' : 'Show details'}
        >
          {showDetails ? '▼ Hide' : '▶ Details'}
        </button>
      </div>

      {/* Quick metrics */}
      <div className="flex gap-4 text-xs">
        <div className="flex items-center gap-1">
          <span className="text-muted-foreground">Render:</span>
          <span className="font-mono text-green-500">{formatTime(metrics.renderTime)}</span>
        </div>
        <div className="flex items-center gap-1">
          <span className="text-muted-foreground">Components:</span>
          <span className="font-mono">{metrics.componentCount}</span>
        </div>
      </div>

      {/* Detailed metrics */}
      {showDetails && (
        <div className="mt-3 pt-3 border-t space-y-2 animate-fade-in">
          <div className="flex justify-between text-xs">
            <span className="text-muted-foreground">Messages rendered:</span>
            <span className="font-mono">{metrics.messageCount}</span>
          </div>

          {metrics.memoryUsage && (
            <div className="flex justify-between text-xs">
              <span className="text-muted-foreground">Memory used:</span>
              <span className="font-mono text-blue-500">{formatBytes(metrics.memoryUsage)}</span>
            </div>
          )}

          <div className="flex justify-between text-xs">
            <span className="text-muted-foreground">Last render time:</span>
            <span className="font-mono">{formatTime(metrics.renderTime)}</span>
          </div>

          {/* Performance tips */}
          <div className="mt-3 pt-3 border-t text-xs text-muted-foreground">
            <div className="font-semibold mb-1">Optimization Tips:</div>
            <ul className="space-y-1">
              {metrics.messageCount > 100 && (
                <li>• Virtual scroll enabled for {metrics.messageCount} messages</li>
              )}
              {metrics.renderTime > 100 && (
                <li className="text-yellow-500">• Consider reducing component complexity</li>
              )}
              {metrics.memoryUsage && metrics.memoryUsage > 50 * 1024 * 1024 && (
                <li className="text-red-500">• High memory usage detected</li>
              )}
            </ul>
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * Hook for tracking component render performance
 */
export function usePerformanceTracking(componentName: string) {
  useEffect(() => {
    const startMark = `${componentName}-render-start`;
    const endMark = `${componentName}-render-end`;
    const measureName = `${componentName}-render`;

    performance.mark(startMark);

    return () => {
      performance.mark(endMark);
      try {
        performance.measure(measureName, startMark, endMark);
      } catch (e) {
        // Mark might not exist if component unmounted quickly
      }

      // Clean up marks
      performance.clearMarks(startMark);
      performance.clearMarks(endMark);
    };
  }, [componentName]);
}