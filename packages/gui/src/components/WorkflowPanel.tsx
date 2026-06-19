import React, { useState } from 'react';

export type WorkflowViewMode = 'dag' | 'progress' | 'detail';

export type WorkflowNodeStatus = 'pending' | 'running' | 'completed' | 'failed' | 'skipped';

export interface WorkflowNode {
  id: string;
  name: string;
  type: string;
  status: WorkflowNodeStatus;
  progress?: number;  // 0-100
  error?: string;
  startTime?: number;
  endTime?: number;
}

export interface WorkflowEdge {
  from: string;
  to: string;
  label?: string;
}

export interface WorkflowState {
  visible: boolean;
  viewMode: WorkflowViewMode;
  workflowDef?: {
    id: string;
    name: string;
    nodes: WorkflowNode[];
    edges: WorkflowEdge[];
  };
  selectedNode?: string;
  progress?: number;
}

interface WorkflowPanelProps {
  workflowState: WorkflowState;
  onToggle: () => void;
  onSelectNode?: (nodeId: string) => void;
  onChangeViewMode?: (mode: WorkflowViewMode) => void;
}

const STATUS_COLORS: Record<WorkflowNodeStatus, string> = {
  pending: 'text-gray-400 bg-gray-100 dark:bg-gray-800',
  running: 'text-yellow-600 bg-yellow-50 dark:bg-yellow-900/20 animate-pulse',
  completed: 'text-green-600 bg-green-50 dark:bg-green-900/20',
  failed: 'text-red-600 bg-red-50 dark:bg-red-900/20',
  skipped: 'text-gray-500 bg-gray-50 dark:bg-gray-800/50',
};

const STATUS_ICONS: Record<WorkflowNodeStatus, string> = {
  pending: '⏳',
  running: '🔄',
  completed: '✅',
  failed: '❌',
  skipped: '⏭️',
};

export function WorkflowPanel({ workflowState, onToggle, onSelectNode, onChangeViewMode }: WorkflowPanelProps) {
  const [viewMode, setViewMode] = useState<WorkflowViewMode>(workflowState.viewMode || 'dag');

  if (!workflowState.visible) {
    return null;
  }

  const workflow = workflowState.workflowDef;

  const handleViewModeChange = (mode: WorkflowViewMode) => {
    setViewMode(mode);
    onChangeViewMode?.(mode);
  };

  const renderDagView = () => {
    if (!workflow) {
      return (
        <div className="p-4 text-center text-muted-foreground">
          <p className="text-sm">No workflow loaded</p>
          <p className="text-xs mt-2">Use /workflow commands to start</p>
        </div>
      );
    }

    return (
      <div className="p-3 space-y-2">
        {/* Workflow title */}
        <div className="text-sm font-semibold text-foreground border-b pb-2 mb-3">
          {workflow.name}
        </div>

        {/* Nodes list */}
        {workflow.nodes.map((node) => (
          <button
            key={node.id}
            onClick={() => onSelectNode?.(node.id)}
            className={`w-full p-2 rounded-lg text-xs transition-colors ${
              workflowState.selectedNode === node.id
                ? 'ring-2 ring-primary bg-primary/10'
                : 'hover:bg-muted/50'
            } ${STATUS_COLORS[node.status]}`}
          >
            <div className="flex items-center gap-2">
              <span>{STATUS_ICONS[node.status]}</span>
              <span className="font-medium truncate">{node.name}</span>
            </div>

            {/* Progress bar */}
            {node.status === 'running' && node.progress !== undefined && (
              <div className="mt-1.5 h-1 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                <div
                  className="h-full bg-yellow-500 transition-all"
                  style={{ width: `${node.progress}%` }}
                />
              </div>
            )}

            {/* Error message */}
            {node.status === 'failed' && node.error && (
              <div className="mt-1 text-xs text-red-600 truncate">
                Error: {node.error.slice(0, 50)}...
              </div>
            )}
          </button>
        ))}

        {/* Edges visualization */}
        {workflow.edges.length > 0 && (
          <div className="mt-4 pt-3 border-t">
            <div className="text-xs text-muted-foreground mb-2">Dependencies:</div>
            {workflow.edges.map((edge, idx) => (
              <div key={idx} className="text-xs text-muted-foreground/70 flex items-center gap-1">
                <span className="truncate">{edge.from}</span>
                <span>→</span>
                <span className="truncate">{edge.to}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    );
  };

  const renderProgressView = () => {
    if (!workflow) {
      return (
        <div className="p-4 text-center text-muted-foreground">
          <p className="text-sm">No workflow in progress</p>
        </div>
      );
    }

    const completedNodes = workflow.nodes.filter(n => n.status === 'completed').length;
    const totalNodes = workflow.nodes.length;
    const overallProgress = totalNodes > 0 ? (completedNodes / totalNodes) * 100 : 0;

    return (
      <div className="p-4">
        {/* Overall progress */}
        <div className="mb-4">
          <div className="flex justify-between text-sm mb-2">
            <span className="text-muted-foreground">Overall Progress</span>
            <span className="font-medium">{Math.round(overallProgress)}%</span>
          </div>
          <div className="h-2 bg-muted rounded-full overflow-hidden">
            <div
              className="h-full bg-primary transition-all"
              style={{ width: `${overallProgress}%` }}
            />
          </div>
          <div className="text-xs text-muted-foreground mt-1">
            {completedNodes} / {totalNodes} nodes completed
          </div>
        </div>

        {/* Current running nodes */}
        {workflow.nodes.filter(n => n.status === 'running').map((node) => (
          <div key={node.id} className="p-3 bg-yellow-50 dark:bg-yellow-900/20 rounded-lg mb-2">
            <div className="flex items-center gap-2 text-sm">
              <span className="animate-spin">🔄</span>
              <span className="font-medium">{node.name}</span>
            </div>
            {node.progress !== undefined && (
              <div className="mt-2 h-1 bg-yellow-200 dark:bg-yellow-800 rounded-full">
                <div
                  className="h-full bg-yellow-600"
                  style={{ width: `${node.progress}%` }}
                />
              </div>
            )}
          </div>
        ))}

        {/* Failed nodes */}
        {workflow.nodes.filter(n => n.status === 'failed').length > 0 && (
          <div className="mt-3 pt-3 border-t">
            <div className="text-xs text-red-600 font-medium mb-2">Failed Nodes:</div>
            {workflow.nodes.filter(n => n.status === 'failed').map((node) => (
              <div key={node.id} className="text-xs text-red-600 mb-1">
                • {node.name}
              </div>
            ))}
          </div>
        )}
      </div>
    );
  };

  const renderDetailView = () => {
    if (!workflow || !workflowState.selectedNode) {
      return (
        <div className="p-4 text-center text-muted-foreground text-sm">
          <p>No node selected</p>
          <p className="text-xs mt-2">Click a node in DAG view</p>
        </div>
      );
    }

    const node = workflow.nodes.find(n => n.id === workflowState.selectedNode);
    if (!node) return null;

    return (
      <div className="p-4 space-y-3">
        {/* Node info */}
        <div>
          <div className="text-sm font-semibold text-foreground">
            {node.name}
          </div>
          <div className="text-xs text-muted-foreground mt-1">
            ID: {node.id}
          </div>
          <div className="text-xs text-muted-foreground">
            Type: {node.type}
          </div>
        </div>

        {/* Status */}
        <div className={`p-2 rounded ${STATUS_COLORS[node.status]}`}>
          <div className="flex items-center gap-2 text-xs">
            <span>{STATUS_ICONS[node.status]}</span>
            <span className="font-medium capitalize">{node.status}</span>
          </div>
        </div>

        {/* Timing */}
        {node.startTime && (
          <div className="text-xs text-muted-foreground space-y-1">
            <div>Started: {new Date(node.startTime).toLocaleTimeString()}</div>
            {node.endTime && (
              <div>Ended: {new Date(node.endTime).toLocaleTimeString()}</div>
            )}
          </div>
        )}

        {/* Error */}
        {node.error && (
          <div className="p-2 bg-red-50 dark:bg-red-900/20 rounded text-xs text-red-600">
            <div className="font-medium mb-1">Error:</div>
            <div className="whitespace-pre-wrap">{node.error}</div>
          </div>
        )}

        {/* Back button */}
        <button
          onClick={() => handleViewModeChange('dag')}
          className="w-full mt-4 px-3 py-1.5 text-xs bg-muted hover:bg-muted/80 rounded transition-colors"
        >
          Back to DAG View
        </button>
      </div>
    );
  };

  return (
    <>
      {/* Toggle button */}
      <button
        onClick={onToggle}
        className="fixed right-4 top-4 z-50 px-3 py-1.5 bg-primary text-primary-foreground rounded-lg text-xs font-medium shadow-lg hover:bg-primary/90 transition-colors"
        title="Toggle Workflow Panel"
      >
        ⚙ Workflow
      </button>

      {/* Panel overlay */}
      <div className="fixed right-0 top-0 h-full w-[320px] bg-card border-l shadow-xl z-40 overflow-hidden">
        {/* Header */}
        <div className="border-b p-3 bg-muted/30">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <span className="text-lg">⚙</span>
              <span className="font-semibold text-sm">Workflow Panel</span>
            </div>
            <button
              onClick={onToggle}
              className="text-muted-foreground hover:text-foreground transition-colors"
            >
              ✕
            </button>
          </div>

          {/* View mode tabs */}
          <div className="flex gap-1 mt-3">
            <button
              onClick={() => handleViewModeChange('dag')}
              className={`px-2 py-1 text-xs rounded ${
                viewMode === 'dag'
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-muted hover:bg-muted/80'
              }`}
            >
              DAG
            </button>
            <button
              onClick={() => handleViewModeChange('progress')}
              className={`px-2 py-1 text-xs rounded ${
                viewMode === 'progress'
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-muted hover:bg-muted/80'
              }`}
            >
              Progress
            </button>
            <button
              onClick={() => handleViewModeChange('detail')}
              className={`px-2 py-1 text-xs rounded ${
                viewMode === 'detail'
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-muted hover:bg-muted/80'
              }`}
            >
              Detail
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="h-[calc(100%-120px)] overflow-y-auto">
          {viewMode === 'dag' && renderDagView()}
          {viewMode === 'progress' && renderProgressView()}
          {viewMode === 'detail' && renderDetailView()}
        </div>

        {/* Footer hint */}
        <div className="absolute bottom-0 left-0 right-0 p-2 bg-muted/30 border-t text-xs text-muted-foreground">
          <div className="flex justify-between">
            <span>Alt+W to toggle</span>
            <span>{workflow?.nodes.length || 0} nodes</span>
          </div>
        </div>
      </div>
    </>
  );
}