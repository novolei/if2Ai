# Tauri Application

一个基于 Tauri 框架构建的现代化桌面应用程序。

## 📋 目录结构

```
├── src/                    # 前端源代码
├── src-tauri/              # Rust 后端代码
│   ├── src/
│   │   └── main.rs         # 应用入口
│   ├── Cargo.toml          # Rust 依赖配置
│   └── tauri.conf.json     # Tauri 配置文件
├── capabilities/           # Tauri 能力配置
├── gen/                    # 生成的代码
├── icons/                  # 应用图标
├── build.rs                # 构建脚本
└── tauri.conf.json         # Tauri 根配置
```

## 🛠️ 技术栈

- **前端**: HTML, CSS, JavaScript
- **后端**: Rust
- **框架**: Tauri
- **构建工具**: Cargo, npm/pnpm

## 🚀 快速开始

### 环境要求

- Node.js >= 16
- Rust >= 1.60
- npm 或 pnpm

### 安装依赖

```bash
npm install
```

### 开发模式

```bash
npm run tauri dev
```

### 构建应用

```bash
npm run tauri build
```

## ⚙️ 配置

主要配置文件位于 `src-tauri/tauri.conf.json`，可配置：
- 应用名称和版本
- 窗口属性
- 权限和安全设置
- 构建选项

## 📦 了解更多

- [Tauri 官方文档](https://tauri.app/)
- [Rust 官方文档](https://doc.rust-lang.org/)
