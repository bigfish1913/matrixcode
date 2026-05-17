/**
 * Configuration Manager
 * Handles VSCode settings for MatrixCode
 */

import * as vscode from 'vscode';

export class ConfigManager {
    private config: vscode.WorkspaceConfiguration;
    
    constructor() {
        this.config = vscode.workspace.getConfiguration('matrixcode');
    }
    
    reload(): void {
        this.config = vscode.workspace.getConfiguration('matrixcode');
    }
    
    get<T>(key: string): T {
        return this.config.get<T>(key)!;
    }
    
    getCliPath(): string {
        return this.config.get<string>('cliPath') || 'matrixcode';
    }
    
    getProvider(): string {
        return this.config.get<string>('provider') || 'anthropic';
    }
    
    getModel(): string {
        return this.config.get<string>('model') || '';
    }
    
    getThink(): boolean {
        return this.config.get<boolean>('think') ?? true;
    }
    
    getMarkdown(): boolean {
        return this.config.get<boolean>('markdown') ?? true;
    }
    
    getMaxTokens(): number {
        return this.config.get<number>('maxTokens') || 16384;
    }
    
    getCompressModel(): string | undefined {
        return this.config.get<string>('compressModel') || undefined;
    }
    
    getDaemonMode(): boolean {
        return this.config.get<boolean>('daemonMode') ?? true;
    }
    
    getAutoContext(): boolean {
        return this.config.get<boolean>('autoContext') ?? true;
    }
    
    getShowThinking(): boolean {
        return this.config.get<boolean>('showThinking') ?? true;
    }
    
    getShowToolUse(): boolean {
        return this.config.get<boolean>('showToolUse') ?? true;
    }
    
    toClientConfig(): {
        cliPath: string;
        provider: string;
        model: string;
        think: boolean;
        markdown: boolean;
        maxTokens: number;
        compressModel?: string;
        daemonMode: boolean;
    } {
        return {
            cliPath: this.getCliPath(),
            provider: this.getProvider(),
            model: this.getModel(),
            think: this.getThink(),
            markdown: this.getMarkdown(),
            maxTokens: this.getMaxTokens(),
            compressModel: this.getCompressModel(),
            daemonMode: this.getDaemonMode()
        };
    }
    
    async set(key: string, value: unknown): Promise<void> {
        await this.config.update(key, value, vscode.ConfigurationTarget.Global);
    }
}