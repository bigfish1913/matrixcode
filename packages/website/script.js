/**
 * MatrixCode 官网交互脚本
 */

// 等待 DOM 加载完成
document.addEventListener('DOMContentLoaded', () => {
    // 初始化所有功能
    initMatrixEffect();
    initNavigation();
    initToolsTabs();
    initCopyButtons();
    initBackToTop();
    initAnimations();
    initMobileMenu();
});

/**
 * Matrix 数字雨效果
 */
function initMatrixEffect() {
    const canvas = document.getElementById('matrix-canvas');
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    
    // 设置画布大小
    function resizeCanvas() {
        canvas.width = canvas.offsetWidth;
        canvas.height = canvas.offsetHeight;
    }
    resizeCanvas();
    window.addEventListener('resize', resizeCanvas);

    // Matrix 字符
    const chars = 'MatrixCode 0123456789 ABCDEFGHIJKLMNOPQRSTUVWXYZアイウエオカキクケコサシスセソタチツテトナニヌネノ';
    const charArray = chars.split('');
    
    const fontSize = 14;
    const columns = Math.floor(canvas.width / fontSize);
    const drops = Array(columns).fill(1);

    // 颜色设置
    const primaryColor = '#4A90E2';
    const secondaryColor = '#357ABD';

    function drawMatrix() {
        // 半透明黑色背景，产生渐隐效果
        ctx.fillStyle = 'rgba(10, 11, 17, 0.05)';
        ctx.fillRect(0, 0, canvas.width, canvas.height);

        // 设置字体
        ctx.font = `${fontSize}px monospace`;

        // 绘制字符
        for (let i = 0; i < drops.length; i++) {
            // 随机选择字符
            const char = charArray[Math.floor(Math.random() * charArray.length)];
            
            // 渐变颜色效果
            const brightness = Math.random();
            if (brightness > 0.9) {
                ctx.fillStyle = '#FFFFFF'; // 高亮字符
            } else if (brightness > 0.7) {
                ctx.fillStyle = primaryColor;
            } else {
                ctx.fillStyle = secondaryColor;
            }

            // 绘制字符
            const x = i * fontSize;
            const y = drops[i] * fontSize;
            ctx.fillText(char, x, y);

            // 重置下落位置
            if (y > canvas.height && Math.random() > 0.975) {
                drops[i] = 0;
            }
            
            drops[i]++;
        }
    }

    // 动画循环
    setInterval(drawMatrix, 50);
}

/**
 * 导航栏功能
 */
function initNavigation() {
    const navbar = document.querySelector('.navbar');
    if (!navbar) return;

    // 滚动时导航栏样式变化
    let lastScroll = 0;
    window.addEventListener('scroll', () => {
        const currentScroll = window.pageYOffset;
        
        // 添加阴影效果
        if (currentScroll > 50) {
            navbar.style.boxShadow = '0 4px 20px rgba(0, 0, 0, 0.3)';
        } else {
            navbar.style.boxShadow = 'none';
        }

        lastScroll = currentScroll;
    });

    // 平滑滚动到锚点
    document.querySelectorAll('a[href^="#"]').forEach(anchor => {
        anchor.addEventListener('click', (e) => {
            e.preventDefault();
            const targetId = anchor.getAttribute('href');
            const target = document.querySelector(targetId);
            
            if (target) {
                const offset = navbar.offsetHeight + 20;
                const targetPosition = target.offsetTop - offset;
                
                window.scrollTo({
                    top: targetPosition,
                    behavior: 'smooth'
                });
            }
        });
    });
}

/**
 * 工具标签页切换
 */
function initToolsTabs() {
    const tabs = document.querySelectorAll('.tab-btn');
    const panels = document.querySelectorAll('.tools-panel');

    if (tabs.length === 0 || panels.length === 0) return;

    tabs.forEach(tab => {
        tab.addEventListener('click', () => {
            const targetTab = tab.getAttribute('data-tab');

            // 更新标签状态
            tabs.forEach(t => t.classList.remove('active'));
            tab.classList.add('active');

            // 更新面板显示
            panels.forEach(panel => {
                if (panel.getAttribute('data-tab') === targetTab) {
                    panel.classList.add('active');
                    // 添加动画效果
                    panel.style.animation = 'fadeIn 0.3s ease forwards';
                } else {
                    panel.classList.remove('active');
                }
            });
        });
    });
}

/**
 * 代码复制按钮
 */
function initCopyButtons() {
    const copyButtons = document.querySelectorAll('.copy-btn');

    copyButtons.forEach(btn => {
        btn.addEventListener('click', async () => {
            const code = btn.getAttribute('data-code');
            
            try {
                await navigator.clipboard.writeText(code);
                
                // 显示复制成功提示
                const originalHTML = btn.innerHTML;
                btn.innerHTML = `
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M20 6L9 17l-5-5"/>
                    </svg>
                `;
                btn.style.color = '#4CAF50';
                
                // 2秒后恢复原状
                setTimeout(() => {
                    btn.innerHTML = originalHTML;
                    btn.style.color = '';
                }, 2000);

            } catch (err) {
                console.error('复制失败:', err);
            }
        });
    });
}

/**
 * 返回顶部按钮
 */
function initBackToTop() {
    const backToTopBtn = document.querySelector('.back-to-top');
    if (!backToTopBtn) return;

    // 滚动监听
    window.addEventListener('scroll', () => {
        if (window.pageYOffset > 300) {
            backToTopBtn.classList.add('visible');
        } else {
            backToTopBtn.classList.remove('visible');
        }
    });

    // 点击返回顶部
    backToTopBtn.addEventListener('click', () => {
        window.scrollTo({
            top: 0,
            behavior: 'smooth'
        });
    });
}

/**
 * 页面动画
 */
function initAnimations() {
    // 观察器配置
    const observerOptions = {
        root: null,
        rootMargin: '0px',
        threshold: 0.1
    };

    // 创建观察器
    const observer = new IntersectionObserver((entries) => {
        entries.forEach(entry => {
            if (entry.isIntersecting) {
                entry.target.classList.add('animate-fadeIn');
                // 停止观察已动画的元素
                observer.unobserve(entry.target);
            }
        });
    }, observerOptions);

    // 观察所有需要动画的元素
    const animateElements = document.querySelectorAll(
        '.feature-card, .tool-card, .case-card, .step-card, .example-card'
    );
    
    animateElements.forEach(el => {
        el.style.opacity = '0';
        observer.observe(el);
    });

    // 终端演示动画
    animateTerminalDemo();
}

/**
 * 终端演示动画
 */
function animateTerminalDemo() {
    const terminalLines = document.querySelectorAll('.terminal-body .terminal-line');
    
    if (terminalLines.length === 0) return;

    // 初始隐藏所有行
    terminalLines.forEach(line => {
        line.style.opacity = '0';
        line.style.transform = 'translateY(10px)';
    });

    // 逐行显示动画
    let index = 0;
    const showLine = () => {
        if (index < terminalLines.length) {
            const line = terminalLines[index];
            line.style.transition = 'opacity 0.3s ease, transform 0.3s ease';
            line.style.opacity = '1';
            line.style.transform = 'translateY(0)';
            index++;
            
            // 每行显示间隔
            setTimeout(showLine, 500);
        }
    };

    // 延迟开始动画
    setTimeout(showLine, 1000);
}

/**
 * 移动端菜单
 */
function initMobileMenu() {
    const menuToggle = document.querySelector('.menu-toggle');
    const navLinks = document.querySelector('.nav-links');

    if (!menuToggle || !navLinks) return;

    menuToggle.addEventListener('click', () => {
        navLinks.classList.toggle('show');
        menuToggle.classList.toggle('active');
    });

    // 点击链接后关闭菜单
    navLinks.querySelectorAll('a').forEach(link => {
        link.addEventListener('click', () => {
            navLinks.classList.remove('show');
            menuToggle.classList.remove('active');
        });
    });

    // 点击外部关闭菜单
    document.addEventListener('click', (e) => {
        if (!menuToggle.contains(e.target) && !navLinks.contains(e.target)) {
            navLinks.classList.remove('show');
            menuToggle.classList.remove('active');
        }
    });
}

/**
 * 工具搜索过滤（文档页面）
 */
function initToolSearch() {
    const searchInput = document.getElementById('tool-search');
    const toolCards = document.querySelectorAll('.tool-doc-card');

    if (!searchInput || toolCards.length === 0) return;

    searchInput.addEventListener('input', (e) => {
        const searchTerm = e.target.value.toLowerCase();

        toolCards.forEach(card => {
            const toolName = card.querySelector('.tool-name')?.textContent.toLowerCase();
            const toolDesc = card.querySelector('.tool-desc')?.textContent.toLowerCase();

            if (toolName?.includes(searchTerm) || toolDesc?.includes(searchTerm)) {
                card.style.display = 'block';
            } else {
                card.style.display = 'none';
            }
        });
    });
}

/**
 * 主题切换（可选功能）
 */
function initThemeToggle() {
    const themeToggle = document.querySelector('.theme-toggle');
    if (!themeToggle) return;

    // 检查本地存储的主题
    const savedTheme = localStorage.getItem('theme') || 'dark';
    document.documentElement.setAttribute('data-theme', savedTheme);

    themeToggle.addEventListener('click', () => {
        const currentTheme = document.documentElement.getAttribute('data-theme');
        const newTheme = currentTheme === 'dark' ? 'light' : 'dark';
        
        document.documentElement.setAttribute('data-theme', newTheme);
        localStorage.setItem('theme', newTheme);
    });
}

/**
 * 懒加载图片
 */
function initLazyLoad() {
    const lazyImages = document.querySelectorAll('img[data-src]');
    
    if (lazyImages.length === 0) return;

    const imageObserver = new IntersectionObserver((entries) => {
        entries.forEach(entry => {
            if (entry.isIntersecting) {
                const img = entry.target;
                img.src = img.dataset.src;
                img.removeAttribute('data-src');
                imageObserver.unobserve(img);
            }
        });
    });

    lazyImages.forEach(img => imageObserver.observe(img));
}

/**
 * 平滑滚动动画（用于内部链接）
 */
function smoothScrollTo(target, duration = 500) {
    const start = window.pageYOffset;
    const distance = target - start;
    let startTime = null;

    function animation(currentTime) {
        if (startTime === null) startTime = currentTime;
        const timeElapsed = currentTime - startTime;
        const progress = Math.min(timeElapsed / duration, 1);
        
        // 缓动函数
        const ease = easeInOutCubic(progress);
        
        window.scrollTo(0, start + distance * ease);
        
        if (timeElapsed < duration) {
            requestAnimationFrame(animation);
        }
    }

    requestAnimationFrame(animation);
}

function easeInOutCubic(t) {
    return t < 0.5 
        ? 4 * t * t * t 
        : 1 - Math.pow(-2 * t + 2, 3) / 2;
}

/**
 * 统计数字动画
 */
function animateNumbers() {
    const statNumbers = document.querySelectorAll('.stat-number');
    
    statNumbers.forEach(stat => {
        const target = stat.textContent;
        const isInfinity = target === '∞';
        
        if (isInfinity) return;

        const targetNum = parseInt(target);
        let current = 0;
        const increment = targetNum / 50;
        const duration = 1500;
        const stepTime = duration / 50;

        const counter = setInterval(() => {
            current += increment;
            if (current >= targetNum) {
                stat.textContent = target;
                clearInterval(counter);
            } else {
                stat.textContent = Math.floor(current) + '+';
            }
        }, stepTime);
    });
}

// 页面加载完成后启动数字动画
window.addEventListener('load', animateNumbers);

/**
 * 代码块语法高亮（简易版）
 */
function highlightCode() {
    const codeBlocks = document.querySelectorAll('code');
    
    codeBlocks.forEach(block => {
        let code = block.innerHTML;
        
        // 关键字高亮
        const keywords = ['fn', 'let', 'const', 'if', 'else', 'for', 'while', 'return', 'pub', 'struct', 'impl', 'use', 'mod', 'async', 'await'];
        keywords.forEach(kw => {
            code = code.replace(new RegExp(`\\b${kw}\\b`, 'g'), `<span class="keyword">${kw}</span>`);
        });

        // 字符串高亮
        code = code.replace(/"([^"]*)"/g, '<span class="string">"$1"</span>');
        
        // 注释高亮
        code = code.replace(/\/\/(.*)/g, '<span class="comment">//$1</span>');

        block.innerHTML = code;
    });
}

// 初始化代码高亮
document.addEventListener('DOMContentLoaded', highlightCode);

/**
 * 工具卡片悬停效果
 */
document.addEventListener('DOMContentLoaded', () => {
    const toolCards = document.querySelectorAll('.tool-card');
    
    toolCards.forEach(card => {
        card.addEventListener('mouseenter', () => {
            // 添加发光效果
            card.style.boxShadow = '0 8px 30px rgba(74, 144, 226, 0.2)';
        });
        
        card.addEventListener('mouseleave', () => {
            card.style.boxShadow = '';
        });
    });
});

/**
 * 响应式导航栏
 */
window.addEventListener('resize', () => {
    const navLinks = document.querySelector('.nav-links');
    const menuToggle = document.querySelector('.menu-toggle');
    
    if (window.innerWidth > 1024) {
        navLinks?.classList.remove('show');
        menuToggle?.classList.remove('active');
    }
});

/**
 * 键盘导航支持
 */
document.addEventListener('keydown', (e) => {
    // ESC 关闭移动端菜单
    if (e.key === 'Escape') {
        const navLinks = document.querySelector('.nav-links');
        const menuToggle = document.querySelector('.menu-toggle');
        
        navLinks?.classList.remove('show');
        menuToggle?.classList.remove('active');
    }
});