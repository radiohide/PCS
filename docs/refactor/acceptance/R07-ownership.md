# R07 验收记录：划分代码与目录所有权

| 字段 | 内容 |
| --- | --- |
| 步骤与状态 | R07 · 待验收 |
| 实现范围 | 五条工作线（方案四条 + 一条既有实现）的文件责任边界；五处共享入口各设唯一整合责任人；类型与来源的实现归属；迁移号归属；生成物登记；共享改动登记格式 |
| 代码定位 | 新增 `model/ownership.json`、`scripts/check-ownership.cjs`、`docs/refactor/ownership.md`、`docs/refactor/shared-changes/0001-catalog-1.1.0.md` |
| 验证环境 | Node v24.20.0；Git 2.55.0（PortableGit，不在 PATH，由检查自行解析） |
| 验证过程 | `node scripts/check-ownership.cjs`；三次注入测试 |
| 结果 | 205 个文件恰好各归一处，0 遗漏 0 重复；12 个类别与 128 个来源各有唯一实现归属；15/15 检查通过 |
| 遗留问题 | 见第 5 节 |
| 下一步 | R08：固定工作区与集成策略 |

## 1. 交付物

| 交付 | 位置 |
| --- | --- |
| 文件责任表 | `model/ownership.json` 的 `workstreams` 与 `shared_entry_points` |
| 类型边界表 | `docs/refactor/parallel-boundaries.md`（R06 前已生成，本步纳入归属） |
| 共享改动登记格式 | `docs/refactor/ownership.md` 第 4 节，首个真实条目 `shared-changes/0001-catalog-1.1.0.md` |
| 强制检查 | `scripts/check-ownership.cjs`（第 16 个检查） |

## 2. 认领结果

```text
Ownership: 5 workstreams, 5 shared entry points
Tracked files 205, claimed 205, unclaimed 0
   58  工作线 existing-runtime
   39  共享入口「文档与仓库元数据」（整合责任人 base-data）
   32  工作线 query-ui
   29  工作线 type-adapters
   15  共享入口「公共契约」（整合责任人 base-data）
   10  工作线 base-data
   10  共享入口「中立模型目录」（整合责任人 base-data）
    8  工作线 migration
    4  共享入口「契约检查」（整合责任人 base-data）
    1  共享入口「数据库 schema 与迁移号」（整合责任人 base-data）
Type owners 12 categories; adapter rules 32 covering 128 sources
Schema version writers: pcscope/crates/storage/src/lib.rs、pcscope/crates/storage/src/model.rs
```

## 3. 三条可机器检查的验收

| 计划验收 | 状态 |
| --- | --- |
| 重叠类型只有一个主实现归属 | 强制：每个类别一个实现步骤；每个来源恰好匹配一条适配器归属；跨类别的来源（如日志通道产出 12 个类型）适配器仍只有一个归属 |
| 无人自行分配迁移号 | 强制：只有声明路径内可写 `PRAGMA user_version`，且声明路径必须确实在写 |
| 生成目录不手工修改 | 强制：每个生成物登记生成器，生成器文件必须存在 |

**注入测试（三次，全部使检查以非零码失败并指出对象）**：

| 注入 | 结果 |
| --- | --- |
| 新增一个无人认领的文件 | `有 2 个文件没有任何归属，例如：…ownership-probe.rs…` |
| 在声明路径之外写 `PRAGMA user_version` | `pcscope/crates/contracts/src/lib.rs 写入 PRAGMA user_version，但它不在声明的迁移号归属内` |
| 删掉一个类别的类型归属 | `类别 network 没有类型归属` |

注入后均已还原，最终检查通过。

## 4. 过程中修正的问题

**1. 检查只枚举已跟踪文件，抓不到新文件。** 第一次注入测试**没有失败**——`git ls-files` 只列已跟踪文件，而一个新建的未跟踪文件根本不在枚举范围内。这恰恰是它最该抓的时刻：文件即将被提交、还没人决定谁负责。已改为 `git ls-files -co --exclude-standard`（已跟踪 + 未跟踪且未被忽略）。

**2. 适配器归属允许隐式前缀，造成一个来源两个归属。** 最初 `pcscope.software` 把 `pcscope.software.appx-user` 也认领了。已改为默认精确匹配，只有以 `.*` 结尾的规则才做前缀匹配。

**3. 迁移号声明的路径不全。** 我最初只声明了 `storage/src/model.rs`，而检查发现**迁移块实际在 `storage/src/lib.rs` 执行**——`lib.rs` 才是写 `PRAGMA user_version` 的地方。已声明两处，并增加反向检查：声明路径必须确实在写，防止声明过时。

**4. 生成物与工作线范围的冲突。** `collection-catalog.json` 由既有采集工具生成，却位于界面目录内。原检查要求生成物不得被工作线认领，这条规则过于死板——真正要保证的是「生成物有登记的生成器」，而不是「生成物不能位于某个目录下」。已改为只强制前者。

**5. 六处文件无人认领**：五个契约示例与 `contracts/src/model.rs`。已并入「公共契约」共享入口。

**6. `dist/**` 被声明为主责范围但没有任何匹配**（dist 被 gitignore）。已移除并改为在说明中体现。

**7. 检查脚本自身被 git 当成二进制文件。** 提交时 git 报告 `check-ownership.cjs | Bin 0 -> 9652 bytes`。原因是 `globToRegExp` 里用一个字符做 `**` 的占位符，而那个位置实际写入的是 **NUL 字节**而不是空格。

一个被 git 判定为二进制的源码文件不能 diff、不能合并——对并行开发是硬伤，而这一步本身就是关于并行边界的。已改为逐字符扫描生成正则，不再需要占位符，因此这类问题从结构上不会重现。同时扫过全部已跟踪文本文件，确认没有第二处 NUL。

该提交当时只在本地、未推送，因此直接修订而不是追加提交，避免历史中留下一个不可读的源文件 blob。**这是本步唯一一次改写提交历史。**

## 5. 遗留问题

1. **文件责任表只到目录与文件级别**，不细分到函数或结构体。工作线数量增加时需要更细的边界。
2. **分支与目录隔离未定义。** 本步只定义「谁负责协调」，不保证并发修改不冲突——那是 R08 的内容。
3. **登记是流程约定，不是机器门禁。** 检查不验证共享改动都有登记条目；是否补齐靠评审。
4. **`existing-runtime` 这条工作线不在方案里。** 它是我为消除无人认领空白而加的；如果方案方认为应并入四条之一，应调整清单并说明协调责任。
5. **适配器归属的步骤号是预估的。** 32 条规则里的 S09–S21 按计划的类别顺序分配；若 S09–S20 的实际边界与预估不同，规则需随之调整。
6. **`crates/adapters/` 尚未创建。** 它在清单里是 `reserved`，检查会在其中出现文件时要求移入 `owns`。

## 6. 下一步

R08 的前置（R07）已满足。R08 需要固定工作区与集成策略：分支与目录隔离、独立测试数据库约定、集成分支流程，并完成一次并发无冲突的验证。Git 侧的条件在 R01 已具备（git 2.55.0、仓库根 `SYS/SSS`、全部产出已提交）。
