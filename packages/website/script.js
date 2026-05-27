// 平滑滚动和动画效果
document.addEventListener('DOMContentLoaded', () => {
    // 导航栏滚动效果
    const navbar = document.querySelector('.navbar');
    let lastScroll = 0;

    window.addEventListener('scroll', () => {
        const currentScroll = window.pageYOffset;
        
        if (currentScroll > 100) {
            navbar.style.background = 'rgba(15, 15, 30, 0.98)';
        } else {
            navbar.style.background = 'rgba(15, 15, 30, 0.95)';
        }

        lastScroll = currentScroll;
    });

    // 平滑滚动到锚点
    document.querySelectorAll('a[href^="#"]').forEach(anchor => {
        anchor.addEventListener('click', function(e) {
            e.preventDefault();
            const target = document.querySelector(this.getAttribute('href'));
            if (target) {
                const headerOffset = 80;
                const elementPosition = target.getBoundingClientRect().top;
                const offsetPosition = elementPosition + window.pageYOffset - headerOffset;

                window.scrollTo({
                    top: offsetPosition,
                    behavior: 'smooth'
                });
            }
        });
    });

    // 滚动动画
    const observerOptions = {
        threshold: 0.1,
        rootMargin: '0px 0px -100px 0px'
    };

    const observer = new IntersectionObserver((entries) => {
        entries.forEach(entry => {
            if (entry.isIntersecting) {
                entry.target.classList.add('visible');
            }
        });
    }, observerOptions);

    // 为需要动画的元素添加 fade-in 类
    document.querySelectorAll('.feature-card, .step, .module, .example-card, .doc-card').forEach(el => {
        el.classList.add('fade-in');
        observer.observe(el);
    });

    // 统计数字动画
    const animateNumbers = () => {
        const stats = document.querySelectorAll('.stat-number');
        
        stats.forEach(stat => {
            const target = stat.textContent;
            const isPercentage = target.includes('%');
            const isPlus = target.includes('+');
            const numericValue = parseInt(target.replace(/[^0-9]/g, ''));
            
            let current = 0;
            const increment = numericValue / 50;
            const duration = 2000;
            const stepTime = duration / 50;

            const updateNumber = () => {
                current += increment;
                if (current < numericValue) {
                    stat.textContent = Math.floor(current) + (isPlus ? '+' : '') + (isPercentage ? '%' : '');
                    setTimeout(updateNumber, stepTime);
                } else {
                    stat.textContent = target;
                }
            };

            // 当元素进入视口时开始动画
            const numberObserver = new IntersectionObserver((entries) => {
                entries.forEach(entry => {
                    if (entry.isIntersecting) {
                        updateNumber();
                        numberObserver.unobserve(entry.target);
                    }
                });
            }, { threshold: 0.5 });

            numberObserver.observe(stat);
        });
    };

    animateNumbers();

    // 代码块复制功能
    document.querySelectorAll('pre code').forEach(block => {
        block.addEventListener('click', async () => {
            try {
                await navigator.clipboard.writeText(block.textContent);
                
                // 显示复制成功提示
                const tooltip = document.createElement('div');
                tooltip.className = 'copy-tooltip';
                tooltip.textContent = '已复制!';
                tooltip.style.cssText = `
                    position: fixed;
                    top: 50%;
                    left: 50%;
                    transform: translate(-50%, -50%);
                    background: rgba(74, 144, 226, 0.95);
                    color: white;
                    padding: 10px 20px;
                    border-radius: 5px;
                    font-size: 14px;
                    z-index: 9999;
                    animation: fadeInOut 2s forwards;
                `;
                document.body.appendChild(tooltip);
                
                setTimeout(() => {
                    tooltip.remove();
                }, 2000);
            } catch (err) {
                console.error('复制失败:', err);
            }
        });
    });

    // 添加工具提示动画
    const style = document.createElement('style');
    style.textContent = `
        @keyframes fadeInOut {
            0% { opacity: 0; transform: translate(-50%, -50%) scale(0.8); }
            20% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
            80% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
            100% { opacity: 0; transform: translate(-50%, -50%) scale(0.8); }
        }
    `;
    document.head.appendChild(style);

    // 表格行悬停效果增强
    document.querySelectorAll('.tools-table tbody tr').forEach(row => {
        row.addEventListener('mouseenter', function() {
            this.style.transition = 'all 0.3s ease';
        });
    });

    // 移动端菜单切换
    const createMobileMenu = () => {
        if (window.innerWidth <= 768) {
            const navLinks = document.querySelector('.nav-links');
            if (!document.querySelector('.mobile-menu-toggle')) {
                const menuToggle = document.createElement('button');
                menuToggle.className = 'mobile-menu-toggle';
                menuToggle.innerHTML = '☰';
                menuToggle.style.cssText = `
                    background: none;
                    border: none;
                    color: var(--text-primary);
                    font-size: 1.5rem;
                    cursor: pointer;
                    display: block;
                `;
                
                const navbar = document.querySelector('.navbar .container');
                navbar.insertBefore(menuToggle, navLinks);
                
                menuToggle.addEventListener('click', () => {
                    navLinks.style.display = navLinks.style.display === 'flex' ? 'none' : 'flex';
                    navLinks.style.flexDirection = 'column';
                    navLinks.style.position = 'absolute';
                    navLinks.style.top = '60px';
                    navLinks.style.left = '0';
                    navLinks.style.right = '0';
                    navLinks.style.background = 'rgba(15, 15, 30, 0.98)';
                    navLinks.style.padding = '20px';
                    navLinks.style.borderBottom = '1px solid var(--border-color)';
                });
            }
        }
    };

    createMobileMenu();
    window.addEventListener('resize', createMobileMenu);

    // 页面加载完成后的入场动画
    setTimeout(() => {
        document.body.classList.add('loaded');
    }, 100);
});

// 添加页面加载动画
window.addEventListener('load', () => {
    document.body.style.opacity = '1';
    document.body.style.transition = 'opacity 0.5s ease';
});