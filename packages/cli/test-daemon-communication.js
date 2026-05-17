/**
 * Test script to simulate VSCode extension daemon communication
 * This helps diagnose where the communication breaks down
 */

const { spawn } = require('child_process');
const path = require('path');

console.log('=== Simulating VSCode Extension Daemon Communication ===\n');

// Step 1: Simulate extension activation
console.log('Step 1: Extension activation');
console.log('  - Creating OutputChannel');
console.log('  - Initializing ConfigManager');
console.log('  - daemonMode = true (default)');
console.log('  - Creating MatrixCodeClient');

// Step 2: Start daemon
console.log('\nStep 2: Starting daemon');
const args = [
  '--daemon',
  '--json',
  '--provider', 'anthropic',
  '--model', 'claude-sonnet-4-20250514',
  '--max-tokens', '4096',
  '--think', 'false',
  '--markdown', 'false'
];

console.log('  - Command: matrixcode', args.join(' '));

const proc = spawn('matrixcode', args, {
  stdio: ['pipe', 'pipe', 'pipe'],
  env: { ...process.env }
});

let eventCount = 0;
let buffer = '';

// Step 3: Setup event listeners (simulating MatrixCodeClient)
console.log('\nStep 3: Setting up event listeners');
console.log('  - Listening to stdout');
console.log('  - Listening to stderr');

proc.stdout.on('data', (data) => {
  const str = data.toString();
  console.log('  ✅ STDOUT received:', str.length, 'bytes');
  
  // Simulate handleStdout
  buffer += str;
  const lines = buffer.split('\n');
  buffer = lines.pop() || '';
  
  for (const line of lines) {
    if (line.trim()) {
      try {
        const event = JSON.parse(line);
        eventCount++;
        console.log(`  📦 Event #${eventCount}: type=${event.type}`);
        
        // Simulate onEventEmitter.fire(event)
        handleEvent(event);
      } catch (e) {
        console.log('  📋 Non-JSON line:', line.substring(0, 50));
      }
    }
  }
});

proc.stderr.on('data', (data) => {
  console.log('  ⚠️ STDERR:', data.toString().trim().substring(0, 80));
});

proc.on('error', (err) => {
  console.error('  ❌ Process error:', err);
});

proc.on('exit', (code, signal) => {
  console.log(`\n  🛑 Process exited: code=${code}, signal=${signal}`);
  console.log(`  Total events received: ${eventCount}`);
});

// Step 4: Wait for daemon startup then send request
setTimeout(() => {
  console.log('\nStep 4: Sending chat request');
  
  const request = { type: 'chat', content: 'hello from test' };
  const json = JSON.stringify(request) + '\n';
  
  console.log('  - Request:', JSON.stringify(request));
  console.log('  - Writing to stdin...');
  
  const written = proc.stdin.write(json);
  console.log('  - Write result:', written);
  
}, 3000);

// Step 5: Cleanup after test
setTimeout(() => {
  console.log('\nStep 5: Cleanup');
  console.log('  - Closing stdin');
  proc.stdin.end();
  
  setTimeout(() => {
    proc.kill();
    console.log('\n=== Test Complete ===');
    console.log(`Total events received: ${eventCount}`);
    
    if (eventCount > 0) {
      console.log('✅ Communication working - events were received');
    } else {
      console.log('❌ Communication broken - no events received');
    }
  }, 1000);
}, 10000);

// Simulate ChatPanel handleStreamEvent
function handleEvent(event) {
  // This simulates what ChatPanelProvider.handleStreamEvent should do
  switch (event.type) {
    case 'session_started':
      console.log('    → ChatPanel would: log session started');
      break;
    case 'text':
      console.log('    → ChatPanel would: append text to message');
      break;
    case 'thinking':
      console.log('    → ChatPanel would: show thinking (if enabled)');
      break;
    case 'done':
      console.log('    → ChatPanel would: mark message as complete');
      break;
    case 'error':
      console.log('    → ChatPanel would: show error message');
      console.log('    → Error content:', event.content || event.message);
      break;
    default:
      console.log('    → ChatPanel would: handle', event.type);
  }
}