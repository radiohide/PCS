# Docker 原生管道采集

2026-10-14。新增一项原生路由，当前为 118 原生 / 13 待迁移 / 2 分组，累计迁移 78 项。运行后台尚未升级。

后端使用编译的 C# 宿主，以 GET 请求读取固定本机 `docker_engine` 管道中的 Engine API。先查询 `/version`，在服务端范围和客户端支持的 1.24–1.55 之间选择版本；不使用 Docker CLI、PowerShell、远程地址或用户 Docker context，不启动 Docker。该项仍要求用户显式开启。版本机制见 [Docker Engine API](https://docs.docker.com/reference/api/engine/)。

保留容器、镜像、卷和网络四类对象及原字段、原范围和 16 分片。容器名称移除一个前导斜杠并以逗号连接；镜像按照仓库/标签分别登记，保留无标签、仅摘要和带端口仓库，避免同镜像多个标签被合并。字段映射核对了 Docker 官方 [容器格式实现](https://github.com/docker/cli/blob/master/cli/command/formatter/container.go) 和 [镜像格式实现](https://github.com/docker/cli/blob/master/cli/command/formatter/image.go)。不采集命令、环境、日志、挂载内容、网络选项或标签中的敏感数据。

连接最多等待 3 秒，每次请求最多 5 秒，最多 5 个顺序请求；无重试循环。响应上限 8 MiB，集合/对象数量有界。支持 Content-Length、chunked 和关闭连接定界；拒绝不完整返回、异常状态、不兼容版本、错误长度和非法 JSON。全部读取并校验后才发快照，任何中途失败都发 gap 并保留旧状态。关闭配置后调度器不启动该采集、不回退脚本。

验收材料：`target/native-docker-tests/1791978174792/summary.json`。随机隔离命名管道返回 9 条独立预期对象，身份和全部允许字段一致；普通响应与分块响应通过。拒绝响应、超限、截断、5 秒超时、不兼容版本、非法 JSON 和缺失卷字段均不产生快照。错误输出不泄露测试敏感内容。包装器关闭开关验证通过；隔离运行目录无兼容脚本。调度器单元测试、来源清单与迁移清单检查通过。Mihomo 开关及 HTTP 回归通过，见 `target/native-mihomo-tests` 中本轮 summary。

环境边界：本机没有 Docker Desktop CLI，因此未完成真实 Engine 的成功读取和 CLI 同时刻字段比对。隔离协议验证证明实现行为，不能代替 Docker Desktop Linux/Windows 容器、各目标版本及普通/增强权限矩阵的实际验收。这些门槛继续开放。测试没有连接真实 Docker 管道、创建容器或修改正式数据库。

测试命令：`node scripts/test-native-docker.cjs --wrapper`。测试专用入口只允许随机 `pcscope-docker-test-` 加 32 位十六进制名称，生产管道不接受配置覆盖。
