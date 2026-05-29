/**
 * MatrixCode VSCode Extension - Terminal Launcher
 * 
 * A simplified extension that launches MatrixCode CLI in VSCode's integrated terminal.
 */

import * as vscode from 'vscode';

let statusBarItem: vscode.StatusBarItem;
let modelStatusBarItem: vscode.StatusBarItem;
let matrixcodeTerminal: vscode.Terminal | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    try {
        const outputChannel = vscode.window.createOutputChannel('MatrixCode');
        context.subscriptions.push(outputChannel);
        
        outputChannel.appendLine('MatrixCode extension is activating...');
        
        // Register commands
        registerCommands(context);
        
        // Add status bar button
        statusBarItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Left,
            100
        );
        statusBarItem.text = '◈ MatrixCode';
        statusBarItem.tooltip = 'Open MatrixCode in Terminal (Ctrl+K)';
        statusBarItem.command = 'matrixcode.openChat';
        statusBarItem.show();
        context.subscriptions.push(statusBarItem);
        
        // Add model status bar item
        modelStatusBarItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Right,
            200
        );
        const config = vscode.workspace.getConfiguration('matrixcode');
        const model = config.get<string>('model', '');
        const provider = config.get<string>('provider', 'anthropic');
        modelStatusBarItem.text = '$(hub) ' + getModelDisplayName(model, provider);
        modelStatusBarItem.tooltip = 'Current Model: ' + (model || 'Default');
        modelStatusBarItem.command = 'matrixcode.openSettings';
        modelStatusBarItem.show();
        context.subscriptions.push(modelStatusBarItem);
        
        // Listen for configuration changes
        context.subscriptions.push(
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration('matrixcode')) {
                    const newConfig = vscode.workspace.getConfiguration('matrixcode');
                    const newModel = newConfig.get<string>('model', '');
                    const newProvider = newConfig.get<string>('provider', 'anthropic');
                    modelStatusBarItem.text = '$(hub) ' + getModelDisplayName(newModel, newProvider);
                    modelStatusBarItem.tooltip = 'Current Model: ' + (newModel || 'Default');
                }
            })
        );
        
        // Handle terminal close
        context.subscriptions.push(
            vscode.window.onDidCloseTerminal(closedTerminal => {
                if (closedTerminal === matrixcodeTerminal) {
                    matrixcodeTerminal = undefined;
                }
            })
        );
        
        outputChannel.appendLine('MatrixCode extension activated successfully!');
        console.log('MatrixCode extension activated');
        
    } catch (error) {
        const errMsg = error instanceof Error ? error.message : String(error);
        console.error('MatrixCode activation failed:', errMsg);
        vscode.window.showErrorMessage(`MatrixCode activation failed: ${errMsg}`);
    }
}

function registerCommands(context: vscode.ExtensionContext): void {
    // Open MatrixCode in terminal
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.openChat', () => {
            openMatrixCodeTerminal();
        })
    );
    
    // Quick action - open terminal with context
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
            
            const question = await vscode.window.showInputBox({
                prompt: 'Ask about the selected code',
                placeHolder: 'e.g., What does this function do? How can I optimize it?'
            });
            
            if (!question) {
                return;
            }
            
            openMatrixCodeTerminal(question);
        })
    );
    
    // Explain code
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.explain', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.selection.isEmpty) {
                vscode.window.showWarningMessage('Please select some code to explain');
                return;
            }
            openMatrixCodeTerminal('Explain this code');
        })
    );
    
    // Fix code
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.fix', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.selection.isEmpty) {
                vscode.window.showWarningMessage('Please select some code to fix');
                return;
            }
            openMatrixCodeTerminal('Fix this code');
        })
    );
    
    // Generate tests
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.generateTests', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }
            openMatrixCodeTerminal('Generate tests for this code');
        })
    );
    
    // Refactor
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.refactor', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.selection.isEmpty) {
                vscode.window.showWarningMessage('Please select some code to refactor');
                return;
            }
            
            const instructions = await vscode.window.showInputBox({
                prompt: 'Refactor instructions (optional)',
                placeHolder: 'e.g., Extract method, Rename variables, etc.'
            });
            
            openMatrixCodeTerminal(instructions ? `Refactor: ${instructions}` : 'Refactor this code');
        })
    );
    
    // Improve code
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.improve', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.selection.isEmpty) {
                vscode.window.showWarningMessage('Please select some code to improve');
                return;
            }
            openMatrixCodeTerminal('Improve this code');
        })
    );
    
    // New session
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.newSession', () => {
            // Close existing terminal if any
            if (matrixcodeTerminal) {
                matrixcodeTerminal.dispose();
                matrixcodeTerminal = undefined;
            }
            // Open new terminal for new session
            openMatrixCodeTerminal();
            vscode.window.showInformationMessage('MatrixCode: Started new session');
        })
    );
    
    // Open settings
    context.subscriptions.push(
        vscode.commands.registerCommand('matrixcode.openSettings', () => {
            vscode.commands.executeCommand('workbench.action.openSettings', 'matrixcode');
        })
    );
}

/**
 * Open or focus MatrixCode terminal
 */
function openMatrixCodeTerminal(initialPrompt?: string): void {
    const config = vscode.workspace.getConfiguration('matrixcode');
    const cliPath = config.get<string>('cliPath', 'matrixcode');
    const provider = config.get<string>('provider', 'anthropic');
    const model = config.get<string>('model', '');
    const think = config.get<boolean>('think', true);
    const maxTokens = config.get<number>('maxTokens', 16384);
    
    // Build command arguments
    const args: string[] = [];
    
    if (provider) {
        args.push('--provider', provider);
    }
    
    if (model) {
        args.push('--model', model);
    }
    
    if (think) {
        args.push('--think');
    }
    
    if (maxTokens) {
        args.push('--max-tokens', maxTokens.toString());
    }
    
    // Check if terminal already exists
    if (matrixcodeTerminal) {
        // Focus existing terminal
        matrixcodeTerminal.show();
        
        // Send prompt if provided
        if (initialPrompt) {
            // For existing terminal, just send the text (user needs to press Enter)
            matrixcodeTerminal.sendText(initialPrompt);
        }
    } else {
        // Create new terminal
        const terminalOptions: vscode.TerminalOptions = {
            name: 'MatrixCode',
            shellPath: process.env.SHELL || (process.platform === 'win32' ? 'cmd.exe' : 'bash'),
        };
        
        matrixcodeTerminal = vscode.window.createTerminal(terminalOptions);
        matrixcodeTerminal.show();
        
        // Run matrixcode command
        const fullCommand = `${cliPath} ${args.join(' ')}`;
        matrixcodeTerminal.sendText(fullCommand);
        
        // Send initial prompt after command starts
        if (initialPrompt) {
            setTimeout(() => {
                matrixcodeTerminal?.sendText(initialPrompt);
            }, 500);
        }
    }
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

export function deactivate(): void {
    if (matrixcodeTerminal) {
        matrixcodeTerminal.dispose();
    }
    if (statusBarItem) {
        statusBarItem.dispose();
    }
    if (modelStatusBarItem) {
        modelStatusBarItem.dispose();
    }
    console.log('MatrixCode extension deactivated');
}