# 项目工作约定

- 日常开发包固定使用项目下的 `dist/dev`，运行入口为 `dist/dev/pcscope-desktop.exe`。后续更新统一覆盖此目录，不因功能或修改内容另起构建目录。
- 日常运行及性能验收包的 Rust 后台和运行依赖使用标准 `target/release` 优化构建，打包时显式指定 `scripts/stage-dev.ps1 -RuntimeProfile release`；该脚本默认的 debug 模式继续用于已有开发测试。正常目录和桌面 debug 入口都可复用同一组优化运行依赖，保留各自路径配置。`target/debug` 中的未优化 Rust 产物供源码调试使用，不再作为日常性能包。
- 桌面 debug 输出固定为 `apps/desktop/src-tauri/target/debug`，保持直接运行所需依赖与已有路径配置。
- 除非用户明确要求独立验收包或版本归档，不创建新的命名发布目录，不改变用户的运行入口。
- 构建产物、历史数据库、日志、机器专属配置和凭据不纳入 Git。沿用 `.gitignore`。
