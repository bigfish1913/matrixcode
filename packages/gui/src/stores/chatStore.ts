import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import {
  extractToolUseData,
  extractToolResultData,
  isErrorData,
  isUsageData,
  isProgressData,
  isTextData,
  isThinkingData,
  isToolUseInputData,
} from '../utils/typeGuards';

// Agent event types matching Rust EventType (snake_case serialized)
interface AgentEvent {
  event_type: string;
  timestamp: number;
  data?: Record<string, unknown>;
}

// Event data structures (snake_case keys matching Rust serde)
// Exported for use in type guards and other modules
export interface TextData { delta: string }
export interface ThinkingData { delta: string; signature: string | null }
export interface ToolUseData { id: string; name: string; input?: unknown }
export interface ToolUseInputData { id: string; delta: string }
export interface ToolResultData { tool_use_id: string; name: string; detail?: string; content: string; is_error: boolean }
export interface ErrorData { message: string; code: string | null; source: string | null }
export interface UsageData {
  input_tokens: number;
  output_tokens: number;
  cache_creation_input_tokens?: number;
  cache_read_input_tokens?: number;
}
export interface ProgressData { message: string; percentage: number | null }

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant' | 'tool' | 'error';
  content: string;
  toolName?: string;
  toolInput?: unknown;
  isToolResult?: boolean;
  isError?: boolean;
  isStreaming?: boolean;
  thinking?: string;  // Extended thinking content
  isThinkingStreaming?: boolean;  // Thinking is being streamed
  timestamp?: number;  // Message timestamp
  executionTime?: number;  // Tool execution time in milliseconds (Phase 6 enhancement)
}

// Ask question dialog state
export interface AskQuestionState {
  question: string;
  options?: Record<string, string> | string[];
  isVisible: boolean;
}

// Debug log entry
export interface DebugLog {
  category: string;
  message: string;
  timestamp: number;
}

// Activity state
export type ActivityType =
  | 'idle'
  | 'thinking'
  | 'reading'
  | 'writing'
  | 'editing'
  | 'searching'
  | 'running'
  | 'websearch'
  | 'webfetch'
  | 'tool'
  | 'asking';  // Waiting for user response (matching TUI Activity::Asking)

export interface ActivityState {
  type: ActivityType;
  detail?: string;
  startTime?: number;  // milliseconds
  toolName?: string;  // Tool name when type is 'tool' (matching TUI Activity::Tool(name))
}

// Pending message in queue
export interface PendingMessage {
  content: string;
  timestamp: number;
}

// Workflow state (from WorkflowPanel)
export type WorkflowViewMode = 'dag' | 'progress' | 'detail';
export type WorkflowNodeStatus = 'pending' | 'running' | 'completed' | 'failed' | 'skipped';

export interface WorkflowNode {
  id: string;
  name: string;
  type: string;
  status: WorkflowNodeStatus;
  progress?: number;
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

// Todo state (from TodoWrite tool)
export interface TodoItem {
  id: string;
  content: string;
  status: 'pending' | 'in_progress' | 'completed';
  priority?: 'high' | 'medium' | 'low';
}

// LSP server status (aligns with TUI lsp_servers)
export interface LspServerInfo {
  name: string;
  status: 'running' | 'stopped' | 'error';
  language?: string;
  command?: string;
  error?: string;
}

// CodeGraph status (aligns with TUI codegraph_status)
export interface CodeGraphStatus {
  initialized: boolean;
  indexing: boolean;
  filesIndexed: number;
  symbolsIndexed: number;
  edgesIndexed: number;
  pendingFiles: string[];
  lastSync?: string;
  error?: string;
}

// Loop task (aligns with TUI loop_task)
export interface LoopTask {
  message: string;
  intervalSeconds: number;
  count: number;
  maxCount?: number;
  isActive: boolean;
}

// Cron task (aligns with TUI cron_tasks)
export interface CronTask {
  id: number;
  message: string;
  minuteInterval: number;
  isActive: boolean;
}

interface ChatState {
  messages: ChatMessage[];
  status: 'idle' | 'running' | 'error';
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;  // Tokens read from cache
  cacheCreationTokens: number;  // Tokens written to cache
  progressMessage: string | null;  // Current progress message
  askQuestion: AskQuestionState | null;  // Current ask question dialog
  showDebugPanel: boolean;  // Toggle debug panel
  debugLogs: DebugLog[];  // Debug logs
  apiCalls: number;  // API call counter
  toolCalls: number;  // Tool call counter
  compressions: number;  // Compression counter
  memorySaves: number;  // Memory save counter
  activity: ActivityState;  // Current activity
  pendingMessages: PendingMessage[];  // Messages waiting to be sent
  inputHistory: string[];  // Input history for Up/Down navigation
  workflowState: WorkflowState;  // Workflow panel state
  todos: TodoItem[];  // Todo list from TodoWrite tool
  // New fields for TUI alignment
  lspServers: LspServerInfo[];  // LSP server status
  codeGraphStatus: CodeGraphStatus | null;  // CodeGraph index status
  loopTask: LoopTask | null;  // Loop task status
  cronTasks: CronTask[];  // Cron tasks status
  _streamingMessageId: string | null;
  _unlisten: UnlistenFn | null;
  _isStarting: boolean;  // Guard flag set immediately on start
  _currentTaskId: string | null;  // Track current running task

  sendMessage: (content: string) => Promise<void>;
  retryLastMessage: () => Promise<void>;
  stopAgent: () => Promise<void>;
  loadMessages: () => Promise<void>;
  startListening: () => Promise<void>;
  stopListening: () => void;
  clearMessages: () => void;
  answerQuestion: (answer: string) => void;  // Answer the current question
  dismissQuestion: () => void;  // Dismiss the question dialog
  toggleDebugPanel: () => void;  // Toggle debug panel visibility
  addDebugLog: (category: string, message: string) => void;  // Add debug log
  clearDebugLogs: () => void;  // Clear debug logs
  addToHistory: (input: string) => void;  // Add input to history
  clearPendingMessages: () => void;  // Clear pending messages queue
  toggleWorkflowPanel: () => void;  // Toggle workflow panel visibility
  updateWorkflowState: (state: Partial<WorkflowState>) => void;  // Update workflow state
  updateTodos: (todos: TodoItem[]) => void;  // Update todo list
  // New methods for TUI alignment
  updateLspServers: (servers: LspServerInfo[]) => void;  // Update LSP status
  updateCodeGraphStatus: (status: CodeGraphStatus | null) => void;  // Update CodeGraph status
  updateLoopTask: (task: LoopTask | null) => void;  // Update loop task
  updateCronTasks: (tasks: CronTask[]) => void;  // Update cron tasks
  stopLoopTask: () => void;  // Stop active loop task
  stopCronTask: (id: number) => void;  // Stop specific cron task
}

let messageCounter = 0;
function nextId(): string {
  return `msg-${++messageCounter}`;
}

export const useChatStore = create<ChatState>((set, get) => ({
  messages: [],
  status: 'idle',
  inputTokens: 0,
  outputTokens: 0,
  cacheReadTokens: 0,
  cacheCreationTokens: 0,
  progressMessage: null,
  askQuestion: null,  // No question initially
  showDebugPanel: false,  // Debug panel hidden by default
  debugLogs: [],  // No debug logs initially
  apiCalls: 0,
  toolCalls: 0,
  compressions: 0,
  memorySaves: 0,
  activity: { type: 'idle' },  // No activity initially
  pendingMessages: [],  // No pending messages initially
  inputHistory: [],  // No input history initially
  workflowState: { visible: false, viewMode: 'dag' },  // Workflow panel hidden
  todos: [],  // No todos initially
  // New state for TUI alignment
  lspServers: [],  // No LSP servers initially
  codeGraphStatus: null,  // No CodeGraph status initially
  loopTask: null,  // No loop task initially
  cronTasks: [],  // No cron tasks initially
  _streamingMessageId: null,
  _unlisten: null,
  _isStarting: false,
  _currentTaskId: null,

  retryLastMessage: async () => {
    // Find the last user message and resend it
    const messages = get().messages;
    const lastUserMsg = [...messages].reverse().find(m => m.role === 'user');
    if (lastUserMsg) {
      // Remove error messages after the last user message
      const lastUserIdx = messages.findIndex(m => m.id === lastUserMsg.id);
      const filteredMessages = messages.slice(0, lastUserIdx + 1);
      const taskId = `task-${Date.now()}`;
      set({ messages: filteredMessages, status: 'running', _currentTaskId: taskId });
      try {
        await invoke('send_message', { message: lastUserMsg.content });
        set({ status: 'idle', _streamingMessageId: null, _currentTaskId: null });
      } catch (e) {
        const errMsg: ChatMessage = {
          id: nextId(),
          role: 'error',
          content: String(e),
          isError: true,
          timestamp: Date.now(),
        };
        set((s) => ({
          messages: [...s.messages, errMsg],
          status: 'error',
          _streamingMessageId: null,
          _currentTaskId: null,
        }));
      }
    }
  },

  stopAgent: async () => {
    const taskId = get()._currentTaskId;
    if (taskId) {
      try {
        await invoke('cancel_task', { taskId });
      } catch (e) {
        console.warn('Failed to cancel task:', e);
      }
    }
    // Reset state regardless of backend response
    set({
      status: 'idle',
      _streamingMessageId: null,
      _currentTaskId: null,
    });
  },

  sendMessage: async (content: string) => {
    // Add to input history
    if (content.trim().length > 0) {
      get().addToHistory(content.trim());
    }

    // Add user message
    const userMsg: ChatMessage = {
      id: nextId(),
      role: 'user',
      content,
      timestamp: Date.now(),
    };
    // Generate a task ID for this request (even though backend doesn't support cancellation yet)
    const taskId = `task-${Date.now()}`;
    set((s) => ({
      messages: [...s.messages, userMsg],
      status: 'running',
      progressMessage: null,
      _currentTaskId: taskId,
    }));

    try {
      await invoke('send_message', { message: content });
      // Agent completed successfully - reset status
      set({ status: 'idle', _streamingMessageId: null, _currentTaskId: null, activity: { type: 'idle' } });
    } catch (e) {
      const errMsg: ChatMessage = {
        id: nextId(),
        role: 'error',
        content: String(e),
        isError: true,
        timestamp: Date.now(),
      };
      set((s) => ({
        messages: [...s.messages, errMsg],
        status: 'error',
        _streamingMessageId: null,
        _currentTaskId: null,
        activity: { type: 'idle' },
      }));
    }
  },

  loadMessages: async () => {
    interface MessageInfo {
      role: string;
      content: string;
      thinking?: string;  // Thinking content from backend
      timestamp?: number;
    }
    const raw = await invoke<MessageInfo[]>('get_messages');
    // Map roles safely - Rust sends lowercase: 'user', 'assistant', 'system', 'tool'
    const messages: ChatMessage[] = raw.map((m, i) => {
      let role: ChatMessage['role'] = 'assistant';
      if (m.role === 'user') role = 'user';
      else if (m.role === 'tool') role = 'tool';
      else if (m.role === 'error') role = 'error';
      // 'system' and other roles map to 'assistant' as fallback
      return {
        id: `loaded-${i}`,
        role,
        content: m.content,
        thinking: m.thinking,  // Include thinking content from history
        timestamp: m.timestamp || Date.now() - i * 1000, // Approximate timestamp if missing
      };
    });
    set({ messages });
  },

  startListening: async () => {
    // Check both guard flags - this prevents duplicate listeners in StrictMode
    if (get()._unlisten || get()._isStarting) return;

    // Set flag immediately to prevent concurrent calls
    set({ _isStarting: true });

    try {
      const unlisten = await listen<AgentEvent>('agent-event', (event) => {
        const agentEvent = event.payload;
        const { event_type, data } = agentEvent;

        // Debug: log all events for troubleshooting
        console.log('[Agent Event]', event_type, data);

        // All event types are snake_case from Rust serde
        switch (event_type) {
          case 'text_start': {
            // Create message if not exists, or use existing one (if thinking came first)
            const streamingId = get()._streamingMessageId;
            if (!streamingId) {
              // No message exists - create new one
              const msgId = nextId();
              const msg: ChatMessage = {
                id: msgId,
                role: 'assistant',
                content: '',
                isStreaming: true,
                timestamp: Date.now(),
              };
              set((s) => ({
                messages: [...s.messages, msg],
                _streamingMessageId: msgId,
              }));
            }
            // If message already exists (created by thinking_start), use it
            // No need to create another message
            break;
          }
          case 'text_delta': {
            if (data && isTextData(data)) {
              const streamingId = get()._streamingMessageId;
              const deltaText = data.text?.delta || '';
              set((s) => {
                const msgs = [...s.messages];
                const idx = streamingId ? msgs.findIndex(m => m.id === streamingId) : msgs.length - 1;
                if (idx >= 0 && msgs[idx]?.isStreaming) {
                  msgs[idx] = {
                    ...msgs[idx],
                    content: msgs[idx].content + deltaText,
                  };
                }
                return { messages: msgs };
              });
            }
            break;
          }
          case 'text_end': {
            const streamingId = get()._streamingMessageId;
            set((s) => {
              const msgs = [...s.messages];
              const idx = streamingId ? msgs.findIndex(m => m.id === streamingId) : msgs.length - 1;
              if (idx >= 0 && msgs[idx]?.isStreaming) {
                msgs[idx] = { ...msgs[idx], isStreaming: false };
              }
              return { messages: msgs, _streamingMessageId: null };
            });
            break;
          }
          case 'thinking_start': {
            // Update activity to 'thinking'
            set({
              activity: {
                type: 'thinking',
                startTime: Date.now(),
              },
            });

            // Create message if not exists (thinking can come before text_start)
            const streamingId = get()._streamingMessageId;
            if (!streamingId) {
              // Thinking comes before text_start - create message first
              const msgId = nextId();
              const msg: ChatMessage = {
                id: msgId,
                role: 'assistant',
                content: '',
                thinking: '',
                isStreaming: true,
                isThinkingStreaming: true,
                timestamp: Date.now(),
              };
              set((s) => ({
                messages: [...s.messages, msg],
                _streamingMessageId: msgId,
              }));
            } else {
              // Text already started - add thinking to existing message
              set((s) => {
                const msgs = [...s.messages];
                const idx = msgs.findIndex(m => m.id === streamingId);
                if (idx >= 0) {
                  msgs[idx] = {
                    ...msgs[idx],
                    thinking: '',
                    isThinkingStreaming: true,
                  };
                }
                return { messages: msgs };
              });
            }
            break;
          }
          case 'thinking_delta': {
            if (data && isThinkingData(data)) {
              const streamingId = get()._streamingMessageId;
              const deltaThinking = data.thinking?.delta || '';
              set((s) => {
                const msgs = [...s.messages];
                const idx = streamingId ? msgs.findIndex(m => m.id === streamingId) : msgs.length - 1;
                if (idx >= 0 && msgs[idx]?.isThinkingStreaming) {
                  msgs[idx] = {
                    ...msgs[idx],
                    thinking: (msgs[idx].thinking || '') + deltaThinking,
                  };
                }
                return { messages: msgs };
              });
            }
            break;
          }
          case 'thinking_end': {
            // Reset activity to idle when thinking ends
            set({ activity: { type: 'idle' } });

            const streamingId = get()._streamingMessageId;
            set((s) => {
              const msgs = [...s.messages];
              const idx = streamingId ? msgs.findIndex(m => m.id === streamingId) : msgs.length - 1;
              if (idx >= 0 && msgs[idx]?.isThinkingStreaming) {
                msgs[idx] = { ...msgs[idx], isThinkingStreaming: false };
              }
              return { messages: msgs };
            });
            break;
          }
          case 'tool_use_start': {
            // Update activity to 'tool' and increment counter
            set((s) => ({
              activity: {
                type: 'tool',
                detail: data ? (data as any).name || 'Unknown' : 'Tool',
                startTime: Date.now(),
              },
              toolCalls: s.toolCalls + 1,
            }));

            if (data) {
              const toolData = extractToolUseData(data);

              if (toolData) {
                const msg: ChatMessage = {
                  id: toolData.id,
                  role: 'tool',
                  content: '',
                  toolName: toolData.name,
                  toolInput: toolData.input ?? {},
                  timestamp: Date.now(),
                };
                set((s) => ({ messages: [...s.messages, msg] }));
              } else {
                console.warn('[tool_use_start] Could not parse tool data:', data);
              }
            }
            break;
          }
          case 'tool_use_input_delta': {
            // Streaming tool input - append to existing tool message
            if (data && isToolUseInputData(data)) {
              set((s) => {
                const msgs = [...s.messages];
                const toolMsg = msgs.find(m => m.id === data.tool_use_input.id);
                if (toolMsg && toolMsg.role === 'tool') {
                  // Append delta to tool input JSON string
                  const currentInput = toolMsg.toolInput ? JSON.stringify(toolMsg.toolInput, null, 2) : '';
                  toolMsg.toolInput = currentInput + data.tool_use_input.delta;
                }
                return { messages: msgs };
              });
            }
            break;
          }
          case 'tool_use_input_end':
            // Tool input streaming complete - parse final input
            break;
          case 'tool_result': {
            // Reset activity after tool execution
            set({ activity: { type: 'idle' } });

            if (data) {
              const resultData = extractToolResultData(data);

              if (resultData) {
                // Calculate execution time
                const toolStartMsg = get().messages.find(m => m.id === resultData.tool_use_id);
                const executionTime = toolStartMsg?.timestamp
                  ? Date.now() - toolStartMsg.timestamp
                  : undefined;

                const msg: ChatMessage = {
                  id: nextId(),
                  role: resultData.is_error ? 'error' : 'tool',
                  content: resultData.content,
                  toolName: resultData.name,
                  isToolResult: true,
                  isError: resultData.is_error,
                  timestamp: Date.now(),
                  executionTime,
                };
                set((s) => ({ messages: [...s.messages, msg] }));
              } else {
                console.warn('[tool_result] Could not parse result data:', data);
              }
            }
            break;
          }
          case 'error': {
            if (data && isErrorData(data)) {
              const msg: ChatMessage = {
                id: nextId(),
                role: 'error',
                content: data.error.message,
                isError: true,
              };
              set((s) => ({
                messages: [...s.messages, msg],
                status: 'error',
                _streamingMessageId: null,
              }));
            }
            break;
          }
          case 'usage': {
            // Increment API call counter when usage reported
            if (data && isUsageData(data)) {
              set((s) => ({
                inputTokens: data.usage.input_tokens,
                outputTokens: data.usage.output_tokens,
                cacheReadTokens: data.usage.cache_read_input_tokens ?? 0,
                cacheCreationTokens: data.usage.cache_creation_input_tokens ?? 0,
                apiCalls: s.apiCalls + 1,
                progressMessage: null,
              }));
            }
            break;
          }
          case 'progress': {
            if (data && isProgressData(data)) {
              set({ progressMessage: data.progress.message });
              console.log(`Progress: ${data.progress.message} (${data.progress.percentage}%)`);
            }
            break;
          }
          case 'compression_triggered':
            // Context compression started - increment counter and show status
            set((s) => ({ compressions: s.compressions + 1 }));
            console.log('Context compression triggered...');
            break;
          case 'compression_completed':
            console.log('Context compression completed');
            break;
          case 'queue_processed': {
            // Pending messages processed by agent - show notification
            if (data) {
              const queueData = (data as { queue_processed: { count: number; messages?: string[] } }).queue_processed;
              // Add pending messages to state for display
              if (queueData.messages && queueData.messages.length > 0) {
                const messages = queueData.messages;  // Ensure type safety
                set((s) => ({
                  pendingMessages: messages.map((msg, idx) => ({
                    content: msg,
                    timestamp: Date.now() + idx,
                  })),
                }));
              }
              console.log(`Queue processed: ${queueData.count} messages`);
            }
            break;
          }
          case 'debug_log': {
            // Debug log entry - record for debug panel
            if (data) {
              const debugData = (data as { debug_log: { category: string; message: string } }).debug_log;
              const log: DebugLog = {
                category: debugData.category,
                message: debugData.message,
                timestamp: Date.now(),
              };
              set((s) => ({ debugLogs: [...s.debugLogs.slice(-100), log] }));
            }
            break;
          }
          case 'session_started':
            break;
          case 'session_ended':
            set({ status: 'idle', _streamingMessageId: null });
            break;
          case 'session_restored':
          case 'history_loaded':
            // Session restored - could refresh messages
            break;
          case 'ask_question': {
            // Ask tool waiting for user input - show dialog
            if (data) {
              const askData = (data as { ask_question: { question: string; options?: Record<string, string> | string[] } }).ask_question;
              set({
                askQuestion: {
                  question: askData.question,
                  options: askData.options,
                  isVisible: true,
                },
              });
            }
            break;
          }
          case 'mcp_server_added':
          case 'mcp_server_removed':
          case 'mcp_server_status':
          case 'lsp_server_added':
          case 'lsp_server_removed':
          case 'lsp_server_status':
            // Infrastructure status events
            break;
          case 'loop_task_started': {
            // Loop task started - update state
            if (data) {
              const loopData = (data as { loop_task: LoopTask }).loop_task;
              set({ loopTask: loopData });
            }
            break;
          }
          case 'loop_task_progress': {
            // Loop task progress update
            if (data) {
              const progressData = (data as { loop_task: { count: number } }).loop_task;
              set((s) => ({
                loopTask: s.loopTask ? { ...s.loopTask, count: progressData.count } : null,
              }));
            }
            break;
          }
          case 'loop_task_completed': {
            // Loop task completed or stopped
            set({ loopTask: null });
            break;
          }
          case 'cron_task_started': {
            // Cron task started
            if (data) {
              const cronData = (data as { cron_task: CronTask }).cron_task;
              set((s) => ({
                cronTasks: [...s.cronTasks, cronData],
              }));
            }
            break;
          }
          case 'cron_task_list': {
            // List of active cron tasks
            if (data) {
              const listData = (data as { cron_tasks: CronTask[] }).cron_tasks;
              set({ cronTasks: listData });
            }
            break;
          }
          case 'cron_task_stopped': {
            // Cron task stopped
            if (data) {
              const stopData = (data as { cron_task_id: number }).cron_task_id;
              set((s) => ({
                cronTasks: s.cronTasks.filter(t => t.id !== stopData),
              }));
            }
            break;
          }
          case 'workflow_started': {
            // Workflow execution started
            if (data) {
              const workflowData = (data as { workflow: WorkflowState['workflowDef'] }).workflow;
              set((s) => ({
                workflowState: {
                  ...s.workflowState,
                  visible: true,
                  workflowDef: workflowData,
                },
              }));
            }
            break;
          }
          case 'workflow_progress': {
            // Workflow node progress update
            if (data) {
              const progressData = (data as { workflow: { nodes: WorkflowNode[]; progress?: number } }).workflow;
              set((s) => {
                const workflowDef = s.workflowState.workflowDef;
                if (workflowDef) {
                  return {
                    workflowState: {
                      ...s.workflowState,
                      workflowDef: {
                        ...workflowDef,
                        nodes: progressData.nodes,
                      },
                      progress: progressData.progress,
                    },
                  };
                }
                return s;
              });
            }
            break;
          }
          case 'workflow_completed': {
            // Workflow execution completed
            if (data) {
              const resultData = (data as { workflow: { success: boolean; summary?: string } }).workflow;
              set((s) => ({
                workflowState: {
                  ...s.workflowState,
                  visible: false,
                  progress: 100,
                },
              }));
              console.log(`Workflow completed: ${resultData.success ? 'success' : 'failed'}`);
            }
            break;
          }
          case 'workflow_node_started': {
            // Individual workflow node started
            if (data) {
              const nodeData = (data as { node: { id: string; name: string; type: string; startTime: number } }).node;
              set((s) => {
                const workflowDef = s.workflowState.workflowDef;
                if (workflowDef) {
                  const nodes = workflowDef.nodes.map(n =>
                    n.id === nodeData.id
                      ? { ...n, status: 'running' as WorkflowNodeStatus, startTime: nodeData.startTime }
                      : n
                  );
                  return {
                    workflowState: {
                      ...s.workflowState,
                      workflowDef: { ...workflowDef, nodes },
                    },
                  };
                }
                return s;
              });
            }
            break;
          }
          case 'workflow_node_completed': {
            // Individual workflow node completed
            if (data) {
              const nodeData = (data as { node: { id: string; endTime: number; error?: string } }).node;
              set((s) => {
                const workflowDef = s.workflowState.workflowDef;
                if (workflowDef) {
                  const nodes = workflowDef.nodes.map(n =>
                    n.id === nodeData.id
                      ? {
                          ...n,
                          status: nodeData.error ? 'failed' as WorkflowNodeStatus : 'completed' as WorkflowNodeStatus,
                          endTime: nodeData.endTime,
                          error: nodeData.error,
                        }
                      : n
                  );
                  return {
                    workflowState: {
                      ...s.workflowState,
                      workflowDef: { ...workflowDef, nodes },
                    },
                  };
                }
                return s;
              });
            }
            break;
          }
          case 'todo_updated': {
            // Todo list updated (from TodoWrite tool)
            if (data) {
              const todoData = (data as { todos: TodoItem[] }).todos;
              set({ todos: todoData });
            }
            break;
          }
          case 'memory_saved': {
            // Memory saved to disk
            set((s) => ({ memorySaves: s.memorySaves + 1 }));
            if (data) {
              const memoryData = (data as { memory: { key: string; value: string } }).memory;
              console.log(`Memory saved: ${memoryData.key}`);
            }
            break;
          }
          default:
            // Log unhandled events for debugging instead of silently dropping
            console.debug(`Unhandled event type: ${event_type}`, data);
            break;
        }
      });

      set({ _unlisten: unlisten, _isStarting: false });
    } catch (e) {
      // Reset flag on error
      set({ _isStarting: false });
      console.error('Failed to start listening:', e);
    }
  },

  stopListening: () => {
    const unlisten = get()._unlisten;
    if (unlisten) {
      unlisten();
      set({ _unlisten: null, _isStarting: false });
    }
  },

  clearMessages: () => {
    set({ messages: [], status: 'idle', _streamingMessageId: null, askQuestion: null });
  },

  answerQuestion: (answer: string) => {
    // Add answer as a user message
    const userMsg: ChatMessage = {
      id: nextId(),
      role: 'user',
      content: answer,
      timestamp: Date.now(),
    };
    set((s) => ({
      messages: [...s.messages, userMsg],
      askQuestion: null,  // Hide the dialog
    }));
    // Note: The answer will be sent to the agent via pending input mechanism
    // This requires backend support for real-time input appending
    console.log('User answered question:', answer);
  },

  dismissQuestion: () => {
    set({ askQuestion: null });
  },

  toggleDebugPanel: () => {
    set((s) => ({ showDebugPanel: !s.showDebugPanel }));
  },

  addDebugLog: (category: string, message: string) => {
    const log: DebugLog = {
      category,
      message,
      timestamp: Date.now(),
    };
    set((s) => ({ debugLogs: [...s.debugLogs.slice(-100), log] }));  // Keep last 100 logs
  },

  clearDebugLogs: () => {
    set({ debugLogs: [] });
  },

  addToHistory: (input: string) => {
    // Add input to history, limit to last 100 entries
    set((s) => ({
      inputHistory: [...s.inputHistory.slice(-99), input],
    }));
  },

  clearPendingMessages: () => {
    set({ pendingMessages: [] });
  },

  toggleWorkflowPanel: () => {
    set((s) => ({
      workflowState: {
        ...s.workflowState,
        visible: !s.workflowState.visible,
      },
    }));
  },

  updateWorkflowState: (state: Partial<WorkflowState>) => {
    set((s) => ({
      workflowState: {
        ...s.workflowState,
        ...state,
      },
    }));
  },

  updateTodos: (todos: TodoItem[]) => {
    set({ todos });
  },

  // New methods for TUI alignment
  updateLspServers: (servers: LspServerInfo[]) => {
    set({ lspServers: servers });
  },

  updateCodeGraphStatus: (status: CodeGraphStatus | null) => {
    set({ codeGraphStatus: status });
  },

  updateLoopTask: (task: LoopTask | null) => {
    set({ loopTask: task });
  },

  updateCronTasks: (tasks: CronTask[]) => {
    set({ cronTasks: tasks });
  },

  stopLoopTask: () => {
    const task = get().loopTask;
    if (task && task.isActive) {
      // Mark as inactive
      set({ loopTask: null });
      // TODO: Call backend to cancel the loop task
    }
  },

  stopCronTask: (id: number) => {
    const tasks = get().cronTasks;
    const updated = tasks.map(t =>
      t.id === id ? { ...t, isActive: false } : t
    ).filter(t => t.isActive);
    set({ cronTasks: updated });
    // TODO: Call backend to cancel the cron task
  },
}));