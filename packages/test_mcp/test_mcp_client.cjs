#!/usr/bin/env node

/**
 * Simple MCP Test Client - Tests Playwright MCP
 */

const { spawn } = require('child_process');

// MCP server configuration (Windows)
const server = spawn('cmd.exe', ['/c', 'npx', '-y', '@playwright/mcp@latest'], {
  stdio: ['pipe', 'pipe', 'pipe']
});

let requestId = 0;
let buffer = '';

// Handle stdout (MCP responses)
server.stdout.on('data', (data) => {
  buffer += data.toString();
  
  // Process complete JSON lines
  const lines = buffer.split('\n');
  buffer = lines.pop(); // Keep incomplete line
  
  for (const line of lines) {
    if (line.trim()) {
      try {
        const msg = JSON.parse(line);
        handleMessage(msg);
      } catch (e) {
        console.log('Raw output:', line);
      }
    }
  }
});

// Handle stderr (logs)
server.stderr.on('data', (data) => {
  // Ignore npm install logs
});

// Handle process exit
server.on('close', (code) => {
  console.log('\n=== MCP Server closed ===');
  process.exit(0);
});

// Send JSON-RPC message
function send(method, params = {}) {
  const id = ++requestId;
  const msg = JSON.stringify({
    jsonrpc: '2.0',
    id,
    method,
    params
  });
  
  console.log(`>>> Sending: ${method}`);
  server.stdin.write(msg + '\n');
  
  return id;
}

// Send notification (no response expected)
function notify(method, params = {}) {
  const msg = JSON.stringify({
    jsonrpc: '2.0',
    method,
    params
  });
  
  console.log(`>>> Notification: ${method}`);
  server.stdin.write(msg + '\n');
}

// Handle received message
function handleMessage(msg) {
  if (msg.result) {
    console.log(`<<< Result (id=${msg.id}):`);
    
    if (msg.result.tools) {
      console.log(`    Found ${msg.result.tools.length} tools:`);
      msg.result.tools.forEach((tool, i) => {
        console.log(`    ${i+1}. ${tool.name} - ${tool.description.slice(0, 60)}...`);
      });
    } else if (msg.result.serverInfo) {
      console.log(`    Server: ${msg.result.serverInfo.name} v${msg.result.serverInfo.version}`);
      console.log(`    Protocol: ${msg.result.protocolVersion}`);
    } else {
      console.log('    ', JSON.stringify(msg.result).slice(0, 100));
    }
  } else if (msg.error) {
    console.log(`<<< Error: ${msg.error.message}`);
  } else if (msg.method) {
    console.log(`<<< Notification: ${msg.method}`);
  }
}

// Test sequence
async function runTest() {
  console.log('=== MCP Playwright Test ===\n');
  
  // 1. Initialize
  send('initialize', {
    protocolVersion: '2024-11-05',
    capabilities: { roots: { listChanged: false } },
    clientInfo: { name: 'test-client', version: '1.0' }
  });
  
  await sleep(2000);
  
  // 2. Send initialized notification
  notify('notifications/initialized');
  
  await sleep(1000);
  
  // 3. List tools
  send('tools/list');
  
  await sleep(3000);
  
  // 4. Shutdown
  console.log('\n=== Test Complete ===');
  server.kill();
}

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

// Start test after 1 second
setTimeout(runTest, 1000);

// Timeout after 30 seconds
setTimeout(() => {
  console.log('\nTimeout - shutting down');
  server.kill();
  process.exit(1);
}, 30000);