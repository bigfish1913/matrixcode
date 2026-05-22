# MatrixCode 安全边界说明

**版本**: v0.4.7  
**更新日期**: 2025年1月22日

---

## 🔒 安全防护概览

MatrixCode 采用多层安全防护机制，在保证功能完整性的同时防止危险操作。

### 防护层级

```
┌─────────────────────────────────────────────┐
│  第1层：命令黑名单 (bash工具)                │
│  - 阻止30+种危险命令                        │
│  - 路径穿越检测                             │
│  - 系统文件保护                             │
└─────────────────────────────────────────────┘
┌─────────────────────────────────────────────┐
│  第2层：路径验证 (write/edit工具)           │
│  - 阻止路径穿越 (../)                       │
│  - 阻止系统文件写入                         │
│  - 路径长度限制 (1024字符)                  │
└─────────────────────────────────────────────┘
┌─────────────────────────────────────────────┐
│  第3层：内容大小限制                         │
│  - 单次写入最大10MB                         │
│  - 防止意外超大操作                         │
└─────────────────────────────────────────────┘
┌─────────────────────────────────────────────┐
│  第4层：迭代次数限制                         │
│  - MAX_ITERATIONS = 50                      │
│  - 防止无限循环                             │
│  - 达到上限时明确提示                       │
└─────────────────────────────────────────────┘
```

---

## 🚫 阻止的操作

### 1. 系统破坏命令 (bash工具)

**完全阻止**:
```bash
# 文件系统破坏
rm -rf /                   ❌ 阻止：删除根目录
rm -rf /*                  ❌ 阻止：删除根目录所有文件
rm -rf --no-preserve-root  ❌ 阻止：强制删除保护
mkfs                       ❌ 阻止：格式化磁盘
dd if=/dev/zero of=/dev/   ❌ 阻止：磁盘覆写

# 权限破坏
chmod 777 /                ❌ 阻止：全开放权限
chmod -R 777 /             ❌ 阻止：递归全开放
chown -R root:root /       ❌ 阻止：修改所有者

# 系统控制
shutdown                   ❌ 阻止：关机
reboot                     ❌ 阻止：重启
halt                       ❌ 阻止：停止系统
poweroff                   ❌ 阻止：断电
init 0                     ❌ 阻止：关机
init 6                     ❌ 阻止：重启

# 危险脚本
:(){:|:&};:               ❌ 阻止：Fork炸弹
wget | sh                  ❌ 阻止：下载执行
curl | bash                ❌ 阻止：下载执行
wget | sudo                ❌ 阻止：下载提权
```

**条件阻止** (路径穿越):
```bash
rm -rf ../../etc          ❌ 阻止：路径穿越
chmod 777 ../../../var    ❌ 阻止：路径穿越
chown -R root ../root     ❌ 阻止：路径穿越
```

**白名单允许** (安全路径):
```bash
rm -rf /tmp/test          ✅ 允许：安全临时目录
rm -rf /var/tmp/cache     ✅ 允许：安全临时目录
rm -rf /home/user/project ✅ 允许：用户项目目录
rm -rf ./build            ✅ 允许：相对路径（无穿越）
rm -rf build/             ✅ 允许：相对路径（无穿越）
```

### 2. 系统文件写入 (write/edit工具)

**完全阻止**:
```bash
write("/etc/passwd", ...)      ❌ 阻止：关键系统文件
write("/etc/shadow", ...)      ❌ 阻止：密码文件
write("/etc/sudoers", ...)     ❌ 阻止：权限配置
write("/boot/vmlinuz", ...)    ❌ 阻止：启动文件
write("/dev/sda", ...)         ❌ 阻止：磁盘设备
write("/proc/self/mem", ...)   ❌ 阻止：内存访问
```

**路径穿越阻止**:
```bash
write("../../etc/passwd", ...)     ❌ 阻止：路径穿越
write("../../../windows/system", ...) ❌ 阻止：路径穿越
write("/tmp/../etc/passwd", ...)   ❌ 阻止：路径穿越
```

**安全路径允许**:
```bash
write("./src/main.rs", ...)       ✅ 允许：项目相对路径
write("config.json", ...)         ✅ 允许：项目文件
write("/tmp/output.txt", ...)     ✅ 允许：临时目录（文档说明风险）
```

### 3. 内容大小限制 (write工具)

**超过限制阻止**:
```bash
write("large.txt", "内容超过10MB") ❌ 阻止：超大内容
```

**错误消息**:
```
Content too large: 15,000,000 bytes (max: 10,000,000 bytes = 10 MB).
Split into smaller files or use streaming
```

**建议**: 大文件请分批写入或使用流式写入。

### 4. 迭代次数限制 (Agent)

**达到上限提示**:
```
⚠️ Reached maximum iterations limit (50 iterations).

**Task status**: The task may not be fully complete.

**What happened**: Agent stopped after 50 iterations to prevent infinite loops.

**Next steps**:
1. Check if the task is complete
2. If incomplete, you can:
   - Continue with more specific instructions
   - Break down the task into smaller subtasks
   - Use '/resume' to continue from current state

**Why this limit exists**: Prevents runaway operations and resource exhaustion.
**Adjustable**: Future versions will allow custom iteration limits.
```

---

## ✅ 允许的操作

### 安全命令执行

**文件操作** (无穿越):
```bash
cat README.md              ✅ 允许：读取文件
ls src/                    ✅ 允许：列出目录
grep "pattern" file.txt    ✅ 允许：搜索内容
find . -name "*.rs"        ✅ 允许：查找文件
mkdir build                ✅ 允许：创建目录
rm file.txt                ✅ 允许：删除单个文件（非强制递归）
```

**构建操作**:
```bash
cargo build                ✅ 允许：构建项目
npm install                ✅ 允许：安装依赖
make                       ✅ 允许：编译项目
git status                 ✅ 允任：Git操作
```

**开发工具**:
```bash
rustc main.rs              ✅ 允许：编译Rust
python script.py           ✅ 允许：运行Python
node app.js                ✅ 允许：运行Node
docker ps                  ✅ 允许：查看容器
```

### 安���文件写入

**项目文件**:
```bash
write("src/main.rs", code)      ✅ 允许：项目源码
write("config.json", data)      ✅ 允许：配置文件
write("README.md", doc)         ✅ 允许：文档文件
write("./test.txt", content)    ✅ 允许：相对路径
```

**临时文件** (需注意风险):
```bash
write("/tmp/output.txt", data)  ✅ 允许：临时目录
# 注意：文档会说明这是临时文件，重启后可能消失
```

---

## ⚠️ 灰色区域（需用户判断）

### 系统文件读取

**允许但需谨慎**:
```bash
read("/etc/passwd")        ✅ 允许：读取（非写入）
read("/etc/hosts")         ✅ 允许：读取配置
cat /var/log/syslog        ✅ 允许：读取日志

# ⚠️ 注意：读取系统文件是允许的，但要：
# 1. 确认你有权限读取
# 2. 不要泄露敏感信息
# 3. 只读取必要的部分
```

### 网络操作

**未明确限制**:
```bash
curl https://api.example.com   ✅ 允许：API调用
wget https://file.zip          ✅ 允许：下载文件
# 但禁止：curl | bash (下载并执行)

# ⚠️ 建议：
# 1. 验证URL安全性
# 2. 检查下载文件内容
# 3. 不要执行未验证的脚本
```

### 外部工具调用

**无限制但需谨慎**:
```bash
docker run image            ✅ 允许：运行容器
kubectl apply -f config     ✅ 允许：K8s操作
terraform apply             ✅ 允许：基础设施

# ⚠️ 建议：
# 1. 检查操作影响范围
# 2. 使用approve_mode确认
# 3. 在测试环境先验证
```

---

## 🔧 用户配置选项

### approve_mode (审批模式)

**建议启用场景**:
```bash
# 高风险操作环境
approve_mode: ask     # 每次操作都询问用户确认

# 自动化脚本环境
approve_mode: auto    # 自动执行（仅阻止危险操作）

# 学习/测试环境
approve_mode: suggest # 建议但不强制
```

### 安全级别建议

**开发环境**:
```yaml
approve_mode: auto      # 快速开发
# 安全限制仍然生效
```

**生产环境**:
```yaml
approve_mode: ask       # 严格审批
# 每个操作都确认
```

**学习环境**:
```yaml
approve_mode: suggest   # 温和提醒
# 学习危险操作边界
```

---

## 🛡️ 安全最佳实践

### 1. 理解阻止机制

```bash
# ❌ 被阻止的操作会收到明确错误消息
> rm -rf /
Error: destructive rm -rf on root paths blocked

# ✅ 查看错误消息理解原因
# ✅ 使用建议的安全替代方案
```

### 2. 使用相对路径

```bash
# ❌ 避免：绝对路径（可能误触系统文件）
write("/usr/lib/file.txt", ...)

# ✅ 推荐：相对路径（项目内操���）
write("./lib/file.txt", ...)
write("src/module.rs", ...)
```

### 3. 分解复杂任务

```bash
# ❌ 可能触发MAX_ITERATIONS限制
"重构整个系统架构"

# ✅ 推荐：分解为小任务
"重构auth模块"
"重构database模块"
"重构api模块"
```

### 4. 启用审批模式

```bash
# 生产环境建议启用
matrix config set approve_mode ask

# 查看当前配置
matrix config show
```

---

## 📊 安全边界总结

| 操作类型 | bash工具 | write工具 | Agent |
|---------|---------|----------|-------|
| **系统破坏** | ❌ 阻止30+命令 | ❌ 阻止系统文件 | ⚠️ 提示上限 |
| **路径穿越** | ❌ 条件阻止 | ❌ 完全阻止 | N/A |
| **内容大小** | N/A | ❌ 限制10MB | N/A |
| **迭代次数** | N/A | N/A | ⚠️ 限制50次 |

---

## 🆘 遇到阻止时怎么办

### 1. 查看错误消息

所有阻止操作都有明确错误消息：
```
Path traversal detected: '../../etc/passwd'
Paths cannot contain '..' for security

解决方案：
1. 使用项目相对路径
2. 确认目标文件安全性
3. 如确需访问，手动执行并确认风险
```

### 2. 确认真实需求

```bash
# 我真的需要删除整个目录吗？
rm -rf /project  # ❌ 阻止：可能误删重要文件

# ✅ 更安全的方式：
rm -rf ./build   # 删除构建目录（相对路径）
rm file1 file2   # 明确删除指定文件
```

### 3. 使用替代方案

```bash
# ❌ 被阻止：写入系统文件
write("/etc/hosts", "新的hosts配置")

# ✅ 替代方案：
# 1. 写入项目配置文件
write("./hosts.conf", "配置内容")
# 2. 手动执行（确认风险后）
# sudo cp ./hosts.conf /etc/hosts
```

### 4. 调整任务策略

```bash
# ❌ 达到MAX_ITERATIONS
# 任务复杂度过高

# ✅ 解决方案：
# 1. 检查已完成部分
# 2. 继续剩余任务（新对话）
# 3. 分解为多个小任务
```

---

## 🔍 安全审计日志

MatrixCode 会在以下情况记录审计信息：

```bash
# 阻止操作日志
[SECURITY] Blocked: rm -rf /
[SECURITY] Blocked: write /etc/passwd
[SECURITY] Blocked: Path traversal ../../etc

# 达到限制日志
[AGENT] Reached MAX_ITERATIONS (50)
[AGENT] Task may not be complete

# 大文件警告
[WRITE] Large file written: 5.2 MB
[WRITE] Consider splitting for performance
```

---

## 📞 安全问题反馈

发现安全问题或建议改进：

1. GitHub Issues: [提交安全建议]
2. 安全漏洞: 请私下联系开发者
3. 改进建议: 创建Issue讨论

---

## 🔄 版本更新

安全边界会随版本更新：

**v0.4.7** (当前):
- bash黑名单30+命令
- 路径验证完整
- 内容限制10MB
- MAX_ITERATIONS提示

**未来版本**:
- 用户可配置MAX_ITERATIONS
- 动态迭代次数调整
- 更多黑名单命令
- 安全审计面板

---

**安全是MatrixCode的核心价值**。我们坚持：
- ✅ 明确阻止危险操作
- ✅ 详细错误消息指导
- ✅ 平衡安全与可用性
- ✅ 用户可理解边界

---

**文档更新**: 2025年1月22日  
**版本**: v0.4.7  
**下次审查**: 2025年2月