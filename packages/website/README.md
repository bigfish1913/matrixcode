# MatrixCode 官网

这是 MatrixCode 项目的官方网站源码。

## 📁 文件结构

```
website/
├── index.html      # 首页
├── docs.html       # 文档页面
├── examples.html   # 示例页面
├── styles.css      # 样式文件
├── script.js       # 脚本文件
└── README.md       # 本文件
```

## 🚀 快速开始

### 本地预览

直接在浏览器中打开 `index.html` 文件即可预览网站。

或使用简单的 HTTP 服务器：

```bash
# 使用 Python
python -m http.server 8000

# 使用 Node.js (需要安装 http-server)
npx http-server

# 使用 PHP
php -S localhost:8000
```

然后访问 `http://localhost:8000`

### 部署到静态网站托管

网站是纯静态的，可以部署到任何静态网站托管服务：

#### GitHub Pages

1. 将 `website/` 目录内容推送到 GitHub 仓库
2. 在仓库设置中启用 GitHub Pages
3. 选择分支和目录

#### Netlify

1. 连接 GitHub 仓库
2. 设置构建目录为 `website/`
3. 自动部署

#### Vercel

1. 导入 GitHub 仓库
2. 设置根目录为 `website/`
3. 自动部署

#### 其他平台

- Cloudflare Pages
- GitLab Pages
- 传统 Web 服务器 (Apache/Nginx)

## 🎨 自定义配置

### 修改 Logo

在 HTML 文件中找到 logo 图片标签，替换 `src` 属性：

```html
<img src="your-logo-url" alt="MatrixCode Logo">
```

### 修改配色

编辑 `styles.css` 中的 CSS 变量：

```css
:root {
    --primary-color: #4A90E2;      /* 主色调 */
    --secondary-color: #7B68EE;    /* 次要色调 */
    --accent-color: #FF6B6B;       /* 强调色 */
    --background-dark: #0F0F1E;    /* 深色背景 */
    --background-light: #1A1A2E;   /* 浅色背景 */
    --text-primary: #FFFFFF;        /* 主文本色 */
    --text-secondary: #B0B0C0;      /* 次要文本色 */
}
```

### 修改内容

直接编辑 HTML 文件中的文本内容即可。

### 添加新页面

1. 复制任意 HTML 文件作为模板
2. 修改内容和链接
3. 更新导航栏链接

## 📱 响应式设计

网站已内置响应式设计，支持：
- 桌面端（> 1024px）
- 平板端（768px - 1024px）
- 移动端（< 768px）

## ⚡ 性能优化

网站已包含以下优化：
- CSS 动画使用 `transform` 和 `opacity`
- 图片使用 SVG 格式（Logo）
- 最小化 DOM 操作
- 延迟加载非关键资源

## 🔧 技术栈

- **HTML5**: 语义化标记
- **CSS3**: Flexbox、Grid、动画
- **JavaScript**: 原生 JS，无依赖
- **设计**: 渐变、毛玻璃效果、现代 UI

## 📝 更新日志

### v1.0.0 (2024-01-15)
- 初始版本发布
- 包含首页、文档、示例三个页面
- 响应式设计
- 平滑滚动和动画效果

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

MIT License - 详见 LICENSE 文件