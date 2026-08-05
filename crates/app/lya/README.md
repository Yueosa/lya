# lya

lya 进程入口：组装配置、启动 HTTP 服务、可选系统托盘。

## 职责

- 加载 `~/.lya/` 配置并初始化数据库迁移
- 注册工具、动作、agent，交给 `lya-core` 启动——`SessionHub` 是 `lya-hub` 的，
  在那里被组装起来
- 可选托盘图标（退出、打开浏览器）

## 用法

```bash
lya          # 前台运行，监听 core.toml 配置的端口
```

实现细节见 [`lya-core`](../lya-core/README.md)。
