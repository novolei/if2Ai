# if2AI 动态视觉系统规范

## 1. 主题定位

**主题名**：Breathing Glass  
**变体名**：Pearl Mist  
**气质关键词**：温柔、未来感、陪伴感、流动感、轻智能、安静但有生命力

这套主题适用于 AI Agent 桌面 App，目标不是炫技，而是营造一种“界面在轻微呼吸、AI 正在安静陪伴”的感受。

---

## 2. 核心配色 Tokens

### 基础背景色
- `bg.app = #F4F5F7`
- `bg.secondary = #EDEFF3`
- `surface.base = #E7EAEE`
- `surface.soft = #F7F8FA`
- `border.soft = #D8DDE5`
- `border.strong = #C7CED9`

### 主色
- `primary.400 = #8FAFD6`
- `primary.500 = #6E8FBF`
- `primary.600 = #4F6FA3`

### AI 存在感色
- `accent.iris = #DCCFF4`
- `accent.mint = #D9ECE5`
- `accent.cyan = #BCEEE7`
- `accent.lavender = #CFC7FF`
- `accent.peach = #F6E4D6`

### 文本色
- `text.primary = #1E2630`
- `text.secondary = #556070`
- `text.tertiary = #7C8797`
- `text.inverse = #F8FAFC`

### 状态色
- `state.success = #8FC7B5`
- `state.warning = #E8C89A`
- `state.error = #D89BA5`
- `state.info = #8FAFD6`

---

## 3. 渐变规范

### 主背景渐变
```css
background:
  radial-gradient(circle at 30% 30%, rgba(220,207,244,0.45), transparent 42%),
  radial-gradient(circle at 75% 35%, rgba(143,175,214,0.35), transparent 40%),
  radial-gradient(circle at 55% 78%, rgba(217,236,229,0.38), transparent 36%),
  linear-gradient(180deg, #F4F5F7 0%, #EDEFF3 100%);
```

### Agent Orb 渐变
```css
background:
  radial-gradient(circle at 35% 30%, rgba(255,255,255,0.9), rgba(255,255,255,0.22) 35%, transparent 60%),
  linear-gradient(135deg, rgba(220,207,244,0.55), rgba(143,175,214,0.42), rgba(217,236,229,0.5));
```

### 按钮高亮渐变
```css
background: linear-gradient(135deg, #8FAFD6 0%, #DCCFF4 100%);
```

---

## 4. 材质系统

### 玻璃卡片
```css
background: rgba(255,255,255,0.58);
backdrop-filter: blur(20px);
border: 1px solid rgba(255,255,255,0.42);
box-shadow:
  0 8px 30px rgba(80, 105, 145, 0.08),
  inset 0 1px 0 rgba(255,255,255,0.45);
```

### 深层浮层
```css
background: rgba(247,248,250,0.72);
backdrop-filter: blur(28px);
border: 1px solid rgba(215,221,229,0.85);
```

### 轻量分隔
```css
border-color: rgba(199,206,217,0.65);
```

---

## 5. 动效规范

### 呼吸动效
- 时长：`8s`
- 曲线：`ease-in-out`
- 节奏：缓慢、连续、不突兀
- 缩放范围：`0.96 ~ 1.06`
- 透明度范围：`0.9 ~ 1.0`
- 模糊范围：`18px ~ 28px`

### 漂移动效
- 时长：`10s ~ 14s`
- 位移范围：`8px ~ 24px`
- 方向：轻微横向+纵向组合

### Hover 动效
- 时长：`180ms ~ 240ms`
- 内容：透明度、阴影、轻微位移

### 禁忌
- 不要快速 pulse
- 不要大面积颜色闪烁
- 不要在正文阅读区做持续强动画

---

## 6. UI 组件规范

### 6.1 App 主背景
- 使用超浅雾面背景
- 叠加 2~3 层柔和彩色模糊光斑
- 最上层再叠一层轻薄 material

### 6.2 Sidebar
- 背景比主内容区域略深一点点
- 顶部可以放小型 Orb 或品牌呼吸光
- 当前选中项使用淡蓝或淡紫高亮

### 6.3 Chat 区域
- 保持高可读性
- 消息气泡建议使用：
  - 用户：较清晰的淡蓝底
  - Agent：玻璃白 + 轻灰边框
- 工具调用 / 事件流使用更细的字重和更低对比度

### 6.4 Task / Run 卡片
- 使用玻璃卡片样式
- 状态变化通过边框和顶部微光体现
- 不要依赖高饱和色块区分状态

### 6.5 输入框
- 背景应稳定，不建议持续动态流动
- 可用轻微内发光 + hover 提示聚焦
- 聚焦边框：`#8FAFD6`

### 6.6 按钮

#### Primary Button
- 背景：主蓝到淡紫渐变
- 文本：深色或白色，视背景而定
- hover：亮度略升、阴影更柔

#### Secondary Button
- 玻璃白底 + 轻边框
- hover 时增加轻微背景雾度

#### Ghost Button
- 减少填充，靠文字和 hover 表现

---

## 7. Agent Presence Orb 规范

Orb 是 if2AI 的核心视觉符号之一。

### 视觉构成
- 外层：半透明玻璃球体
- 中层：淡蓝 / 淡紫 / 薄荷绿流动层
- 内层：中心柔白高光
- 底层：极弱彩虹折射感

### 状态映射
- 空闲：慢呼吸
- 思考中：轻微流动 + 柔和亮度提升
- 执行任务：增加一点点方向性漂移
- 完成：短暂更明亮后回归稳定
- 错误：不要闪红，用轻暖粉灰提醒即可

### 尺寸建议
- 小：20~28px
- 中：40~64px
- 大：96~160px
- Hero Orb：220~320px

---

## 8. Tailwind Design Tokens 示例

```js
export default {
  theme: {
    extend: {
      colors: {
        fog: '#F4F5F7',
        fogSecondary: '#EDEFF3',
        glass: '#E7EAEE',
        surfaceSoft: '#F7F8FA',
        borderSoft: '#D8DDE5',
        borderStrong: '#C7CED9',

        mistBlue: '#8FAFD6',
        mistBlueDeep: '#6E8FBF',
        mistBlueStrong: '#4F6FA3',

        softIris: '#DCCFF4',
        pearlMint: '#D9ECE5',
        haloCyan: '#BCEEE7',
        haloLavender: '#CFC7FF',
        haloPeach: '#F6E4D6',

        textPrimary: '#1E2630',
        textSecondary: '#556070',
        textTertiary: '#7C8797',
      },
      boxShadow: {
        softGlow: '0 0 40px rgba(143,175,214,0.18)',
        irisGlow: '0 0 50px rgba(220,207,244,0.20)',
        glassCard: '0 8px 30px rgba(80,105,145,0.08)',
      },
      borderRadius: {
        glass: '24px',
      },
      keyframes: {
        breathe: {
          '0%, 100%': {
            transform: 'scale(1)',
            opacity: '0.92',
            filter: 'blur(18px)',
          },
          '50%': {
            transform: 'scale(1.04)',
            opacity: '1',
            filter: 'blur(24px)',
          },
        },
        drift: {
          '0%, 100%': { transform: 'translate(0px, 0px)' },
          '50%': { transform: 'translate(14px, -10px)' },
        },
      },
      animation: {
        breathe: 'breathe 8s ease-in-out infinite',
        drift: 'drift 12s ease-in-out infinite',
      },
    },
  },
}
```

---

## 9. SwiftUI 主题落地建议

### 颜色定义
```swift
import SwiftUI

extension Color {
    static let appBackground = Color(hex: "#F4F5F7")
    static let appBackgroundSecondary = Color(hex: "#EDEFF3")
    static let glassSurface = Color(hex: "#E7EAEE")

    static let mistBlue = Color(hex: "#8FAFD6")
    static let mistBlueDeep = Color(hex: "#6E8FBF")
    static let softIris = Color(hex: "#DCCFF4")
    static let pearlMint = Color(hex: "#D9ECE5")
    static let haloLavender = Color(hex: "#CFC7FF")
}
```

### 背景层结构
1. 底层雾白色
2. 叠加 2~3 个大面积 blur color blobs
3. 叠加 ultraThinMaterial
4. 上层再放业务 UI

### 动画建议
- 使用 `withAnimation(.easeInOut(duration: 7).repeatForever(autoreverses: true))`
- 对 blob 做 `offset`、`scaleEffect`、`opacity` 微调
- 不要给文本区域加大幅 blur 动画

---

## 10. 推荐页面映射

### 最适合使用动态背景的页面
- 启动页
- 首页欢迎页
- 空状态页
- Agent 正在思考 / 等待输入状态
- 设置页顶部视觉区

### 最适合使用 Orb 的位置
- Sidebar 顶部品牌区
- 首页 Hero 区
- Agent 当前状态指示器
- 任务执行中的微型状态球

### 动效应克制的区域
- 长文本聊天正文
- 高密度任务列表
- 表格/数据面板
- 输入框连续区域

---

## 11. 推荐层级系统

### Z 层级
- `z0`：基础背景
- `z1`：呼吸彩色模糊层
- `z2`：glass material 层
- `z3`：主内容层
- `z4`：浮层 / modal / command palette
- `z5`：全局状态提示

### 圆角建议
- 大容器：24px
- 卡片：20px
- 输入框：16px
- 小按钮：12px
- Orb：完全圆形

---

## 12. 设计原则

1. **让 AI 活着，但不要让界面吵。**
2. **动画服务于情绪，不服务于炫技。**
3. **高可读性优先于强视觉表现。**
4. **颜色以低饱和、低攻击性为主。**
5. **把动态效果集中在“品牌、状态、存在感”区域。**

---

## 13. 下一步建议

最适合继续补齐的是下面三项：

1. `if2AI Tailwind UI Demo`：做一版网页原型
2. `if2AI SwiftUI Theme Kit`：输出可直接复用的颜色/背景/按钮/卡片组件
3. `Agent Presence Orb Spec`：单独定义 Orb 的几种状态和动效切换规范

