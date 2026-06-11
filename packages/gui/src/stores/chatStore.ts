import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

// Agent event types matching Rust EventType (snake_case serialized)
interface AgentEvent {
  event_type: string;
  timestamp: number;
  data?: Record<string, unknown>;
}

// Event data structures (snake_case keys matching Rust serde)
interface TextData { delta: string }
interface ThinkingData { delta: string; signature: string | null }
interface ToolUseData { id: string; name: string; input?: unknown }
interface ToolUseInputData { id: string; delta: string }
interface ToolResultData { tool_use_id: string; name: string; detail?: string; content: string; is_error: boolean }
interface ErrorData { message: string; code: string | null; source: string | null }
interface UsageData {
  input_tokens: number;
  output_tokens: number;
  cache_creation_input_tokens?: number;
  cache_read_input_tokens?: number;
}
interface ProgressData { message: string; percentage: number | null }

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
}

interface ChatState {
  messages: ChatMessage[];
  status: 'idle' | 'running' | 'error';
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;  // Tokens read from cache
  cacheCreationTokens: number;  // Tokens written to cache
  progressMessage: string | null;  // Current progress message
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
  },

  loadMessages: async () => {
    interface MessageInfo {
      role: string;
      content: string;
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
            break;
          }
          case 'text_delta': {
            if (data) {
              const textData = (data as { text: TextData }).text;
              const streamingId = get()._streamingMessageId;
              set((s) => {
                const msgs = [...s.messages];
                // Find the streaming message by ID, not just the last one
                const idx = streamingId ? msgs.findIndex(m => m.id === streamingId) : msgs.length - 1;
                if (idx >= 0 && msgs[idx]?.isStreaming) {
                  msgs[idx] = {
                    ...msgs[idx],
                    content: msgs[idx].content + (textData?.delta || ''),
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
            const streamingId = get()._streamingMessageId;
            if (streamingId) {
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
            if (data) {
              const thinkingData = (data as { thinking: ThinkingData }).thinking;
              const streamingId = get()._streamingMessageId;
              set((s) => {
                const msgs = [...s.messages];
                const idx = streamingId ? msgs.findIndex(m => m.id === streamingId) : msgs.length - 1;
                if (idx >= 0 && msgs[idx]?.isThinkingStreaming) {
                  msgs[idx] = {
                    ...msgs[idx],
                    thinking: (msgs[idx].thinking || '') + (thinkingData?.delta || ''),
                  };
                }
                return { messages: msgs };
              });
            }
            break;
          }
          case 'thinking_end': {
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
            if (data) {
              // Try to extract tool_use from data, handle both nested and flat formats
              let toolData: ToolUseData | null = null;
              if ('tool_use' in data) {
                toolData = (data as { tool_use: ToolUseData }).tool_use;
              } else if ('id' in data && 'name' in data) {
                // Handle flat format (direct fields)
                toolData = data as unknown as ToolUseData;
              }

              if (toolData) {
                const msg: ChatMessage = {
                  id: toolData.id,
                  role: 'tool',
                  content: '',
                  toolName: toolData.name,
                  toolInput: toolData.input ?? {},
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
            if (data) {
              const inputDelta = (data as { tool_use_input: ToolUseInputData }).tool_use_input;
              set((s) => {
                const msgs = [...s.messages];
                const toolMsg = msgs.find(m => m.id === inputDelta.id);
                if (toolMsg && toolMsg.role === 'tool') {
                  // Append delta to tool input JSON string
                  const currentInput = toolMsg.toolInput ? JSON.stringify(toolMsg.toolInput, null, 2) : '';
                  toolMsg.toolInput = currentInput + inputDelta.delta;
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
            if (data) {
              // Try to extract tool_result from data, handle both nested and flat formats
              let resultData: ToolResultData | null = null;
              if ('tool_result' in data) {
                resultData = (data as { tool_result: ToolResultData }).tool_result;
              } else if ('tool_use_id' in data && 'content' in data) {
                // Handle flat format
                resultData = data as unknown as ToolResultData;
              }

              if (resultData) {
                const msg: ChatMessage = {
                  id: nextId(),
                  role: resultData.is_error ? 'error' : 'tool',
                  content: resultData.content,
                  toolName: resultData.name,
                  isToolResult: true,
                  isError: resultData.is_error,
                };
                set((s) => ({ messages: [...s.messages, msg] }));
              } else {
                console.warn('[tool_result] Could not parse result data:', data);
              }
            }
            break;
          }
          case 'error': {
            if (data) {
              const errData = (data as { error: ErrorData }).error;
              const msg: ChatMessage = {
                id: nextId(),
                role: 'error',
                content: errData.message,
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
            if (data) {
              const usageData = (data as { usage: UsageData }).usage;
              set({
                inputTokens: usageData.input_tokens,
                outputTokens: usageData.output_tokens,
                cacheReadTokens: usageData.cache_read_input_tokens ?? 0,
                cacheCreationTokens: usageData.cache_creation_input_tokens ?? 0,
                progressMessage: null,  // Clear progress when usage is reported
              });
            }
            break;
          }
          case 'progress': {
            if (data) {
              const progressData = (data as { progress: ProgressData }).progress;
              set({ progressMessage: progressData.message });
              console.log(`Progress: ${progressData.message} (${progressData.percentage}%)`);
            }
            break;
          }
          case 'compression_triggered':
            // Context compression started - show status
            console.log('Context compression triggered...');
            break;
          case 'compression_completed':
            console.log('Context compression completed');
            break;
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
            // Ask tool waiting for user input - this blocks the agent
            // TODO: Show UI prompt and send response back
            console.warn('Agent is asking a question - UI prompt needed');
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
    set({ messages: [], status: 'idle', _streamingMessageId: null });
  },
}));