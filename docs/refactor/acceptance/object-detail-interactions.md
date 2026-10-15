# 系统对象详情交互验收

日期：2026-10-13。

## 原因与修复

列表“查看”按钮只输出行号属性 `data-object`，页面却只给 `data-object-alias` 按钮绑定点击事件。修复前，实际加载 `index.html` 与 `app.js` 后点击“查看”，详情查询次数和展开行数均为 0；记录位于 `target/object-interactions-before.json`。

列表现在把原始对象 alias 正确转义后保留在按钮属性中，查询按原身份执行，默认文字仍不显示内部标识。页面为新按钮分配跨追加页唯一的详情区域编号，已绑定按钮不重复绑定。读取失败可原地重试；同一按钮正在进行的读取可在快速收起、展开时复用，避免重复请求。已收起或离开页面的详情不会被较晚到达的结果重新打开。

保留现有刷新语义：自动状态轮询与采集结束不会重绘系统对象列表；用户主动“载入最新”重新载入列表并收起详情。列表刷新不会为已展开对象批量发出详情查询。

后续同日修复补齐 `account`、`file`、`runtime` 的中文类别名称，并为明显偏技术的类型提供简短解释。类别和类型的请求值保持原值。系统对象列表发生中文或英文 `querytimeout` 错误时，默认提示使用本页实际存在的“类别、类型、名称”，原始错误保留在折叠详情中。默认列表仍可直接重试，不强制筛选，也没有延长查询时限。

## 可重复检查

`scripts/check-object-interactions.cjs` 启动本地静态 HTTP 服务，在无界面 Chrome 中运行实际桌面页面和事件处理代码。IPC 由只读固定响应替代；脚本不启动后台、不采集、不申请 UAC。

Playwright 是测试工具，不是产品运行依赖。通过环境变量指定已安装工具：

```powershell
$env:PCSCOPE_PLAYWRIGHT_MODULE = '<Playwright 模块目录>'
$env:PCSCOPE_CHROME_PATH = '<Chrome 可执行文件>'
node scripts/check-object-interactions.cjs
```

若 Playwright 可由标准 Node 模块解析、且已经安装其浏览器，可不指定这些变量。固定场景检查普通对象、缺名登记、同名字典对象、加载更多、收起、失败重试、分页控件唯一性、快速开合、自动轮询、手动刷新、重排序及筛选变化。原始 alias 含引号、尖括号和 `&` 的场景验证属性转义、准确取回与默认隐藏。

本轮另外输入本机后台只读返回的列表和三类详情，用同一个实际页面点击流程验收：

```powershell
node scripts/check-object-interactions.cjs --live-list target/dictionary-live-software.json --live-details target/object-detail-live-responses.json
```

列表输入为 IPC 外层 `{ data: QueryResponse }`，详情输入为 `{ kind, response: QueryResponse }` 数组。这些输入是已保存的真实返回值；浏览器测试本身不连接实时后台。

## 结果

- 15 个点击场景通过，含本机来源名称对象 29、字典名称对象 32、缺名对象 5。新增默认列表超时后无筛选重试、错误后的类别/类型/名称筛选、中文类别与类型解释检查。
- 浏览器未报告脚本错误，未发生采集或配置写入调用。
- 名称显示、模型响应路径、查询消费者、状态文案、刷新行为、采集展示六个已有检查通过。筛选更换列表后再释放旧详情响应的场景也通过，旧结果不会重新展开或串到其他对象。
- 结果：`target/object-interactions-result.json`；展开缺名对象的截图：`target/object-interactions-live.png`。

独立复核另外通过 12 个场景，包含真正延迟的详情响应在筛选替换列表后返回，不会恢复旧行或关联到新对象。记录为 `target/object-detail-independent-review-result.json`。24 个既有 Node 检查全部通过；其中来源映射生成器启动子进程被普通工作区沙箱阻止，改在普通用户执行环境复跑通过，未放宽检查。

后台只读实测：来源名称对象 29 用时 143 ms、字典对象 32 用时 69 ms、缺名对象 5 用时 76 ms，均返回原对象身份。类型目录为 156 ms，默认全部对象 50 行为 5062 ms；后者接近 6 秒预算，和状态读取超时、存储增长等已另行记录为 [待修复的运行问题](object-detail-runtime-notes.md)。

上述浏览器验收覆盖前端真实事件处理与保存的后台响应，不代表打包后的原生窗口验收；最终桌面构建与运行结果将在下节记录。

## 最终构建与运行

桌面构建通过，保留此前的 MinGW 多 manifest 链接警告。新包 `dist/object-details-fixed` 含 29 个文件，逐文件复制校验一致；与字典版本比较，只有 `pcscope-desktop.exe` 改变，后台及采集器未替换。清单见 [产物校验](object-detail-artifacts.json)。

新版从普通用户应用目录启动，界面 PID 14732；旧界面 PID 6116 正常关闭。后台继续使用 PID 20844 和原数据目录，没有申请 UAC、重启后台或改变采集设置。界面启动记录为 `target/object-details-desktop-launch.json`，进程与窗口核对为 `target/object-details-runtime-verification.json`。运行核对确认窗口响应和唯一后台；12 场景点击证据来自实际页面代码在 Chrome 中执行，没有将其说成对原生 WebView 窗口的自动点击。
