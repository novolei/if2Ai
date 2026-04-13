// ==================== 泡泡类定义 ====================

class Bubble {
    constructor(x, y, color, radius = 20) {
        this.x = x;
        this.y = y;
        this.color = color;
        this.radius = radius;
        this.dx = 0;
        this.dy = 0;
        this.active = false;
        this.row = -1;
        this.col = -1;
    }

    // 绘制泡泡
    draw(ctx) {
        ctx.save();
        
        // 创建渐变效果
        const gradient = ctx.createRadialGradient(
            this.x - this.radius * 0.3,
            this.y - this.radius * 0.3,
            0,
            this.x,
            this.y,
            this.radius
        );
        
        gradient.addColorStop(0, this.lightenColor(this.color, 60));
        gradient.addColorStop(0.5, this.color);
        gradient.addColorStop(1, this.darkenColor(this.color, 30));
        
        // 绘制主泡泡
        ctx.beginPath();
        ctx.arc(this.x, this.y, this.radius, 0, Math.PI * 2);
        ctx.fillStyle = gradient;
        ctx.fill();
        
        // 添加高光
        ctx.beginPath();
        ctx.arc(
            this.x - this.radius * 0.3,
            this.y - this.radius * 0.3,
            this.radius * 0.2,
            0,
            Math.PI * 2
        );
        ctx.fillStyle = 'rgba(255, 255, 255, 0.6)';
        ctx.fill();
        
        // 添加小高光点
        ctx.beginPath();
        ctx.arc(
            this.x - this.radius * 0.15,
            this.y - this.radius * 0.5,
            this.radius * 0.08,
            0,
            Math.PI * 2
        );
        ctx.fillStyle = 'rgba(255, 255, 255, 0.8)';
        ctx.fill();
        
        ctx.restore();
    }

    // 颜色变亮
    lightenColor(color, percent) {
        const num = parseInt(color.replace('#', ''), 16);
        const amt = Math.round(2.55 * percent);
        const R = Math.min(255, (num >> 16) + amt);
        const G = Math.min(255, ((num >> 8) & 0x00FF) + amt);
        const B = Math.min(255, (num & 0x0000FF) + amt);
        return '#' + (0x1000000 + R * 0x10000 + G * 0x100 + B).toString(16).slice(1);
    }

    // 颜色变暗
    darkenColor(color, percent) {
        const num = parseInt(color.replace('#', ''), 16);
        const amt = Math.round(2.55 * percent);
        const R = Math.max(0, (num >> 16) - amt);
        const G = Math.max(0, ((num >> 8) & 0x00FF) - amt);
        const B = Math.max(0, (num & 0x0000FF) - amt);
        return '#' + (0x1000000 + R * 0x10000 + G * 0x100 + B).toString(16).slice(1);
    }

    // 移动泡泡
    move() {
        this.x += this.dx;
        this.y += this.dy;
    }

    // 检测碰撞
    collidesWith(other) {
        const dx = this.x - other.x;
        const dy = this.y - other.y;
        const distance = Math.sqrt(dx * dx + dy * dy);
        return distance < this.radius + other.radius;
    }
}

// ==================== 泡泡颜色定义 ====================
const BUBBLE_COLORS = [
    '#FF6B6B',  // 红色
    '#4ECDC4',  // 青色
    '#FFE66D',  // 黄色
    '#95E1D3',  // 浅绿
    '#F38181',  // 粉色
    '#AA96DA',  // 紫色
    '#6C5CE7',  // 深紫
    '#74B9FF',  // 蓝色
];

// ==================== 工具函数 ====================

// 获取随机颜色
function getRandomColor() {
    return BUBBLE_COLORS[Math.floor(Math.random() * BUBBLE_COLORS.length)];
}

// 获取随机颜色（排除某些颜色）
function getRandomColorExcluding(excludedColors) {
    const availableColors = BUBBLE_COLORS.filter(color => !excludedColors.includes(color));
    if (availableColors.length === 0) return getRandomColor();
    return availableColors[Math.floor(Math.random() * availableColors.length)];
}

// 计算两点之间的距离
function distance(x1, y1, x2, y2) {
    return Math.sqrt((x2 - x1) ** 2 + (y2 - y1) ** 2);
}

// 六边形网格辅助函数
function getGridPosition(row, col, bubbleRadius, canvasWidth) {
    const diameter = bubbleRadius * 2;
    const rowHeight = diameter * 0.866; // sqrt(3)/2 for hex grid
    
    // 奇偶行错开
    let x = col * diameter + bubbleRadius;
    if (row % 2 === 1) {
        x += bubbleRadius;
    }
    
    let y = row * rowHeight + bubbleRadius;
    
    return { x, y };
}

function getGridPositionReverse(x, y, bubbleRadius, canvasWidth) {
    const diameter = bubbleRadius * 2;
    const rowHeight = diameter * 0.866;
    
    const row = Math.round((y - bubbleRadius) / rowHeight);
    
    let col;
    if (row % 2 === 0) {
        col = Math.round((x - bubbleRadius) / diameter);
    } else {
        col = Math.round((x - bubbleRadius * 2) / diameter);
    }
    
    return { row, col };
}

// 导出类和函数
if (typeof module !== 'undefined' && module.exports) {
    module.exports = { Bubble, BUBBLE_COLORS, getRandomColor, getRandomColorExcluding, distance, getGridPosition, getGridPositionReverse };
}
