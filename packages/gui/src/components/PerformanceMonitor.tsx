import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface PerformanceMetrics {
  renderTime: number;
  messageCount: number;
  memoryUsage?: number;
  fps?: number;
}

// Performance monitoring component (matching TUI debug_mode metrics)
export function PerformanceMonitor() {
  const [metrics, setMetrics] = useState<PerformanceMetrics>({
    renderTime: 0,
    messageCount: 0,
  });
  const [isVisible, setIsVisible] = useState(false);
  const frameRef = useRef<number>(0);
  const lastTimeRef = useRef<number>(performance.now());

  // Calculate FPS
  useEffect(() => {
    let animationId: number;

    const calculateFPS = () => {
      const now = performance.now();
      const delta = now - lastTimeRef.current;
      lastTimeRef.current = now;

      frameRef.current++;
      if (frameRef.current % 10 === 0) {
        const fps = 1000 / delta;
        setMetrics(prev => ({ ...prev, fps: Math.round(fps) }));
      }

      animationId = requestAnimationFrame(calculateFPS);
    };

    if (isVisible) {
      animationId = requestAnimationFrame(calculateFPS);
    }

    return () => {
      if (animationId) {
        cancelAnimationFrame(animationId);
      }
    };
  }, [isVisible]);

  // Measure render time
  useEffect(() => {
    if (isVisible) {
      const start = performance.now();
      const end = performance.now();
      setMetrics(prev => ({ ...prev, renderTime: end - start }));
    }
  }, [isVisible]);

  // Get memory usage (if available)
  useEffect(() => {
    if (isVisible) {
      // Try to get memory info (Chrome only)
      const memory = (performance as unknown as { memory?: { usedJSHeapSize: number } })?.memory;
      if (memory) {
        setMetrics(prev => ({
          ...prev,
          memoryUsage: Math.round(memory.usedJSHeapSize / 1024 / 1024),  // MB
        }));
      }
    }
  }, [isVisible]);

  // Toggle visibility with Shift+P (matching TUI debug_mode toggle)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'P' && e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        setIsVisible(!isVisible);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isVisible]);

  if (!isVisible) return null;

  return (
    <div className="fixed top-4 right-4 z-50 bg-card border shadow-lg rounded-lg p-3 text-xs font-mono">
      <div className="flex items-center justify-between mb-2">
        <span className="font-semibold text-muted-foreground">Performance Monitor</span>
        <button
          onClick={() => setIsVisible(false)}
          className="text-muted-foreground hover:text-foreground"
        >
          ✕
        </button>
      </div>

      <div className="space-y-1">
        <div className="flex justify-between gap-4">
          <span className="text-muted-foreground">FPS:</span>
          <span className={metrics.fps && metrics.fps > 30 ? 'text-green-500' : 'text-yellow-500'}>
            {metrics.fps || '--'}
          </span>
        </div>

        <div className="flex justify-between gap-4">
          <span className="text-muted-foreground">Render:</span>
          <span className={metrics.renderTime < 16 ? 'text-green-500' : 'text-yellow-500'}>
            {metrics.renderTime.toFixed(2)}ms
          </span>
        </div>

        {metrics.memoryUsage && (
          <div className="flex justify-between gap-4">
            <span className="text-muted-foreground">Memory:</span>
            <span className="text-cyan-500">
              {metrics.memoryUsage}MB
            </span>
          </div>
        )}

        <div className="flex justify-between gap-4">
          <span className="text-muted-foreground">Messages:</span>
          <span className="text-purple-500">
            {metrics.messageCount}
          </span>
        </div>
      </div>

      <div className="mt-2 pt-2 border-t text-xs text-muted-foreground">
        Press <kbd className="px-1 bg-accent rounded">Shift+P</kbd> to hide
      </div>
    </div>
  );
}

// Performance stats collector for debugging
export class PerformanceCollector {
  private metrics: Map<string, number[]> = new Map();

  startMeasure(key: string) {
    performance.mark(`${key}-start`);
  }

  endMeasure(key: string) {
    performance.mark(`${key}-end`);
    performance.measure(key, `${key}-start`, `${key}-end`);

    const measure = performance.getEntriesByName(key, 'measure');
    const duration = measure[0]?.duration || 0;

    if (!this.metrics.has(key)) {
      this.metrics.set(key, []);
    }
    this.metrics.get(key)!.push(duration);

    // Clean up marks
    performance.clearMarks(`${key}-start`);
    performance.clearMarks(`${key}-end`);
    performance.clearMeasures(key);

    return duration;
  }

  getAverage(key: string): number {
    const values = this.metrics.get(key) || [];
    if (values.length === 0) return 0;
    return values.reduce((a, b) => a + b, 0) / values.length;
  }

  getMetrics(): Record<string, { avg: number; min: number; max: number; count: number }> {
    const result: Record<string, any> = {};

    this.metrics.forEach((values, key) => {
      result[key] = {
        avg: this.getAverage(key),
        min: Math.min(...values),
        max: Math.max(...values),
        count: values.length,
      };
    });

    return result;
  }

  clear() {
    this.metrics.clear();
  }
}

// Global performance collector instance
export const performanceCollector = new PerformanceCollector();