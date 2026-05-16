/**
 * MatrixCode CLI Client
 * Manages communication with the MatrixCode CLI process
 */

import * as vscode from 'vscode';
import { spawn, ChildProcess, execSync } from 'child_process';
import * as path from 'path';

export interface StreamEvent {
    type: 'text' | 'thinking' | 'tool_use' | 'tool_result' | 'error' | 'done' | 'session_started';
    content?: string;
    id?: string;
    name?: string;
    input?: unknown;
    tool_use_id?: string;
    usage?: { input: number; output: number };
}

export interface ClientRequest {
    type: 'chat' | 'quick_action' | 'new_session' | 'memory' | 'status';
    content?: string;
    action?: string;
    context?: RequestContext;
    instructions?: string;
}

export interface RequestContext {
    workspace?: string;
    file?: string;
    language?: string;
    selection?: Selection;
    diagnostics?: Diagnostic[];
}

export interface Selection {
    start: Position;
    end: Position;
}

export interface Position {
    line: number;
    character: number;
}

export interface Diagnostic {
    severity: string;
    message: string;
    range: Selection;
}

export interface Config {
    cliPath: string;
    provider: string;
    model: string;
    think: boolean;
    markdown: boolean;
    maxTokens: number;
    compressModel?: string;
    daemonMode: boolean;
}

export class MatrixCodeClient implements vscode.Disposable {
    private process: ChildProcess | null = null;
    private config: Config;
    private onEventEmitter = new vscode.EventEmitter<StreamEvent>();
    private onErrorEmitter = new vscode.EventEmitter<Error>();
    private buffer: string = '';
    private isStarting: boolean = false;
    
    public readonly onEvent = this.onEventEmitter.event;
    public readonly onError = this.onErrorEmitter.event;
    
    constructor(config: Config) {
        this.config = config;
    }
    
    updateConfig(config: Config): void {
        const wasRunning = this.process !== null;
        if (wasRunning) {
            this.dispose();
        }
        this.config = config;
        if (wasRunning && config.daemonMode) {
            this.startDaemon();
        }
    }
    
    async checkAvailability(): Promise<boolean> {
        try {
            const result = execSync(`"${this.config.cliPath}" --version`, {
                encoding: 'utf-8',
                timeout: 5000
            });
            return result.includes('matrixcode');
        } catch {
            // Try with 'matrixcode' directly if the path doesn't work
            try {
                const result = execSync('matrixcode --version', {
                    encoding: 'utf-8',
                    timeout: 5000
                });
                return result.includes('matrixcode');
            } catch {
                return false;
            }
        }
    }
    
    async startDaemon(): Promise<void> {
        if (this.process || this.isStarting) {
            return;
        }
        
        this.isStarting = true;
        
        return new Promise((resolve, reject) => {
            const args = this.buildDaemonArgs();
            
            console.log(`Starting MatrixCode daemon: ${this.config.cliPath} ${args.join(' ')}`);
            
            try {
                this.process = spawn(this.config.cliPath, args, {
                    stdio: ['pipe', 'pipe', 'pipe'],
                    env: { ...process.env }
                });
            } catch (err) {
                this.isStarting = false;
                reject(err);
                return;
            }
            
            this.process.on('error', (err) => {
                console.error('MatrixCode process error:', err);
                this.isStarting = false;
                this.onErrorEmitter.fire(err);
                reject(err);
            });
            
            this.process.stdout?.on('data', (data) => {
                this.handleStdout(data);
            });
            
            this.process.stderr?.on('data', (data) => {
                const msg = data.toString();
                // Non-JSON stderr is just log output
                console.log('[MatrixCode]', msg.trim());
            });
            
            this.process.on('exit', (code, signal) => {
                console.log(`MatrixCode process exited: code=${code}, signal=${signal}`);
                this.process = null;
                this.isStarting = false;
            });
            
            // Give the process a moment to start
            setTimeout(() => {
                this.isStarting = false;
                resolve();
            }, 500);
        });
    }
    
    private buildDaemonArgs(): string[] {
        const args: string[] = ['--daemon', '--json'];
        
        args.push('--provider', this.config.provider);
        args.push('--model', this.config.model);
        args.push('--max-tokens', String(this.config.maxTokens));
        
        if (this.config.think) {
            args.push('--think');
        } else {
            args.push('--think', 'false');
        }
        
        if (this.config.markdown) {
            args.push('--markdown');
        } else {
            args.push('--markdown', 'false');
        }
        
        if (this.config.compressModel) {
            args.push('--compress-model', this.config.compressModel);
        }
        
        return args;
    }
    
    private handleStdout(data: Buffer): void {
        this.buffer += data.toString();
        
        // Process complete JSON lines
        const lines = this.buffer.split('\n');
        this.buffer = lines.pop() || ''; // Keep incomplete line in buffer
        
        for (const line of lines) {
            if (line.trim()) {
                try {
                    const event: StreamEvent = JSON.parse(line);
                    this.onEventEmitter.fire(event);
                } catch (e) {
                    // Not a JSON line, could be a log message
                    console.log('[MatrixCode stdout]', line);
                }
            }
        }
    }
    
    async sendRequest(request: ClientRequest): Promise<void> {
        if (!this.process?.stdin) {
            throw new Error('MatrixCode daemon is not running');
        }
        
        const json = JSON.stringify(request) + '\n';
        this.process.stdin.write(json);
    }
    
    async chat(message: string, context?: RequestContext): Promise<void> {
        await this.sendRequest({
            type: 'chat',
            content: message,
            context
        });
    }
    
    async quickAction(
        action: string, 
        code: string, 
        context?: RequestContext, 
        instructions?: string
    ): Promise<void> {
        await this.sendRequest({
            type: 'quick_action',
            action,
            content: code,
            context,
            instructions
        });
    }
    
    async newSession(): Promise<void> {
        await this.sendRequest({ type: 'new_session' });
    }
    
    async getStatus(): Promise<void> {
        await this.sendRequest({ type: 'status' });
    }
    
    isRunning(): boolean {
        return this.process !== null && !this.isStarting;
    }
    
    dispose(): void {
        if (this.process) {
            this.process.kill();
            this.process = null;
        }
        this.buffer = '';
        this.onEventEmitter.dispose();
        this.onErrorEmitter.dispose();
    }
}