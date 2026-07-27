/**
 * MatrixCode CLI Client - Daemon Mode
 * Communicates with matrixcode CLI via stdin/stdout JSON stream
 */

import { spawn, ChildProcess } from 'child_process';
import { AgentEvent, DaemonRequest, RequestContext } from './types';

export class MatrixCodeClient {
  private process: ChildProcess | null = null;
  private eventHandlers: Map<string, (event: AgentEvent) => void> = new Map();
  private pendingRequests: Map<string, (events: AgentEvent[]) => void> = new Map();
  private eventBuffer: AgentEvent[] = [];
  private isProcessingRequest = false;

  constructor(private cliPath: string = 'matrixcode') {}

  /**
   * Start the daemon process
   */
  async start(): Promise<void> {
    if (this.process) {
      return;
    }

    this.process = spawn(this.cliPath, ['--mode', 'daemon'], {
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    if (!this.process.stdout || !this.process.stdin) {
      throw new Error('Failed to create daemon process pipes');
    }

    // Handle stdout - JSON event stream
    let buffer = '';
    this.process.stdout.on('data', (data: Buffer) => {
      buffer += data.toString();
      const lines = buffer.split('\n');
      buffer = lines.pop() || '';

      for (const line of lines) {
        this.handleLine(line);
      }
    });

    // Handle stderr - debug logs
    this.process.stderr?.on('data', (data: Buffer) => {
      console.debug('[MatrixCode Daemon]', data.toString());
    });

    // Handle process exit
    this.process.on('exit', (code) => {
      console.debug(`[MatrixCode Daemon] exited with code ${code}`);
      this.process = null;
    });

    // Handle errors
    this.process.on('error', (err) => {
      console.error('[MatrixCode Daemon] error:', err);
      this.emit('error', { event_type: 'error', timestamp: Date.now(), data: { error: { message: err.message } } });
    });
  }

  /**
   * Handle a line from daemon output
   */
  private handleLine(line: string): void {
    if (line.trim() === '---END---') {
      // Request completed
      this.isProcessingRequest = false;
      const events = [...this.eventBuffer];
      this.eventBuffer = [];
      
      // Emit completion event
      this.emit('requestComplete', events);
      return;
    }

    if (!line.trim()) {
      return;
    }

    try {
      const event: AgentEvent = JSON.parse(line);
      this.handleEvent(event);
      this.eventBuffer.push(event);
    } catch (e) {
      console.warn('[MatrixCode Daemon] Failed to parse:', line);
    }
  }

  /**
   * Handle an AgentEvent
   */
  private handleEvent(event: AgentEvent): void {
    // Emit to specific handler
    const handler = this.eventHandlers.get(event.event_type);
    if (handler) {
      handler(event);
    }

    // Emit to general handler
    this.emit('event', event);
  }

  /**
   * Send a request to the daemon
   */
  async sendRequest(request: DaemonRequest): Promise<AgentEvent[]> {
    if (!this.process?.stdin) {
      await this.start();
    }

    if (!this.process?.stdin) {
      throw new Error('Daemon process not running');
    }

    return new Promise((resolve) => {
      // Store resolver
      const handler = (events: AgentEvent[]) => {
        resolve(events);
      };

      this.once('requestComplete', handler);

      // Send request
      const json = JSON.stringify(request);
      this.process!.stdin!.write(json + '\n');
      this.isProcessingRequest = true;
    });
  }

  /**
   * Send a chat message
   */
  async chat(content: string, context?: RequestContext): Promise<AgentEvent[]> {
    return this.sendRequest({
      type: 'chat',
      content,
      context,
    });
  }

  /**
   * Send a quick action
   */
  async quickAction(action: string, code: string, context?: RequestContext): Promise<AgentEvent[]> {
    return this.sendRequest({
      type: 'quick_action',
      action,
      content: code,
      context,
    });
  }

  /**
   * Get daemon status
   */
  async status(): Promise<AgentEvent[]> {
    return this.sendRequest({ type: 'status' });
  }

  /**
   * Create new session
   */
  async newSession(): Promise<void> {
    await this.sendRequest({ type: 'new_session' });
  }

  /**
   * Check if CLI is available
   */
  async checkAvailability(): Promise<boolean> {
    try {
      const result = await this.status();
      return result.some(e => e.event_type === 'session_started');
    } catch {
      return false;
    }
  }

  /**
   * Register event handler
   */
  on(eventType: string, handler: (event: AgentEvent) => void): void {
    this.eventHandlers.set(eventType, handler);
  }

  /**
   * Register one-time event handler
   */
  once(eventType: string, handler: (data: any) => void): void {
    const wrappedHandler = (data: any) => {
      this.eventHandlers.delete(eventType + '_once');
      handler(data);
    };
    this.eventHandlers.set(eventType + '_once', wrappedHandler as any);
  }

  /**
   * Emit event to handlers
   */
  private emit(eventType: string, data: any): void {
    const handler = this.eventHandlers.get(eventType);
    if (handler) {
      handler(data as AgentEvent);
    }
  }

  /**
   * Stop the daemon
   */
  stop(): void {
    if (this.process) {
      this.process.kill();
      this.process = null;
    }
  }

  /**
   * Cleanup
   */
  dispose(): void {
    this.stop();
    this.eventHandlers.clear();
  }
}