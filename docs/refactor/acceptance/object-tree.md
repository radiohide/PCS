# 系统对象树形视图验收

日期：2026-10-14。完整开发包 `dist/object-tree`。桌面 SHA256 `7DB5636E23E03F3CB7EE58C31855D5B3402B568C8B7AF9D3DC59CCBA79C8DDF2`。

## 行为

系统对象增加“列表｜树形”，并记住显示方式。树按类别、类型、对象分层，右侧复用现有详情与默认折叠的技术资料。名称、状态和筛选规则均与列表共用；每次读取 50 条，继续分页进入同一分支，不一次加载所有对象。

普通浏览使用后台类型目录，展开类型后读取该分支；未识别类别归到“其他”。搜索使用与列表完全相同的 instance_list 查询及游标，将返回的匹配对象按类别和类型归组，保留父级。搜索结果继续分页，不局限于已打开的分类。历史对象由现有“记录状态”筛选控制。

已读取的分支收起后再次展开不重复请求。展开状态和已选对象在同一筛选条件的页面重建中尽量恢复；选中对象不在当前已加载页时仍需继续翻页。查询继续使用既有串行队列，快速换筛选后旧结果不能覆盖新视图。失败提供局部重试；不新增采集、后台接口、数据库写入或 PowerShell 调用。

这是一棵分类树；程序与模块的从属关系未在本次新增嵌套。目录为空或读取失败时可按全部结果分页浏览。正常分类树依赖后台完整类型目录；当前数据库已验证没有缺目录类型或分类不一致的对象。

## 验证

- `check-object-tree-ui.cjs` 15 项：按需查询、分支分页、共用详情、缓存展开、历史筛选、未知类别、转义、全部对象覆盖、搜索层级与分页、列表树形对象集合相等、失败重试、空结果、过期响应、深浅主题与小窗口、查询串行。
- `check-status-issues-ui.cjs` 11 项回归通过；`check-front-refresh.cjs`、`check-product-status-copy.cjs`、`check-model-view.cjs`、`check-query-consumers.cjs` 通过。文件系统清单所有权检查通过，该工作目录没有 Git 元数据。
- 对当前库只读审计：33,992 个对象，74 个有数据的类型/类别组合，缺目录类型 0、类别不一致 0，约 140 ms。记录在 `target/object-tree-catalog-audit.json`。这是分类覆盖审计，没有将逐条交互检查描述成全部对象实机遍历。
- 原生 WebView PID 17628 实机：101 种目录类型全部出现在树里；电源计划按需读取及详情通过，列表与树形对象集合一致；切换视图恢复选中对象；电池 L21M3PD4 详情通过，无页面异常。后台仍为 PID 14304、增强权限。结果及截图在 `target/object-tree-live.json`、`target/object-tree-live.png`、`target/object-tree-device-live.png`。
- debug 构建及依赖准备成功。MinGW 既有多 manifest 链接警告仍在，未将其计为本次已修复项。普通应用目录为 `C:\Users\radiohide\AppData\Local\Programs\PCScopePreview\AAB7C15946A5F9F3`，29 个文件校验及两处快捷方式同步完成。

## 历史提示清理

2026-10-14 16:42，后续包 `dist/history-notice-cleanup`。用户要求移除没有具体补全办法的历史完整性判断。原对象提示由来源 history_state 推出，未逐对象识别缺失变化；本次删除对象警告、来源历史结论及其对总览当前异常计数的影响。日志检查改为实际读取结果/分批补采状态，历史采集中断与恢复记录只描述当时活动，不再推断过去遗漏。原始材料保留，不将提示删除描述为历史已经补全。

更新并通过 product-status-copy、account-presentation、collection-view、model-view、query-consumers 检查；当前采集失败和解析失败仍保留。原生 WebView PID 3496 验证电源计划详情不含历史完整性警告，树形、列表切换及电池详情通过，无页面异常，后台 PID 14304 未重启。结果和截图在 `target/history-notice-live.json/png`。
