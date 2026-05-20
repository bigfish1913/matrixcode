/**
 * MatrixCode VSCode Extension Entry Point
 * 
 * This extension provides an AI code agent in a dedicated editor tab,
 * similar to Claude Code's approach.
 */

import * as vscode from 'vscode';
import { MatrixCodeClient } from './matrixcodeClient';
import { ChatPanelProvider } from './chatPanel';
import { ConfigManager } from './configManager';
import { SessionManager } from './sessionManager';
import { EditorContext } from './types';

let client: MatrixCodeClient;
let chatPanel: ChatPanelProvider;
let configManager: ConfigManager;
let statusBarItem: vscode.StatusBarItem;
let modelStatusBarItem: vscode.StatusBarItem;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    try {
        // Create output channel for debugging
        const outputChannel = vscode.window.createOutputChannel('MatrixCode');
        context.subscriptions.push(outputChannel);
        
        outputChannel.appendLine('MatrixCode extension is activating...');
        outputChannel.appendLine(`Extension path: ${context.extensionPath}`);
        
        // Initialize configuration manager
        configManager = new ConfigManager();
        const sessionManager = new SessionManager(context);
        outputChannel.appendLine('ConfigManager initialized');
        
        // Initialize CLI client with config object
        client = new MatrixCodeClient({
            cliPath: configManager.getCliPath(),
            provider: configManager.getProvider(),
            model: configManager.getModel(),
            think: configManager.getThink(),
            markdown: configManager.getMarkdown(),
            maxTokens: configManager.getMaxTokens(),
            compressModel: configManager.getCompressModel(),
            daemonMode: configManager.getDaemonMode()
        });
        outputChannel.appendLine('MatrixCodeClient initialized');
        outputChannel.appendLine(`Config: cliPath=${configManager.getCliPath()}, provider=${configManager.getProvider()}, model=${configManager.getModel()}`);
        
        // Initialize chat panel provider
        chatPanel = new ChatPanelProvider(context.extensionUri, client, configManager, sessionManager, outputChannel);
        outputChannel.appendLine('ChatPanelProvider initialized');
        
        // Register all commands
        registerCommands(context);
        outputChannel.appendLine('Commands registered');
        
        // Check CLI availability
        const available = await client.checkAvailability();
        outputChannel.appendLine(`CLI availability: ${available}`);
        
        if (!available) {
            const result = await vscode.window.showWarningMessage(
                'MatrixCode CLI not found. The extension requires the MatrixCode CLI to function.',
                'Install CLI',
                'Configure Path',
                'Ignore'
            );
            
            handleCliNotFound(result);
        } else {
            outputChannel.appendLine('MatrixCode CLI found, starting daemon...');
            
            // Start daemon if enabled
            if (configManager.get('daemonMode')) {
                try {
                    await client.startDaemon();
                    vscode.window.setStatusBarMessage('$(check) MatrixCode connected', 3000);
                    outputChannel.appendLine('Daemon started successfully');
                } catch (error) {
                    const errMsg = error instanceof Error ? error.message : String(error);
                    outputChannel.appendLine(`Failed to start daemon: ${errMsg}`);
                    vscode.window.showErrorMessage(`Failed to start MatrixCode daemon: ${errMsg}`);
                }
            }
        }
        
        // Listen for configuration changes
        context.subscriptions.push(
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration('matrixcode')) {
                    configManager.reload();
                    client.updateConfig(configManager);
                }
            })
        );
        
        // Add status bar button with custom icon
        statusBarItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Left,
            100
        );
        
        // Use the matrix grid icon representation
        statusBarItem.text = '◈ MatrixCode';
        statusBarItem.tooltip = 'Open MatrixCode AI Chat (Ctrl+K)';
        statusBarItem.command = 'matrixcode.openChat';
        statusBarItem.show();
        context.subscriptions.push(statusBarItem);
        
        // Add model status bar item (right side)
        modelStatusBarItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Right,
            200
        );
        modelStatusBarItem.text = '$(hub) ' + getModelDisplayName(configManager.getModel(), configManager.getProvider());
        modelStatusBarItem.tooltip = 'Current Model: ' + (configManager.getModel() || 'Default');
        modelStatusBarItem.command = 'matrixcode.openSettings';
        modelStatusBarItem.show();
        context.subscriptions.push(modelStatusBarItem);
        
        outputChannel.appendLine('StatusBar items added');
        
        outputChannel.appendLine('MatrixCode extension activated successfully!');
        console.log('MatrixCode extension activated');
        
    } catch (error) {
        const errMsg = error instanceof Error ? error.message : String(error);
        console.error('MatrixCode activation failed:', errMsg);
        vscode.window.showErrorMessage(`MatrixCode activation failed: ${errMsg}`);
    }
}

function registerCommands(context: vscode.ExtensionContext): void {
    // Open chat in a new tab
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.openChat', () => {
            chatPanel.openOrCreate();
        })
    );
    
    // Quick action command (Ctrl+Shift+K)
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.quickAction', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            
            const selection = editor.selection;
            if (selection.isEmpty) {
                vscode.window.showWarningMessage('Please select some code first');
                return;
            }
            
            // Prompt for question
            const question = await vscode.window.showInputBox({
                prompt: 'Ask about the selected code',
                placeHolder: 'e.g., What does this function do? How can I optimize it?'
            });
            
            if (!question) {
                return;
            }
            
            const text = editor.document.getText(selection);
            const ctx = buildEditorContext(editor);
            
            // Open chat panel and send question
            chatPanel.openOrCreate();
            await chatPanel.sendQuickAction('ask', text, ctx, question);
        })
    );
    
    // Improve code command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.improve', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            
            const selection = editor.selection;
            if (selection.isEmpty) {
                vscode.window.showWarningMessage('Please select some code to improve');
                return;
            }
            
            const text = editor.document.getText(selection);
            const ctx = buildEditorContext(editor);
            
            chatPanel.openOrCreate();
            await chatPanel.sendQuickAction('improve', text, ctx);
        })
    );
    
    // Explain code command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.explain', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            
            const selection = editor.selection;
            if (selection.isEmpty) {
                vscode.window.showWarningMessage('Please select some code to explain');
                return;
            }
            
            const text = editor.document.getText(selection);
            const ctx = buildEditorContext(editor);
            
            // Open chat panel first
            chatPanel.openOrCreate();
            
            // Send quick action
            await chatPanel.sendQuickAction('explain', text, ctx);
        })
    );
    
    // Fix code command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.fix', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            
            const selection = editor.selection;
            if (selection.isEmpty) {
                vscode.window.showWarningMessage('Please select some code to fix');
                return;
            }
            
            const text = editor.document.getText(selection);
            const ctx = buildEditorContext(editor);
            
            // Add diagnostics to context
            const diagnostics = vscode.languages.getDiagnostics(editor.document.uri);
            if (diagnostics.length > 0) {
                const relevantDiagnostics = diagnostics.filter(d => {
                    const range = d.range;
                    return selection.contains(range.start) || selection.contains(range.end);
                });
                if (relevantDiagnostics.length > 0) {
                    ctx.diagnostics = relevantDiagnostics.map(d => ({
                        severity: vscode.DiagnosticSeverity[d.severity].toLowerCase(),
                        message: d.message,
                        range: {
                            start: { line: d.range.start.line, character: d.range.start.character },
                            end: { line: d.range.end.line, character: d.range.end.character }
                        }
                    }));
                }
            }
            
            // Open chat panel first
            chatPanel.openOrCreate();
            
            // Send quick action
            await chatPanel.sendQuickAction('fix', text, ctx);
        })
    );
    
    // Generate tests command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.generateTests', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            
            const selection = editor.selection;
            const text = selection.isEmpty 
                ? editor.document.getText() 
                : editor.document.getText(selection);
            const ctx = buildEditorContext(editor);
            
            // Open chat panel first
            chatPanel.openOrCreate();
            
            // Send quick action
            await chatPanel.sendQuickAction('generateTests', text, ctx);
        })
    );
    
    // Refactor command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.refactor', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            
            const selection = editor.selection;
            if (selection.isEmpty) {
                vscode.window.showWarningMessage('Please select some code to refactor');
                return;
            }
            
            const text = editor.document.getText(selection);
            const ctx = buildEditorContext(editor);
            
            // Prompt for refactor instructions
            const instructions = await vscode.window.showInputBox({
                prompt: 'Refactor instructions (optional)',
                placeHolder: 'e.g., Extract method, Rename variables, etc.'
            });
            
            // Open chat panel first
            chatPanel.openOrCreate();
            
            // Send quick action
            await chatPanel.sendQuickAction('refactor', text, ctx, instructions);
        })
    );
    
    // New session command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.newSession', async () => {
            chatPanel.clearHistory();
            vscode.window.showInformationMessage('MatrixCode: Started new session');
        })
    );
    
    // Open settings command
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.openSettings', () => {
            vscode.commands.executeCommand('workbench.action.openSettings', 'matrixcode');
        })
    );
}

function getModelDisplayName(model: string, provider: string): string {
    if (!model) {
        return provider === 'anthropic' ? 'Claude' : 'GPT';
    }
    // Shorten model name for display
    if (model.includes('claude')) {
        return model.replace('claude-', '').split('-').slice(0, 2).join('-');
    }
    if (model.includes('gpt')) {
        return model.replace('gpt-', '');
    }
    return model.substring(0, 15);
}

function buildEditorContext(editor: vscode.TextEditor): EditorContext {
    const document = editor.document;
    const selection = editor.selection;
    
    return {
        workspace: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
        file: document.uri.fsPath,
        language: document.languageId,
        selection: {
            start: { line: selection.start.line, character: selection.start.character },
            end: { line: selection.end.line, character: selection.end.character }
        }
    };
}

async function handleCliNotFound(result: string | undefined): Promise<void> {
    switch (result) {
        case 'Install CLI':
            await vscode.env.openExternal(
                vscode.Uri.parse('https://github.com/bigfish1913/matrixcode#installation')
            );
            break;
        case 'Configure Path':
            await vscode.commands.executeCommand('workbench.action.openSettings', 'matrixcode.cliPath');
            break;
        case 'Ignore':
            break;
    }
}

export function deactivate(): void {
    if (client) {
        client.dispose();
    }
    if (chatPanel) {
        chatPanel.dispose();
    }
    if (statusBarItem) {
        statusBarItem.dispose();
    }
    if (modelStatusBarItem) {
        modelStatusBarItem.dispose();
    }
    console.log('MatrixCode extension deactivated');
}

