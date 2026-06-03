# LSP 修复测试说明

## 问题诊断

**根本原因**: `lsp_handler.rs:95` 在 `start_all()` 中错误地立即标记所有服务器为 connected，但后台任务还没完成注册。

**症状**: 
- LSP 工具调用返回 "启动超时（180秒）"
- rust-analyzer 进程运行但未注册到 registry
- 状态不一致：manager 显示 connected，registry 中无客户端

## 修复内容

### 1. `cli/src/terminal/lsp_handler.rs`

**修复前**: 
```rust
pub async fn start_all(&self, event_tx: &...) {
    let manager = self.manager.write().await;
    let servers = manager.server_infos();
    
    // ❌ 错误：立即标记为 connected
    for server in &servers {
        manager.mark_connected(&server.language);
    }
    
    // 发送状态（虚假的 "connected"）
    let _ = event_tx.send(AgentEvent::lsp_server_status(servers)).await;
}
```

**修复后**: 
```rust
pub async fn start_all(&self, event_tx: &...) {
    let manager = self.manager.read().await;  // ✅ 只读
    
    // 获取当前状态（应该是 "starting"）
    let servers = manager.server_infos();
    
    // ✅ 不修改状态，让后台任务完成后再更新
    let _ = event_tx.send(AgentEvent::lsp_server_status(servers)).await;
}
```

### 2. 后台任务优化

**修复前**: 持有写锁时间过长
```rust
let mgr = manager.write().await;  // ❌ 在 match 外获取锁
match start_result {
    Ok(Ok(_)) => mgr.mark_connected(&language),
    ...
}
```

**修复后**: 只在需要时获取锁
```rust
match start_result {
    Ok(Ok(_)) => manager.write().await.mark_connected(&language),  // ✅ 短暂持有
    Ok(Err(e)) => manager.write().await.mark_error(&language, e.to_string()),
    Err(_) => manager.write().await.mark_error(&language, "Startup timeout"),
}
```

## 验证步骤

### 1. 重新编译
```bash
cd packages/cli
cargo build --release
```

### 2. 清理旧进程
```bash
taskkill /F /IM rust-analyzer.exe
```

### 3. 启动 matrixcode
```bash
./target/release/matrixcode
```

### 4. 观察日志
查看 debug log：
- `Background: starting 'rust-analyzer'...`
- `Background: 'rust-analyzer' started OK` (或失败/超时)

### 5. 测试 LSP 工具
等待 2-3 分钟后测试：
```rust
lsp_hover(file="packages/core/src/lib.rs", line=33, column=8)
lsp_diagnostics(file="packages/core/src/lib.rs")
```

## 预期结果

✅ **正确流程**:
1. CLI 启动 → LSP 配置加载
2. `add_servers()` → 标记为 "starting"，启动后台任务
3. `start_all()` → 发送 "starting" 状态给 UI
4. 后台任务完成 → registry 注册成功 → manager ��新为 "connected"
5. LSP 工具可用

❌ **之前的错误流程**:
1. CLI 启动 → LSP 配置加载
2. `add_servers()` → 标记为 "starting"，启动后台任务
3. `start_all()` → **立即标记为 "connected"**（错误！）
4. UI 显示 "connected"，但 registry 中无客户端
5. LSP 工具调用 → registry 等待客户端 → 180s 超时

## 编译结果

✅ 编译通过：`cargo check` 成功，无错误

## 下一步

重新启动 matrixcode 并测试 LSP 功能。预计索引时间：
- 小型项目：30-60 秒
- matrixcode 项目：2-5 分钟（~600 文件）