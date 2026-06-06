# LSP 管道错误根本原因分析

## 问题现象
- LSP 服务器启动后意外退出
- 出现 stdin/stdout 管道破裂错误
- 进程在初始化阶段卡住或失败

## 根本原因：stderr 缓冲区溢出

### 问题代码
```rust
// core/src/lsp/transport.rs:47-52
let mut cmd = Command::new(&actual_command);
cmd.args(&actual_args)
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())  // ← stderr 被设置为 piped
    .kill_on_drop(true);

// 但是！
// ❌ 没有代码去读取 stderr
// ❌ stderr.take() 从未被调用
// ❌ stderr 的缓冲区会填满，导致进程阻塞
```

### 为什么会导致进程退出？

1. **缓冲区有限**：操作系统为每个管道分配固定大小的缓冲区（通常 4KB-64KB）

2. **LSP 服务器行为**：
   - `rust-analyzer`：大量调试日志输出到 stderr
   - `typescript-language-server`：错误和警告输出到 stderr
   - 其他 LSP 服务器：初始化信息、诊断日志都可能输出到 stderr

3. **阻塞链式反应**：
   ```
   LSP 进程写入 stderr → 缓冲区填满 → write() 阻塞
   → 进程无法继续 → 初始化停滞 → 超时或崩溃
   → stdin/stdout 管道破裂 → 客户端收到 Broken pipe
   ```

4. **Windows 更严重**：
   - Windows 管道缓冲区较小
   - 某些 LSP 服务器在 Windows 上输出更多调试信息
   - `cmd.exe /c npx` 会产生额外的 stderr 输出

## 验证方法

### 手动测试
```bash
# 启动 rust-analyzer 并观察 stderr
rust-analyzer 2>&1 | head -100

# typescript-language-server
npx typescript-language-server --stdio 2>&1 | head -100
```

### 代码验证
```rust
// 添加临时代码检查 stderr 内容
let stderr = child.stderr.take();
tokio::spawn(async move {
    if let Some(stderr) = stderr {
        let mut reader = BufReader::new(stderr).lines();
        while let Some(line) = reader.next_line().await.ok().flatten() {
            log::warn!("LSP stderr: {}", line);  // ← 你会看到大量日志！
        }
    }
});
```

## 解决方案

### 方案 1：丢弃 stderr（最简单）
```rust
// transport.rs spawn()
cmd.args(&actual_args)
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null())  // ← 改为 null，直接丢弃
    .kill_on_drop(true);
```

**优点**：
- 最简单，立即解决问题
- 无额外开销

**缺点**：
- 丢失错误诊断信息
- 无法调试 LSP 服务器问题

### 方案 2：异步读取 stderr（推荐）
```rust
// transport.rs spawn()
let stderr = child.stderr.take()
    .map(|s| Box::new(s) as Box<dyn AsyncRead + Unpin + Send>);

// 启动后台任务持续读取 stderr
let stderr_reader = stderr.map(|s| BufReader::new(s).lines());
let server_name_clone = server_name.clone();

tokio::spawn(async move {
    if let Some(stderr_reader) = stderr_reader {
        use tokio::io::AsyncBufReadExt;
        let mut lines = stderr_reader;
        
        while let Ok(Some(line)) = lines.next_line().await {
            // 根据内容级别决定日志级别
            if line.contains("error") || line.contains("ERROR") {
                log::error!("LSP '{}' stderr: {}", server_name_clone, line);
            } else if line.contains("warn") || line.contains("WARN") {
                log::warn!("LSP '{}' stderr: {}", server_name_clone, line);
            } else {
                log::debug!("LSP '{}' stderr: {}", server_name_clone, line);
            }
        }
        
        log::info!("LSP '{}' stderr stream ended", server_name_clone);
    }
});
```

**优点**：
- 防止缓冲区溢出
- 保留错误诊断信息
- 可调试 LSP 服务器问题

**缺点**：
- 需要额外代码
- 有轻微性能开销

### 方案 3： stderr 转发到文件（调试用）
```rust
// 创建日志文件
let stderr_log = std::fs::File::create("lsp_stderr.log")?;
let stderr_log = std::process::Stdio::from(stderr_log);

cmd.stderr(stderr_log);  // ← 转发到文件
```

**优点**：
- 完整保留 stderr 输出
- 调试时非常有用

**缺点**：
- 文件可能变大
- 需要管理日志文件

## 推荐实施方案

### 立即修复（方案 1）
```rust
// transport.rs 第 51 行改为：
.stderr(std::process::Stdio::null())
```

### 完整方案（方案 2）
实现异步 stderr 读取器，包含：
1. 后台任务持续读取 stderr
2. 智能日志级别（error/warn/debug）
3. stderr 流结束检测
4. 添加到 LspTransport 结构体

## 其他常见原因

虽然 stderr 缓冲区是最常见原因，但还有其他可能性：

### 1. LSP 服务器配置错误
```json
// 检查配置
{
  "language": "rust",
  "command": "rust-analyzer",  // ← 命令是否存在？
  "args": []
}
```

**验证**：
```bash
rust-analyzer --version  # 检查是否安装
npx typescript-language-server --version  # Node.js LSP
```

### 2. Windows 路径问题
```rust
// 当前 Windows 兼容性处理
if cfg!(target_os = "windows") && (command == "npx" || command == "npm" || command == "node") {
    ("cmd.exe", vec!["/c", command, ...])
}
```

**问题**：
- `npx` 可能不在 PATH
- `cmd.exe /c` 的行为不一致
- Node.js 全局包路��问题

**解决**：
```bash
# Windows 检查
where npx
where rust-analyzer

# 或使用完整路径
C:\Users\{user}\AppData\Roaming\npm\npx.cmd
```

### 3. 进程启动超时
```rust
// constants.rs
pub const PROCESS_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
```

**如果二进制文件启动慢**：
- 首次运行 `npx` 需要下载包
- `rust-analyzer` 首次启动需要编译索引

**解决**：增加超时或预热
```rust
// 增加 10 秒启动超时
pub const PROCESS_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
```

### 4. 初始化参数错误
```rust
// client.rs initialize()
let root_uri = Url::from_file_path(&self.project_root)
    .map_err(|_| anyhow!("Invalid project root path: {:?}", self.project_root))?;
```

**Windows 路径问题**：
```
C:\Users\bigfish\Projects\matrixcode  // ← 反斜杠
file:///C:/Users/bigfish/Projects/matrixcode  // ← URL 格式
```

## 测试修复效果

### 修复前
```bash
# 运行 MatrixCode TUI
cargo run --features tui

# 观察 LSP 状态
# - 卡在 "starting..."
# - 超时失败
# - 管道错误
```

### 修复后（方案 2）
```bash
# 应该看到
[INFO] LSP 'rust-analyzer' stderr: Loading workspace...
[INFO] LSP 'rust-analyzer' stderr: Indexing 1234 files...
[INFO] LSP 'rust-analyzer' initialized successfully
[INFO] LSP client 'rust-analyzer' spawned and initialized successfully
```

## 总结

**核心问题**：stderr 缓冲区溢出导致进程阻塞和退出

**关键证据**：
1. `stderr(Stdio::piped())` 设置但从未读取
2. LSP 服务器输出大量 stderr 日志
3. 管道缓冲区有限（4KB-64KB）

**立即行动**：
- 改用 `Stdio::null()` 快速修复
- 或实现异步 stderr 读取器完整方案

**预期效果**：
- LSP 初始化成功率大幅提升
- Windows 环境稳定性改善
- 可调试 stderr 输出问题