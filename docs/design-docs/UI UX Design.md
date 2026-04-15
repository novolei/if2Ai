桌面 AI Agent 应用设计语言规范

设计哲学：如同一位安静、专注、有温度的助手，始终在场，从不打扰。

一、设计原则

1. 呼吸感优先（Breathable First）
   界面需要"留白"，元素之间保持充足的空气感。不堆砌信息，让用户的眼睛有地方"休息"。每一块内容区都应该像一张白纸，而不是一块告示栏。
2. 温度而非冷峻（Warmth, Not Precision）
   拒绝纯粹机械的精确感。圆角、暖灰、低对比的组合传递出"这是一个在照顾你"的感受，而不是"这是一台机器在处理你"。
3. 层级感知，而非视觉冲击（Perceived Hierarchy, Not Visual Shock）
   层级通过微妙的色差、字重差异和间距传递，而不是通过高对比边框或强调色。用户能"感受"到层级，但说不出来具体在哪里。
4. 长时陪伴设计（Designed for Duration）
   界面需经得起长时间使用。颜色不刺眼，字体不疲劳，交互不惊扰。整体像一个书房的桌面，而不是一个广告牌。
5. 克制的存在感（Restrained Presence）
   AI Agent 本身的"存在感"通过细节体现——一个轻柔的动效、一处柔和的高亮——而不是通过强烈的品牌色或动画轰炸来宣示自己。

二、颜色体系
主色系：雾感玉石青（Misty Jade）

色阶
名称
色值
用途

50
Jade Mist
#F0F7F5
悬浮背景、选中状态底色

100
Jade Veil
#D9EDE8
标签底色、轻量强调背景

200
Jade Soft
#B2D9CF
分割线强调、次级图标

400
Jade Core
#6AADA0
主要可交互元素（按钮、链接、激活态）

600
Jade Deep
#4A8A7E
Hover 状态、深度强调

800
Jade Shadow
#2E6058
文字型按钮、极少使用的深色强调

💡 主色仅用于"需要用户注意"的交互节点，不作大面积铺色。

背景色系：暖灰系（Warm Slate）

名称
色值
用途

Canvas
#F5F3F0
整体应用背景（暖灰基底）

Surface
#FAFAF8
卡片、面板表面（略亮）

Overlay
#F0EDE8
左侧导航背景、右侧面板背景

Sunken
#E8E4DE
输入框底色、内嵌区域

Border Subtle
#DDD9D3
低对比分隔线

Border Soft
#CBC6BE
卡片边框、面板边缘

💡 背景色系整体偏暖，避免纯白或纯灰，维持"纸张感"而非"屏幕感"。

文字色系（Text on Warm Background）

名称
色值
用途

Text Primary
#2C2A27
正文、标题，主要阅读内容

Text Secondary
#6B6760
辅助说明、时间戳、标签文字

Text Tertiary
#A09C96
占位符、禁用状态、低优先级提示

Text Inverse
#FAFAF8
深色背景上的文字

功能色（Functional, Low Saturation）

名称
色值
说明

Success
#7BAF8E
柔和绿，避免荧光感

Warning
#C9A96E
暖琥珀色，不刺眼

Danger
#B87A7A
降饱和玫瑰红，不强烈

Info
#7A9EBA
雾蓝，与主色不冲突

💡 所有功能色均做降饱和处理，与整体暖灰系融合，不产生视觉跳出感。

阴影系统（Shadow as Depth, Not Drama）
Shadow XS: 0 1px 2px rgba(44, 42, 39, 0.04) → 卡片静态
Shadow S: 0 2px 6px rgba(44, 42, 39, 0.06) → 卡片 Hover
Shadow M: 0 4px 12px rgba(44, 42, 39, 0.08) → 浮层、Dropdown
Shadow L: 0 8px 24px rgba(44, 42, 39, 0.10) → Modal、抽屉

💡 阴影颜色基于暖黑（而非纯黑），透明度极低，创造"托起"而非"切割"的感受。

三、间距体系
采用 4pt 基础网格，以 8pt 为核心单位递增，形成有节奏的空间语言。
Space-1: 4px → 图标与文字的内部间距、极细间隙
Space-2: 8px → 标签内边距、行内元素间距
Space-3: 12px → 小组件内边距（pill、badge）
Space-4: 16px → 标准组件内边距（按钮、输入框）
Space-5: 20px → 卡片内边距（紧凑型）
Space-6: 24px → 卡片标准内边距、区块间距
Space-8: 32px → 区域分隔、模块间距
Space-10: 40px → 大区块顶部留白
Space-12: 48px → 页面级顶部间距
Space-16: 64px → 空状态图示与文字间的垂直间距

三栏布局间距规范
左侧导航栏宽度: 220px（固定）
中间内容区: 弹性，最小 480px，最大 860px
右侧信息面板宽度: 280px（固定，可收起）
栏间分隔空间: 1px Border + 无间距（紧贴，靠背景色区分）
内容区水平内边距: 24px 左右
页面顶部内边距: 20px

四、圆角体系
整体偏向中等圆角，传递温和感，避免矩形的冷硬与大圆角的卡通感。
Radius-XS: 4px → 标签(Tag)、代码块、Tooltip
Radius-S: 6px → 输入框、下拉框、小型按钮
Radius-M: 8px → 标准按钮、卡片、对话气泡（常用）
Radius-L: 12px → 浮层面板、Modal、信息卡片
Radius-XL: 16px → 侧边栏激活块、大型容器卡片
Radius-2XL: 24px → AI 消息气泡（主对话区强调温度感）
Radius-Full: 9999px → Pill 标签、头像、状态指示点

💡 同一层级的组件保持统一圆角，不混用。导航区略小圆角（6px），内容卡片中圆角（8–12px），对话气泡大圆角（16–24px）形成层次感。

五、字体层级
字体选型
中文字体栈:
正文 → "PingFang SC", "Noto Sans SC", system-ui
代码 → "JetBrains Mono", "Fira Code", monospace

英文字体栈:
正文 → "Inter", "SF Pro Text", system-ui
展示型 → "Inter", "SF Pro Display"（大号标题使用）

💡 优先使用系统字体保证跨平台一致性；中英混排时，英文字体优先处理西文字符，中文字体兜底。

字号与字重层级

层级
用途
字号
行高
字重
字间距

Display
空状态大标题（极少用）
28px
36px
300
-0.3px

H1
页面主标题
22px
30px
500
-0.2px

H2
区块标题、面板标题
18px
26px
500
-0.1px

H3
卡片标题、列表组标题
15px
22px
500
0

Body L
主要正文（对话内容）
15px
26px
400
0

Body M
标准正文、描述
13px
22px
400
0

Body S
辅助说明、时间戳
12px
18px
400
0.1px

Caption
标签、极小提示
11px
16px
400
0.2px

Code
代码片段
13px
22px
400
0

字重使用原则
300 Light → 仅用于大号展示文字，增加轻盈感
400 Regular → 所有正文内容的基准字重
500 Medium → 标题、激活导航项、按钮文字
600 Semibold → 仅用于需要强调但不超过2处的关键信息
700 Bold → 禁止在此设计语言中使用（破坏温和感）

六、组件设计规则
6.1 导航栏（左侧，220px）
背景色: Overlay (#F0EDE8)
右侧边框: 1px, Border Subtle (#DDD9D3)

顶部 Logo 区:
高度: 56px
内边距: 0 16px
Logo 尺寸: 24px 图标 + 文字，字重 500

导航分组:
组标题: Caption 级别，Text Tertiary 色，全大写字母，字间距 0.8px
组间距: Space-6 (24px)

导航项:
高度: 36px
圆角: Radius-M (8px)
内边距: 0 12px
图标尺寸: 16px，与文字间距 Space-2 (8px)
文字: Body M，Text Secondary

静态: 透明背景
Hover: 背景 Jade Mist (#F0F7F5)，文字 Text Primary
激活: 背景 Jade Veil (#D9EDE8)，文字 Jade Deep (#4A8A7E)，
字重 500，左侧无指示条（用背景色区分即可）

底部用户信息区:
高度: 52px
头像: 28px，Radius-Full
分隔线: 1px Border Subtle，距顶 Space-2

6.2 卡片（Card）
背景: Surface (#FAFAF8)
边框: 1px, Border Soft (#CBC6BE)，透明度 0.6
圆角: Radius-L (12px)
阴影: Shadow XS（静态），Shadow S（Hover）
内边距: Space-6 (24px)

标题区:
字级: H3
下方间距: Space-4 (16px)
下方分隔: 可选，1px Border Subtle

过渡动画:
属性: box-shadow, background-color
时长: 180ms
曲线: ease-out

禁止项:
× 不加粗色彩边框
× 不使用投影颜色带颜色（如蓝色阴影）
× 不在卡片内叠加卡片（最多一层嵌套）

6.3 按钮（Button）
Primary 按钮
背景: Jade Core (#6AADA0)
文字: Text Inverse (#FAFAF8)，Body M，字重 500
圆角: Radius-M (8px)
内边距: 10px 20px
Hover: Jade Deep (#4A8A7E)
Active: Jade Shadow (#2E6058)
过渡: 150ms ease-out

Secondary 按钮
背景: Jade Veil (#D9EDE8)
文字: Jade Deep (#4A8A7E)，字重 500
圆角: Radius-M (8px)
内边距: 10px 20px
Hover: 背景 Jade Soft (#B2D9CF)

Ghost 按钮
背景: 透明
边框: 1px Border Soft
文字: Text Secondary
Hover: 背景 Canvas (#F5F3F0)，文字 Text Primary

尺寸规范
SM: 高度 28px，内边距 6px 14px，字号 12px
MD: 高度 36px，内边距 10px 20px，字号 13px（默认）
LG: 高度 44px，内边距 12px 24px，字号 15px

💡 禁止使用超过两种按钮类型出现在同一视图区域。同组操作的按钮保持统一高度。

6.4 输入框（Input）
背景: Sunken (#E8E4DE)
边框: 1px Border Subtle（静态）→ 1px Jade Soft（聚焦）
圆角: Radius-S (6px)
内边距: 10px 14px
文字: Body M，Text Primary
占位符: Text Tertiary

聚焦环: box-shadow: 0 0 0 3px rgba(106, 173, 160, 0.18)
（柔和的玉石青光晕，不是标准的蓝色 outline）

前缀/后缀图标: 16px，Text Tertiary 色，与输入文字间距 Space-2

Disabled 状态:
背景: Border Subtle (#DDD9D3)
文字: Text Tertiary
不显示聚焦态

6.5 AI 对话气泡（Chat Bubble）
AI 消息气泡:
背景: Surface (#FAFAF8)
边框: 1px Border Soft
圆角: 24px 24px 24px 6px（左下角小，体现"说话"方向感）
内边距: 14px 18px
文字: Body L，Text Primary，行高 26px
阴影: Shadow XS
最大宽度: 72% of 内容区宽度

用户消息气泡:
背景: Jade Veil (#D9EDE8)
圆角: 24px 24px 6px 24px（右下角小）
内边距: 14px 18px
文字: Body L，Text Primary
最大宽度: 66% of 内容区宽度
靠右对齐

气泡间距:
同一方连续气泡: Space-2 (8px)
换方气泡间距: Space-5 (20px)

AI 头像/标识:
尺寸: 28px，Radius-Full
颜色: Jade Core 底色 + 白色图标
位置: 气泡左侧，与第一条气泡顶部对齐

打字中动画（Typing Indicator）:
三个圆点，尺寸 6px，间距 4px
颜色: Jade Soft (#B2D9CF)
动画: 垂直位移 4px，依次错开 160ms，ease-in-out 600ms 循环

6.6 右侧信息面板（280px）
背景: Overlay (#F0EDE8)
左侧边框: 1px Border Subtle
内边距: Space-5 (20px) 水平，Space-6 (24px) 垂直

面板标题:
字级: H3，Text Primary
下方间距: Space-4

信息模块间距: Space-6 (24px)

属性行（Label + Value）:
Label: Caption，Text Tertiary，字间距 0.3px
Value: Body M，Text Primary
行高: Space-6 (24px)

标签（Tag/Badge）:
背景: Jade Mist (#F0F7F5)
边框: 1px Jade Soft
文字: Caption，Jade Deep
圆角: Radius-XS (4px)
内边距: 2px 8px
间距: Space-1 (4px) between tags

6.7 分隔线与层级区分
强分隔（栏间）: 1px, Border Soft (#CBC6BE)，不加间距
弱分隔（区块内）: 1px, Border Subtle (#DDD9D3)，上下各 Space-4 (16px)
纯空间分隔: 不绘制线，直接用 Space-8 (32px) 间距区分

禁止使用:
× 虚线分隔（破坏精致感）
× 彩色分隔线
× 双线分隔

6.8 状态与反馈
Loading Skeleton:
颜色: Border Subtle → Border Soft 渐变
圆角: 与目标组件一致
动画: 从左到右光泽扫过，1.4s linear 循环，透明度 0.4→0.8→0.4

空状态（Empty State）:
图示: 48px 线描图标，Jade Soft 色（#B2D9CF）
标题: H2，Text Secondary
描述: Body M，Text Tertiary
间距: 图示下方 Space-6，标题下方 Space-2
整体垂直居中于容器

Toast / 通知:
位置: 右下角，距边 Space-6
背景: Surface
边框: 1px Border Soft
圆角: Radius-L (12px)
阴影: Shadow M
左侧: 3px 实色条（颜色对应功能色）
进出动画: translateY(8px) + opacity 0→1，200ms ease-out
自动消失: 3000ms
最大宽度: 320px

设计语言总结
核心意象：雾中书房，玉石案台
温度来源：暖灰 + 低饱和主色 + 柔和阴影
克制来源：有限的强调色用量 + 低字重 + 充足留白
结构来源：一致的间距节奏 + 明确的字级层级 + 三栏职责分明
耐看来源：无高对比冲突 + 动效时长短而自然 + 颜色不疲劳眼睛

调色方案说明
Sidebar（左侧面板）— #F8F9F9 系近中性冷灰白
变量 旧值（暖沙米） 新值（冷灰白）
--sidebar oklch(94% 0.012 75) oklch(97.5% 0.005 195)
--sidebar-accent oklch(89% 0.018 75) oklch(93% 0.010 195)
--sidebar-border oklch(85% 0.016 75) oklch(89% 0.010 195)
设计逻辑：明度提至 97.5% 使肉眼感知接近 #F8F9F9，色相转回 195（品牌青调），饱和度极低（0.005），与主背景青白系统融为一体，层级感来自 0.5% 明度差异而非色相跳变——这是 Figma / Linear 等产品常用的「同色系微差分层」手法。

Inspector（右侧面板）— 暖沙米白
变量 值 说明
--inspector oklch(94% 0.012 75) 用户指定色，偏黄暖
--inspector-border oklch(87% 0.016 75) 与面板同色系暖沙线
设计逻辑：左冷（青白）→ 中性（主背景）→ 右暖（沙米），形成自然的「冷↔暖温度渐变」视觉叙事，Inspector 区域的暖意也在心理上暗示「当前项目上下文 / 属性」的亲密感与工作氛围。
