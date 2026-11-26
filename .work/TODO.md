# TODO

> **SiYuan ReadItLater 插件**(0.13.0–0.13.7:camofox-browser REST 集成、标题/粘贴/微信/i18n 修复、发布):记录已迁移至插件仓 GitHub issues —— kamasylvia/SiYuan-ReadItLater,Ledger = #20(pinned),相关 issue #14–#19。本文件不再记录该插件。


> 2026-11-07 重构:双批次结构。已完成事项入 commit 历史 + AGENTS.md 决策记录,不留在本文件。

## PDF 阅读器整页 + 思源式批注(2026-11-26)

- [x] **官方 viewer 整页**:mozilla/pdf.js v6.5.438 `gulp generic` vendor 到 `public/pdfjs`(裁 locale/测试/debug,8MB,布局照搬);`ResourcePreviewModal` 止损删(存活半天,被面板取代);`![pdf]` 与内嵌模式改整页 iframe。
- [x] **思源式批注(只做双链,不写文件)**:`pdf_annotations`＋`annotation_refs`(migration 0016,quads=PDF 点坐标);CRUD/refs API(读=文件 Read,写=创建者或文件 Update,create 限 PDF);`#annotation:` reconcile  ride `reconcile_refs`＋restore 回填;`pdf_annotations_lifecycle` 绿。
- [x] **覆盖层桥**:同源 iframe 注入——官方编辑器按钮 CSS 藏＋事件卫栏锁 NONE;高亮 div 逐页绘制(zoom/rotate 重绘);划选转 quads(单页,64 上限);悬浮看 comment,点开编辑器(改色/改注/复制链接/删/反链跳文档)。
- [x] **两种预览方式**:右侧面板(560px sticky,TOC 让位,Esc 关,跳文档关)为默认;内嵌(链接就地变 viewer,保留静态标题);全局默认设置＋单篇 Metadata 下拉。
- [x] **阅读器打磨**:Metadata 内 PDF 预览改 AppSelect 下拉(与 Parent/Theme 一致);页眉切换钮删(只留文档级设置,顺手删 `previewMode` 死键);内嵌 viewer 上方保留原文标题(静态 div,不可点);默认方式设置 Other→Documents 页(用户审批);Other 续清(用户审批):开发者模式→Advanced、高级视图＋每页条数→Documents、移动确认只留目录齿轮(删 Other 侧),排序＋上传目录留守,死键 `datatable`/`otherSection` 八语同删;面板 flex 吸留白(文档钉宽,面板 460→728,隐藏侧栏时;无面板 800 不变);内嵌改 aspect-ratio 高(一整页,736×1001 实测);Document size 进文档级 Metadata 下拉(跟随默认/S/M/L),全局变默认项(`resolveDocSize`;700 与跟随默认渲染实测,Large 同路未单独渲染)。
- [x] **文档级设置即时生效**:Metadata 选中直写 store 行(PDF/宽度选中→渲染 0ms 实测),防抖 PUT 后台持久;store.update 加 `skipRefresh`(调用方声明 listing 无关),目录块自动保存 ride;编辑器活体绑定保持原样(防目录排序回退)。
- [x] **设置页偶发打不开根修**:用户展开贴出真身 `items is not iterable`(Firefox 报变量名)→ `AppSelect.findSelected` 迭代到 undefined `items`(异步空窗);修组件自卫(`safeItems`,非数组渲空列表＋dev 警告,不再炸整页)。调用方未定位到具体一处(10 余轮无复现),此修覆盖整类。
- [x] **下载含批注**:pdf-lib 现场烧标准 `/Highlight`(QuadPoints＋Contents＋颜色＋Print 标记)副本下载,原文件恒干净;viewer 自带下载=不带批注。
- [x] **坑**:viewer 自带 CSP 杀内联 `<style>`(静态规则改外部 link 注入,CSSOM 写活着);`annotationEditorMode={-1}` setter 不认 DISABLE(改事件卫栏);v6 无 `convertToViewportRectangle`(用 Point 版);`contentFrame().evaluate` 上下文错位(探针改父上下文)。
- [x] **实测**:临时用户全链——划选建批注→落库→重载绘高亮→点高亮开编辑→文档 `#annotation:` 深链跳面板闪烁→内嵌切换→下载复 parse 1 annot→zoom 重绘;探针已删。

## PDF 页内预览 + 本地栈修复(2026-11-26)

- [x] **普通 PDF 链接页内预览**(已被整页面板取代,见上):正文普通 `[]()` PDF 链接拦截弹大窗 `ResourcePreviewModal`(已删);命名 URL 按扩展名判,老裸 `/file` URL 查库 mime;修饰键/中键开新标签;外部 PDF 不拦截。临时用户浏览器实测 5 页 PDF 窗内渲染。
- [x] **本地栈修复**:后端 session 退出后未拉(补起,`:8201` 200);前端 dev 只绑 IPv6(`[::1]:3000`,v4 拒绝致浏览器刷不出),改 `--host` 双栈(`*:3000`,v4 200)。

## 目录树后续 + MCP 二进制资源(2026-11-25)

- [x] **PDF 内嵌不可见根修**:编辑器插的是无文件名裸 URL,扩展名分派永远miss(坏图);现新上传一律命名路由,老裸 URL 按存储 mime 回退分派(缺失即拉, coarse 响应式自动重编)。裸 URL 实测挂载窗口化预览(`451289fb`)。
- [x] **批量栏统一文字按钮**:克隆/移动/复制链接图标改小 secondary 文字按钮,与删除一致(`5e67d9b5`)。
- [x] **Copy to 替复制链接 + 表头计数**:CloneTo 加 copy 模式(递归深拷贝,子快照防自吞,附件留原);表头 `目录 共N篇`(stats.documents)+ Copy/Clone/Move/Delete/Refresh 文字栏;顺手补 sidebarItemsPrefs 缺失声明(消 warning,导航显隐遵设置)(`852f0905`).
- [x] **PDF 独立滚动窗口**:`PDFViewer` 加 `max-height:80vh`,长文档页内翻页不再撑宿主(5 页实测 614px 窗/5292px 内滚)(`5b9d2c39e9`)。
- [x] **MCP 二进制资源(必需)**:`upload_resource` 加 `content_base64`(与 `content` 二选一,mime 按扩展名推断或显式,走 REST 同 allowlist,图床分支与 REST 同语义);`read_resource` 对二进制返元数据 + 下载 URL(后加 base64 分页见下);`update_note` 拒资源改 content(仅改名);单测 + `mcp_resource_binary_roundtrip` 集成绿;spec 同步(`5f49424a` + `8ce32511`)。
- [x] **思源计算摄影整树同步( supersedes 单篇 `01a21180`,已删;后整形见下)**:思源 `20261123063714-akbgu1t` 子树 36 篇 → pergame 新目录 `计算摄影`(folder `01a211ac-e5b5`)+ root-body 文档,全标题(最长 98 字)/正文/时间戳/imported 标记;导出内无内部链接 refs=0;3 PDF 附件(原文重下,思源 file API 只走文本)经 blob 通道入库 + 文档内改命名路由,S3 字节与 db size 对齐。
- [x] **标题 50 字上限解除**:`nodes.name` VARCHAR(50)→TEXT(迁移 0015,search_tsv 重建);REST 校验/MCP+REST 上传截断/前端 8 处 maxlength 同批去;140 字标题 API roundtrip 实测 + FTS 健康;Go 遗留不动,web-clipper 仍自截 50(`b45fb2ec`)。
- [x] **MCP 二进制分页读**:`read_resource` 二进制返 base64 窗口(`offset`/`max_bytes`,默认 256 KiB/单次至多 2 MiB,S3 Range 直取,blob+legacy 双路径)+ 元数据与 URL;文本分支不动;3 PDF 经 MCP 验 magic `%PDF-`;集成测试加切片 roundtrip + 越界拒(`mcp_resource_binary_roundtrip` 绿)。
- [x] **思源树整形(目录文档→folder)**:20 个空容器文档 role 翻 folder(内容留存可逆),根改 folder 直挂[body-doc(0), 人像(1), 通用(2)];终态 21 folder + 16 doc + 3 res,无文档挂非资源子节点;MCP 视角核对。
- [x] **资源不再长箭头**:`has_children` 排除 role 4(children/roots 双查询;目录树与侧栏本就不渲染资源行,删确认走 stats 不受影响);ABPN/Pores 两行箭头消;`paged_tree_reads` 加回归(纯附件文档 `has_children=false`)。
- [x] **目录设置齿轮(复刻 siyuan-auto-directory)**:表头齿轮弹窗,实时生效:最大层级(0 不限)/默认展开层级/默认展开/子计数/修改时间/创建时间/引用行/标签/移动确认(复用既有键);自动展开按深度+48 节点预算逐层懒载;八语 i18n;快捷键项不做(CommandCenter 重叠)。
- [x] **目录块刷新机制(问答)**:无刷新按钮,靠 store 响应式自动刷新——本会话写操作走 `refreshDir` 回填可见页,跨 tab/他人写入需切容器或重进(无轮询/推送)。
- [x] **刷新按钮**:表头加文字刷新键,重拉全部已持目录页 + 本树 refs,保留选择/展开/排序;外部改名实测点按即新(`6f67b422`)。

## 目录树吸纳 siyuan-auto-directory(2026-11-24)

- [x] **A refs 索引**:migration `0014_node_refs`(FK 双向 cascade + SQL 回填),写时 reconcile(挂 repo insert/update,restore 事务回填),`GET /nodes/refs?ids`(100 锚点批量)+`/nodes/:id/refs`(双向,可见性过滤);自链/资源/legacy 数字号跳过。`refs_lifecycle` 集成绿(`3f499ddb`)。
- [x] **B 目录树**:`DirectoryTree/Row` 替换概览目录表(嵌套展开/100 页懒载 + Load more/临时排序置顶优先/置顶/标签/时间/计数/跳转);tree 排序显式镜像服务端(role/order/name);最近浏览表与文档内 children/recent 块不动。i18n 8 语种(`b74c6814`)。
- [x] **C 树操作**:行拖拽(上下沿 reorder,中部 move-into,与侧栏同 mime 可互拖),order 1024-gap(单写 midpoint,耗尽 renumber,pin 排除,不完整层先拉齐);move 确认框 + 不再询问(`directoryConfirmMove` 偏好,设置页可重开);行多选 + 表头全选(仅渲染行)+ 批量删(`6f29a31b`)。
- [x] **D 引用内联**:渲染行单批量拉 refs(容器级缓存),出边去重(自身 + 已在下方的子孙,best-effort)/2 条 + `+N`,referenced-by 同式;点击跳转(`bfa96cff`)。
- [x] **验证**:隔离号逐批活验(树/排序/置顶/导航;reorder 落库 + 显示;move 确认流 + 服务端 parent;多选删 2;去重 `+1` 算术 + 展开 + 单批量);`cargo test --lib` 162 绿;测试号全清。未做:Quick directory 浮层(与 CommandCenter 重叠)、引用路径展示(只名 + 跳转)、refs 跨页实时刷新(切容器重拉)。

## 懒加载消费者迁移 P3a–P3f(2026-11-24)

- [x] **P3a 后端补齐**:neighbors role 过滤断言 + flat/meta/stats/activity/tags/roots?role(随 `a4203431`/`1c658f24` 进仓)。
- [x] **P3b store 链路**:`ensureAncestors`(面包屑/权限/定位),server prev/next,深链 fetch-on-miss(`c4ede2a5`)。
- [x] **P3c 搜索面**:palette Quick/Advanced 走 meta,home 聚合 + stats,侧栏过滤改服务端平铺(`498acb4e`)。
- [x] **P3d 列表页**:docs/分类子树/团队节点走 flat(上限 1000 + `shown/total` 诚实显示),Stats/CDN/卡片/TOC/Footer/详情页 fetch-on-miss(`4ea88911`)。
- [x] **P3e 选择器**:14 处 `getTreeUpToRole` 改 `fetchFolderTree` 异步骨架(上限 2000/role),CloneTo 后代集 + 服务端防环,编辑器 `#` 联想异步 + 内链 fetch-on-miss 重绘,init 拉服务端 tags(`16e32e25`)。
- [x] **P3f 翻转**:init 只拉 roots 页,`fetch()` 改 id-only,`seedDirStates`/`recomputeTags` 退役;`refreshDir`/`refreshLoadedDirs` 保写后可见;switcher/团队页/图片选择/权限 scope/删确认计数/reveal 补齐;MCP `list_children` 分页,activity 加 root/created(`83f42100`)。
- [x] **大库回归(tmpbiglib,1200 文档,已清)**:home 计 1200,启动 0 全量;侧栏展开 + 滚动泵拉齐 12 页(offset 0→1100);docs 页显 `1000/1200`;palette `Zebra` 命中 2。另复用号验过:冷深链、创建即见、删确认计 2、CDN 父列、TOC/footer、团队 insights、MCP 三式(top/parent/limit1)。后端 `cargo test --lib` 160 绿;integration 17 绿,2 失败为 HEAD 既有(trash_lifecycle 计 3vs2、mcp_token 工具表 7vs10,stash 对照确认),另 3 备份类需 `TARGET_TEST_BUCKET=pergame-tgt-test` 才跑(已补跑绿)。

## 侧栏树虚拟滚动+多选(抄思源 DocTreeController,2026-11-24)

- [x] **虚拟化(用户指令"抄思源的虚拟滚动")**:新 `VirtualTree.vue` 替换树区递归——定高实测行高 + 展开感知 flat rows + 开窗(overscan 8)+ 前后 spacer(思源 `getDocTreeWindow` 同式);滚动 rAF 节流,ResizeObserver 跟视口;行渲染复用 `SidebarItem`(拖拽/菜单/图标全保留),缩进改 flat `paddingLeft`,chevron 样式自带(备注 CollapseItem 同源)。另抄:展开入场 fade(仅 transform/opacity,不碰行高数学;`prefers-reduced-motion` 跳过)+ ctrl/shift 多选(位置键,视觉 `selection-color`,Delete/Backspace 调现成批量删 modal,Esc 清空,跨变更自动剪枝,输入框/编辑器/弹窗内不劫持)。不抄:异步分批(思源按需翻页内核数据,pergame 全量已在内存,无事可做)+ 键盘导航。reveal 联动改 `ensureVisible`(按位滚+nearest 抛光);修 reveal 时序真 bug(导航时 store 清空致单次尝试落空——改有界重试 30×200ms,旧 tree watcher 退役)。验证:隔离浏览器 150 节点——挂载 36 行 spacer 正确、滚底换窗、展开见 34 行 `.vt-enter`、ctrl×2+shift 范围精确+URL 不变、批量删 modal 走取消/确认双路、折叠后直链 reveal 回归绿;eslint 过。附带发现:`.sidebar` 嵌套了两层(Resizable 外壳 + 真滚动容器),测试选择器须取内层。

## 内链 UUID 规范修复(2026-11-24)

- [x] **正则认 UUID(用户指令,顺手)**:markdown-it 规则与编辑器装饰器共用 `#(\d{15,20})`,而编辑器插入的是 `#<uuid>` → 预览永远不链化。修 = 正则加 UUID 分支(大小写 hex,旧数字兼容保留),双文件收敛到 `helpers/markdown/internal-links.ts` 单一源(ruler 改 anchored 单点匹配,编辑器改 import),新 `internal-links.test.ts`(node:test 同仓惯例)6 例——UUID/未知回退/旧数字/大写/四种不匹配/全局扫描;`bun test` 9/9 绿(含 counters 回归)。

## 文件树打开定位(2026-11-23 新功能)

- [x] **开笔记自动展开定位(用户需求:非树打开时树还折叠着)**:`Sidebar.vue` 监听 docs/categories 路由 id → `getAncestors` 展开祖先链(`setExpanded`,持久化复用) → `nextTick` 后把 exact-active 行 `scrollIntoView({block:'nearest'})`(已在视口则不动);categories 编辑页用非 exact 回退(`.nav` 图标链排除)。`revealedFor` 守卫防后台刷新乱滚;树异步加载期由 tree watcher 重试;跨 workspace/过滤态无行则静默跳过(不切 workspace)。验证:隔离浏览器登录临时号(工作区>文件夹>笔记三层)——页内折叠文件夹后直链开笔记,断言祖先展开 + 唯一定位行 exact-active + in-viewport,REVEAL OK;eslint 过。测试号/节点已清。

## 单剪阶段进度修复(web-clipper 0.8.6,2026-11-23)

- [x] **单剪过程零提示(用户报:剪藏成功了,但过程中依然没提示,只有成功时有)**:0.8.4 只修了批量 tick;单剪 proxy 路径 `clip()` 的 `onProgress` 参数从未被调用(死参,进度只走知乎 render),微信文章 10 秒静默。修 = `clip()` 对 URL 加两阶段 tick(解析前 `正在抓取：短链接` 5% → 建笔记前 `正在保存…` 80%,纯文本秒完不打扰),TEXTS `stageFetching/stageSaving` ×3 语言;批量内单剪 tick 与 i/n 逐条交错,同 toast 原地滚。harness 加场景 D(单剪三 toast 顺序)后四场景 18 断言全绿;重建产物 + 同批同步 plugin-packs;0.8.6 已装 live env(registry+entry 包双验,临时提权号删,ADMIN_ACCOUNTS 恢复 laniakea 单列)。用户侧硬刷重剪即见过程提示。

## web-clipper 0.8.5 装机 + ADMIN_ACCOUNTS 恢复(2026-11-23)

- [x] **本地 dev 装 0.8.5(用户报:硬刷后剪藏仍失败)**:硬刷无用——运行包取自 S3 安装(0.8.3),非仓库。装机需 admin,但库里无 admin(local.env 丢了 `ADMIN_ACCOUNTS`,而 Go/Rust 启动对账"空名单即全员降权"是上游行为,之前本地重启已把管理员清零)。修 = 临时号提权装机(`POST /api/admin/plugins` 传 0.8.5 `package.zip`,registry 已是 0.8.5,entry 包验有 `slice(0, 50)`+batch 进度),临时号删;`local.env` 永久加回 `ADMIN_ACCOUNTS=<laniakea uuid>`(与 GOAL-STATE 记载惯例一致,防以后每次重启再降权),后果端重启确认无降权日志。用户侧只需硬刷重剪。注意:生产 stack 若同样没配 `ADMIN_ACCOUNTS`,其重启一样全员降权——待查。

## 节点名长度校验补齐(后端 400 + 前端 maxlength,2026-11-23)

- [x] **超长名 500 改 400 + 前端改名 50 上限(0.8.5 遗留,用户指令"修")**:后端 `NodeInput::validate_lengths`(name≤50/description≤250/tags≤250,按字符数)挂 POST/PUT 两入口,超限报 400 `Key: 'Node.Name' Error: ... 'max' tag`(对齐 Go `binding:"required,max=50"` 族报文,不再是 driver 500);上传文件名早已截 50,故仅此一处缺口,无 PATCH 旁路。前端 8 处节点名 input 加 `maxlength="50"`(编辑器标题/新建分类/元数据/分类编辑/资源改名/流程图/团队设置/admin 文档)+ 3 处 description `maxlength="250"` + admin tags 明文框 `maxlength="250"`;AppTagInput 两处未动(组件内逻辑,后端 400 兜底)。验证:`cargo test` 新单测(含 50/51 边界 + 多字节计数)绿 + clippy/rustfmt 干净 + eslint 8 文件过 + 活后端 POST-51/PUT-51/PUT-desc251 均为 HTTP 400 明文、50 字边界 200 建功;dev 后端已重启生效。测试号/节点已清。注意:库里有个非本次建的 `tmpv` 号,未动。

## 超长标题剪藏失败修复(web-clipper 0.8.5,2026-11-23)

- [x] **微信文章剪藏不下来(用户报,思源 readitlater 可剪)**:`mp.weixin.qq.com/s/2GrO4J49SaedhN3ekimt0Q`(Day 17 频域分离,标题 51 字)。排查链:直抓 200/3.8MB 有正文 → 后端 proxy 同样 200 全文 → 真 `parseWebsite` 跑出标题+3156 字 → 全链 harness(真 `clip()` + 真 proxy/upload/建笔记,linkedom 垫 DOMParser)复现 `nodes POST: value too long for type character varying(50)`。根因:插件笔记名截 `slice(0, 60)`,DB `nodes.name VARCHAR(50)`(Go/Rust 两 schema 一致,文件夹早已按 50 截,唯笔记名漏网),51–60 字标题必死在 INSERT(后端报 500 Internal error,用户侧只见剪藏失败)。修 = 一行 `slice(0, 50)` + ceiling 注释;同 URL 全链复测绿(3 图附件入库,标题截 50 字)。测试号/笔记/附件已清(孤儿字节归零)。遗留已消(见上条:后端 400 + 前端 maxlength)。

## 批量剪藏进度与提示修复(web-clipper 0.8.4,2026-11-23)

- [x] **批量无进度+提示错乱(用户报:批量剪藏正在进行,但没有进度条和成功提示)**:根因三件 — ①只有知乎走 `render` 才有进度事件,proxy 路径(`clip` 的 `onProgress` 传参从未被调用、`clipBatch` 签名里根本没有该参数)批量跑几分钟零反馈;②manifest 无 `batch_delimiter` 默认 → 对话框按 `auto` 拆、`clipBatch` 按 `newline` 重拆,逗号分隔单行批量拆出 0 URL(误报 clipboardEmpty + `0/2 success` 假成功);③对话框批量完成吐双 summary(文案还不一致)+ 一条冗余 error toast。修 = `clipBatch` 接 `onProgress`(逐 URL `正在剪藏 i/n：短链接` tick)+ `announceSummary` 开关(对话框/剪贴板命令各自复用自家进度 toast 收尾,剪贴板命令修顺手 shadow 掉 `text` 的旧错);两处分隔符默认统一 `auto`;summary 补失败段、删冗余 error toast;`trackProgress` 提公共 helper。headless harness(真 main.ts + mock ctx,oEmbed 路径零 DOM)三场景 14 断言全绿,运行包与源码 `cmp` 一致后才动手。`package.zip`→`plugin-packs` 同批同步(0.8.3 漏同步,本次补齐)。

## 批量剪藏同步(web-clipper 0.8.3,2026-11-23)

- [x] **对话框批量剪藏(用户报:pergame 完全不支持批量,与思源不同步)**:剪藏对话框提交路径现在按 `batch_delimiter` 设置切分字段(auto/newline/comma/semicolon),多 URL 自动走 clipBatch(与剪贴板批量命令同流程:逐 URL clip → 汇总 `Batch clip finished: ok/total(失败 N)`);单 URL 保持原单剪路径。对话框描述注明多 URL 批量。splitBatch 单测级验证(auto/newline/comma/semicolon 五例)+ 0.8.3 已装本地与生产。`17e004d9`。

## 微信公众号剪藏修复(web-clipper 0.8.2,2026-11-23)

- [x] **公众号文章剪藏失败根修(用户报:思源成功、pergame 失败)**:同一篇 `mp.weixin.qq.com` 链接,思源插件成功、pergame 失败。定案:微信对 **UA 缺失/陌生的请求 302 到验证码墙**(`wappoc_appmsgcaptcha`,title 未知错误)——curl 实证:无 UA/微信内嵌 UA=302,桌面 Chrome UA=200(3.85MB 完整文章)。pergame 的 proxy 直抓从不带 UA。修 = ①web-clipper website.ts 直抓带浏览器 UA(+accept/accept-language,后端 allowlist 本就放行 user-agent);②显式探测验证码页(wappoc_appmsgcaptcha)给出明确报错;③**WeChat DOM fix 三件套**(Obsidian/SiYuan issue #187 同款):正文容器去 visibility style、懒加载 img[data-src]→src、div>figure 提升;④后端 proxy_fetch 默认浏览器 UA(插件未带时兜底,全插件受益)。生产已装 0.8.2,proxy 带 UA 实测 200/3.8MB 无验证码。`a35e27bf`。

## 生产 v0.0.13 发布 + 库/桶统一(2026-11-23)

- [x] **生产 v0.0.13 上线**:release v0.0.13(target main)触发 build ✓ → Portainer API(stack 42)PUT 更新(镜像 v0.0.13 + 显式 S3_BUCKET)→ 双容器重建。验证:health ok、生产 JWT 打开迁移文档页 **10/10 微信外链真图**(no-referrer 生效)、containers/note API 200。issue #19 已关。
- [x] **库/桶统一(用户指令)**:旧生产库 `pergame` DROP(先 pg_dump 备份 186KB → `.work/pergame-backup-20261123.sql`,仓库内),`pergame_dev` RENAME → `pergame`;生产 backend 容器先停(断连接)。S3 桶对齐:旧 `pergame` 桶 103 对象清空(旧生产无主数据),`pergame-dev` 229 对象(595MB)server-side copy → `pergame` 桶(parity 校验 missing=0/sizediff=0;31 个 0 字节用户目录占位为 QuObjects 自动生成,无害)。**两环境配置统一**:生产 stack 显式 `S3_BUCKET: "pergame"`;本地 local.env `DATABASE_NAME=pergame`(原 pergame_dev)、`S3_BUCKET=pergame`。本地后端重启验证 174 containers/笔记 200。⚠️ 历史教训:换库/桶时 DATABASE_NAME/S3_BUCKET 两个 local.env 键都要改,只改一个会静默连旧库。

## Web clipper v0.5.0(2026-11-20 批次)

- [x] **剪藏文案 i18n 化(2026-11-22,v0.8.1)**:用户报「英文界面提示『剪藏中』」。三层修复:①宿主 `formatClipProgress`($i18n.t,**不能在 async boot plugin 用 useI18nT**——setup 上下文断言抛错把整个 dashboard 打成 500,必须走 `nuxtApp.$i18n`)按 UI 语言拼进度 label,经 `net.render` onProgress 传 `{percent, stage, label}` 给插件;②插件删硬编码 STAGE_ZH,进度用 `p.label`、完成/失败/批量文案按 `ctx.api.host.locale` 自带 zh-CN/zh-TW/en 字典;③i18n 键 `settings.plugins.clip.*` 八语言落位(perfectionist 强制字母序:clip 块在 capabilities 后、内部键 progress<stageC<E<Q<R)。宿主 PluginHostInfo 新增 `locale`。浏览器实测 done toast「Clipped: Example Domain」(en 界面);API 实测 render 阶段流转 10 connecting→60 extracted→done(401KB)。
- [x] **🔴 render engine `auto` panic 根修(2026-11-22)**:KV `render_engine: "auto"` 被原样传给 `match engine.as_str()` → 落进 external 分支的 `expect("resolved before spawn")` → **tokio 任务 panic,job 卡 percent=5,前端静默无 toast 无笔记**(日志 `plugins.rs:954 panicked`)。修 = 解析时展开 auto(url 配了→external,否则 builtin)+ external 分支 expect 改 unwrap_or_else 兜底。API 实测修复后 auto→builtin 全链成功。**教训:设置默认值("auto")必须在服务端展开,不能信任它恰好匹配 match 分支**。附带:clippy 2 个既存 deny 已清(用户指令)——`mcp/server.rs` 的 `ApiError::from(e)` 恒等转换改 `map_err(mcp_error)`、`imgbed.rs` 的 `path_segments().last()` 改 `next_back()`(Split 双端迭代器,顺带 rustfmt 该文件两处既存格式债);行为等价零功能差异,`cargo clippy --all-targets` 0 error。

- [x] **设置弹窗四项瑕疵根修(2026-11-22)**:根因 = `PluginSettingsModal` 的 `watch(open)` 只挂在「关→开」变化,而父组件 `settingsFor`+`settingsOpen` 同 tick 置位 → 组件挂载时 open 已 true,加载器**从未运行**(workspace 恒空、select 恒 '--'、模板恒空白四项瑕疵同源)。修:`{ immediate: true }`(挂载即加载)+ 空 stored('') 回落 manifest 默认 + 动态 choices 到位后 select 空值自动选首项。manifest 补知乎组(slug='Zhihu'/title/body 模板)并 v0.8.0;DEFAULT_SLUGS/TITLES 补 zhihu。浏览器实测:workspace 下拉 6 项默认选首项(Archive)、冲突策略=Keep both、分隔符=Auto、引擎=Auto、知乎/YouTube 模板默认值全非空。附带:**SSR 500 修复**——`SidebarWorkspaces` 的 `watch(immediate)` 在服务端跑 `localForage.getItem` → 直连任意 `/dashboard/*` 路由 500(上游 #551 存量),加 `import.meta.client` 守卫。dev 插件已重装 v0.8.0(旧 bundle 的 `template_defaults` KV 键清除,grants 五项全授)。
- [x] **插件设置界面** ✅:manifest 可声明 `settings`(group 折叠分组),设置页齿轮按钮打开表单,值存插件 KV(`ctx.settings` 同源)。43 项设置对齐 SiYuan ReadItLater 构造:General 7(workspace 下拉/路径模板/日期格式/重名策略/批量分隔符/知乎 cookie)+ Browser rendering 4 + 11 内容类型 × 3(slug/标题/正文模板)。
- [x] **每类型模板生效 + 自动建 folder** ✅:`<type>_slug` 作文件夹名与标签,`inbox_dir` 路径模板渲染后逐级 ensure folder;`render` 服务经后端 MCP 客户端(camoufox)出真实浏览器页,30s create 上限走空开页签+navigate 流程,3 轮指纹重试,signin 落地检测。
- [x] **camoufox 通道** ✅(D' 方案):后端 `POST /api/plugins/:id/render`,URL/token 从插件 KV 读(用户自建服务=信任决策,免 SSRF 校验);超时用户可配(`camoufox_wait_seconds`,驱动 wait_for_selector + 后端 HTTP + 外层预算)。
- [x] **轻量容器接口** ✅:`GET /api/nodes/containers`(1.2s vs 全量 14s),插件 `listContainers` + workspace 下拉。
- [x] **知乎风控定性(2026-11-21,内容分级门)**:实测矩阵——列表页/官方帮助文匿名稳定通过;UGC 专栏/回答按篇掷骰(部分放行部分 signin/unhuman);质询 JS 全过(`__zse_ck` 能算出);obscura v0.2.7 stealth 引擎指纹被识别(内容层 403);本机 camoufox-cli 与 NAS camofox-mcp 同结论。signin/unhuman 页可离开但拿不到被拦内容。**匿名通路活着但按内容分级**;登录态(cookie 桥接)使被拦篇目确定性通过。**注**:思源「剪藏成功」传闻经查证无落库记录(11-20 后新建文档 0 篇),此前成功结论不成立。
- [x] **生产 stack 42 恢复 + v0.0.12 上线** ✅(2026-11-21):用户在 Portainer UI 手动连网络触发重部署 → 旧 stack file(v0.0.10 镜像)对 migration 12 库校验失败循环崩溃(11-21 凌晨起生产 502)。修复:发 v0.0.12(含 0001-0012 + 本批 render/containers)→ stack file 更新镜像 tag + backend 挂 `camofox` external 网络(用户网络操作的持久化)。核验:health 200、三网络就位(camofox/pergame/postgres)、migration 干净过。**教训**:Portainer UI 对运行容器的网络改动会在下次 stack 部署时被 compose 覆盖——网络声明必须进 stack file。
- [x] **进度体验终修(2026-11-22)**:①异步 job + 阶段式百分比(10/30/60/100 对应真实事件;线性百分比是假的)——POST /render 立即返 job_id + GET 轮询,AppState.render_jobs 取走即删;②宿主 net.render 轮询透传 onProgress,**onProgress 缺省时宿主自带进度通知**(id=render-<jobId>,兼容旧插件 bundle);③**根因级修复:SW api-cache 路由(3s networkTimeout 回缓存)劫持 /api/plugins/**(entry 旧 bundle + 轮询冻结)——已从该路由排除 /api/plugins/。⚠️ 生产 PWA 注册 SW,此修复须随下一版镜像上生产才对生产剪藏生效。单条 toast 全程原地更新(id 化 notice:update 语义,notifications.update 新增)。用户端首次真剪成功(v0.6.x,静默 50s);v0.7.1 + SW/宿主修复后待用户复验。
- [x] **弹窗优化(2026-11-21)**:macOS 无 Xvfb,Xvfb 等价物不存在(BetterDisplay 付费/Quartz 独占)。改为拉长 daemon idle timeout:cli 的 `--timeout` 被 open 子命令同名参数覆盖(实测),统一 86400(导航卡死由外层 Rust timeout 兜底)→ 弹窗从每 30 分钟一次降为每天最多一次(daemon 常驻复用浏览器)。彻底消除弹窗的备选 = OrbStack 本地起 camofox 双容器走 external_mcp(用户自选,未做)。
- [x] **✅ 门已破(2026-11-21 定案)**:知乎墙检测的是 **headless/自动化痕迹**,非引擎伪装、非 cookie——变量分离实验:camoufox headless ❌、真 Chromium headless ❌、真 Chromium headed+去自动化 ✅、camoufox --headed ✅、camofox-mcp virtual 显示 ✅(334KB 正文)。**登录 modal 不用点**——DOM 提取不挡,正文一直在 modal 下面。双引擎已改:builtin spawn 加 `--headed`(dev 弹窗一次);external 渲染前调 `toggle_display(virtual)`(持久,NAS 已生效)。回归:同篇被门文 builtin 334KB/50s ✅ external 334KB/32s ✅。原「zhihu_cookie 确定性通过」降级为兜底(硬拦残留场景)。
- [x] **双渲染引擎(2026-11-21 用户决策:dev=本地 camoufox-cli,prod=camofox-mcp 容器)**:`render_engine` 设置(builtin|external_mcp|auto;auto = camoufox_url 有值走 external,否则 builtin)。builtin = 后端 spawn `camoufox-cli --session pergame --persistent` 子进程(open → settle → eval 探针),门内文 explore 预热一轮,cookie 桥接走 cli 原生 `cookies import`。活测:官方文 200/397KB(51s),被门文 400 + cookie 指引(139s)。工程坑见 GOAL-STATE(cli 走 stderr 报错、probe 双括号)。生产 v0.0.12 含 external 引擎;builtin 供 dev(本机 cli+浏览器已装)。

## 思源 → Pergame 全量搬迁(2026-11-17 用户指令)

- [x] **全量搬迁到 pergame_dev(本地 dev 库)** ✅ 2026-11-17:872/872 文档(168 纯父目录 → category folder,704 → 笔记)+ 5 笔记本 → 5 workspace + 210 思源 `assets/` 随迁为 resource(外部 http 直链保留原样)。引擎 = /tmp/mig.py(JWT 自签 + REST + siyuan MCP 直连;断点映射 /tmp/mig-map.json)。**验证**:树结构三方对齐(role/parent/name 全 0 错,workspace 5/5 OK)+ 抽样内容比对(KDL 药品文档字节级 MATCH;图库笔记差异仅为预期资源 URL 改写)+ :3000 health OK。资源 URL 与前端原生一致(`/api/resources/:id/file`)。
  - 残留 1 处:`Obsidian 同步运维` 笔记里的 `` `[xxx](assets/y.pdf)` `` 是反引号代码示例(非真链接),不迁。
  - 生产库(stack 42 共享库)仍留 87 个旧测试节点,等用户拍板是否清。
- [ ] 网关遗留:NocoDB MCP 端点挂死(服务活着但 /mcp/* 不回包)→ Portainer 重启 NocoDB 后把 mcp.json 两个 disabled 改回;obsidian(:9100)/mailspring 同理;网关 siyuan 挂载 tools/call 挂起(fastmcp 2.14.7 proxy 怪癖)待查。

## 批次 A — 前端批次(纯前端,独立于重写;**待开工令**)

- [x] **媒体播放器内置化**(video-md → 内置,2026-11-12 完成):issue #6;`html5MediaPlugin` 补 `preload`/`#t=`/`ogv` + PDF 占位符,`ContentCompiled` 后挂载 `PDFViewer`(mermaid 同款);附带修复 Rust `sanitize_html` 对齐 Go bluemonday 策略(audio/video/object/data-*/checkbox/form/SVG/MathML)。
- [x] **Joplin md 特性集对齐** ✅(2026-11-13):issue #7;breaks(默认开)/typographer(默认关)双开关+compile 按设置缓存实例;ins/multimd-table/deflist/abbr 四插件(multimd 垫 `md.utils.assign`);embeds 白名单(YouTube-nocookie/Bilibili/Vimeo)+consent 壳+懒加载;ABC(abcjs 懒加载+abcjsOptions 设置)。浏览器实测 9/9 渲染。Fountain 后置(待需求确认)。
- [x] **M0 ob 语法兼容(缩水版 ✅2026-11-13,用户裁决:冲突/重叠不做,上游优先)**:wikilinks ❌不做(`[[...]]` 与 tooltip 插件冲突 + 上游 `#id` 双链已覆盖)、callout ❌不做(与 `:::` 容器重叠);✅做:frontmatter 解析(YAML 子集 scalar/list)+properties 表渲染+compile 入口 strip、导入改写(相对附件引用→占位→上传后换 API URL,资源先传顺序)。浏览器实测 properties 3 键 + 路径解析 7 例全过。

## 批次 B — 重写批次(Rust + pg + 功能域;**等用户开工令**)

- [x] **Rust 后端重写 + PostgreSQL** ✅(2026-11-08 开工,P1 骨架+Toasty spike PASS → 2026-11-12 已上 QNAP 生产 v0.0.11):issue #1;db 层 = Toasty + sqlx 逃逸层(11 条 `WITH RECURSIVE` 留 sqlx);全新空库;pg GIN + tsvector。
  - [x] 集成测试运行时验证:14 用例全绿(需活后端 + pg + S3,`cargo test --test integration -- --ignored --test-threads=1`),多次活环境跑通;生产 health/SSR/备份全链已核验。
- [x] **Rust 代码质量改进项** ✅(2026-11-13,审计 6 项全落地 + 集成测试修复 + 3 真 bug):①`#![deny(unsafe_code)]`(forbid 与 test scoped allow 互斥,deny 等效;生产 unsafe 0,8 处全在 `#[cfg(test)]`);②删 `croner`/`aws-config`;③拆分 6 文件全 <600(restore→apply/plan/portable/members、backup→mod/store/run/schedule、resources→upload/download/orphans、nodes→mod/trash、repo→mod/tests、routes/backup→mod/tests);④rustdoc 全覆盖 + 6 doc-tests(抽 `src/lib.rs` 纯 binary 才跑得起);⑤llvm-cov 行 13.8%→24.3%(纯逻辑多 85%+,余量需活 pg/S3)+ proptest 3 组(cron/sanitize/cookie);⑥PGO/SoA/SIMD 判 N/A。**真 bug**:环检查 `(Uuid,)`↔bigint 错配恒 500(UUIDv7 批量改残留)、还原子先父后撞外键(改两轮插入)、2FA `qr_code_url` 返 Display 摘要非 otpauth URL(改 `to_url()`)。**集成**:14/14 全绿(修 3 过时测试:target 走 probe-draft token、备份前建 target、还原传 force、sync 改快照语义)。单测 44→117。
- [x] **Per-user 配额 + 内容寻址去重** ✅(2026-11-13):①users.max_file/total_size(NULL=回落 config;config 0=无限制;admin 豁免;PATCH 双 Option 清 null;`GET /users/@me/quota`;CDN 条读服务端+无限制态;用户管理页配额输入);②空/缺省=无限制口径(TRASH_RETENTION unset/空=不清理,0=全清;SNAPSHOT_INTERVAL unset/空=关,0=非法 warn 关;全局审计余量无限制语义);③去重(`object_blobs`+`blobs/{sha256}`+refcount;upload/update/restore acquire;purge/删账户递减;末引用删对象;备份读 blob 优先;orphan 扫失引 blob)。**生产迁移实测**:30 对象全迁 `bytes_saved=0`(无重复)。单测 117→119,集成 14→16,覆盖率行 22.9%。
- [x] **MCP server** ✅(2026-11-12,issue #3 保持开启待用户确认):`rmcp 3.9` + `/api/mcp` streamable HTTP + bearer token middleware + 7 工具 + `api_tokens` 表(migration 0007)+ 设置页 `?p=apiTokens` + docs `use/mcp`。**已实测**:隔离库全链(token 生命周期/7 工具/越权 -32003/环 -32602/回收站/吊销 401)+ 只读窗口(四写 -32023、三读放行、REST 同窗 423)+ 生产库真数据(list/read/search/create/update/delete 全通)。**未做**:速率限制(P1 裁决不做);docker 镜像未含(下次 release 才上 QNAP)。
- [x] **目录索引文件 → 移除,内联为容器概览** ✅(2026-11-13 实施 index 文件;**2026-11-18 移除**:v2 表格直接内联到 workspace/folder 容器视图,`ViewSelection` 新 `overview` 模式为默认(表格/网格/高级/看板保留);后端 ensure/锁/设置批量改名/`auto_index` 全删,migration `0013` drop `idx_nodes_index_child` + 存量 index 节点软删入回收站;DQL `DIRECT`/`id:`/`viewed_timestamp`/`node_views`/`record_view` 保留(dataview 引擎通用);前端 `children`/`recent` 块仍可在普通文档用,`index` 块语法移除。issue #4。spec = `features/directory-index.md`(已标注移除)。原实现摘要:`metadata.index=true` 身份标记(服务端签发,客户端不可伪造)+ 建夹自动创建(响应 `{node,index}`)+ GET 懒重建 + 唯一索引 `0010` + 锁定(改名/移动/删除/pin 全拒,内容可编辑;MCP 三写工具同锁)+ 设置批量改名(`indexFileName`,`index_renamed` 计数,含回收站行)+ `displayIndexFiles` 可见性开关 + index 块子树渲染(过滤 index/目录优先/图标)+ 目录行跳转懒建 + 全站排序 index 置顶。单测 128 + 集成新用例 `directory_index_lifecycle`。spec = `features/directory-index.md`。
- [x] **Dataview 查询引擎** ✅(2026-11-13):issue #5;DQL 子集(LIST/TABLE/FROM folder+tag/WHERE 比较+contains/startswith+AND/OR/NOT/SORT/LIMIT 硬顶100/相对时间-7d)+手写递归下降+参数化 planner(字段白名单/深度32)+`POST /api/nodes/query`(仅本人节点/5s 超时)+前端 ` ```dataview `/` ```index ` 块(LIST/TABLE/index 预设 `LIST FROM 当前目录`)+可点跳转。**实测**:单测 6/6+集成 17/17+浏览器 3 块渲染+跳转。spec = `features/dataview.md`。
- [x] **资源退出文档树(大修)✅(2026-11-18 晚,issue #14,commit `910e48c8` 已推)**:资源(role 4)一律不进文档树,CDN 页为唯一资源面;挂载规则不动(资源挂文档下仍为合法数据形态,后端不加校验、不迁存量)。改动:`useSidebar` 用 `filterTree` 递归剪 role 0/4(浅过滤根因:偏好只滤顶层数组,文件夹内资源恒显示);**删 `displayUncategorizedResources` 偏好**(constants 默认值+持久化白名单行、apparence 开关、i18n ×8 两 key);`childrenSource` 加 `WHERE role <= 3`(概览目录表+`children` 块排除附件;DQL 解析序 FROM→DIRECT→WHERE 已支持,recent 本就 role=3);home 快速搜索跳 role 4;`SidebarItem` 文件拖到文档行改挂其所在 folder(原挂文档=树外不可见)。活测:侧栏 883 行 0 资源行(原 1093 含 210 资产);01-History 目录表 14 行=其 14 文档(26 附件剪净);快搜附件名不中/文档名中;设置开关八语删净;合成 drop 落 `00-Apple`(API 验后清理);CDN 页资源表无回归;typecheck/lint 绿。
- [x] **DataTable 每页条数持久化 ✅(2026-11-18 晚,commit `5da6fd9c` 已推)**:CDN/文档等 DataTable 的「Rows per page」下拉只调 `paginator.setMaxPerPage`(内存),重挂载回到默认 10;补 `preferencesStore.set('datatableItemsCount', n)`(既有偏好,设置页「其他」暴露,GENERAL_KEYS 已同步 user_settings)。活测:设 250 → 预览 → 返回 → 全量刷新,均保持 250/211 行;eslint/stylelint 绿。
- [x] **代码预览:行号 + 可选配色** ✅(2026-11-18 晚):CDN 预览补复制按钮(同 md 代码块 `.code-copy-btn`:复制原文不含行号,1.5s 绿勾反馈);语法配色由硬编码改 CSS 变量 + 4 套可选主题(`palenight` 默认 / `dracula` / `github-light` / `solarized-light`),偏好 `codeTheme` 存 `user_settings.general`,设置页「外观」下拉,`App.vue` 写 `<html data-code-theme>`;修根因:预览 `<code>` 原继承 `--text-body`(浅色 #111)落在深色底,改为吃 `--code-fg`;注释色提亮 `#676e95 → #8a93b5`。验证:eslint/stylelint/typecheck 绿;cmux 内置浏览器活测(行号 1–30 对齐、Palenight/GitHub Light 双主题截图、设置页下拉正常);camoufox MCP 拒私网/localhost,不可用。
- [x] **Admin 用户列表 + 孤儿预览** ✅(2026-11-18 晚):`GET /api/users` 在 Rust 后端缺失(仅 POST)→ 管理页 `No users found`;补 admin-only `list_users`(`is_app_admin` 门,role 1 实测 403)。孤儿预览 400 根因 = `object_blobs` 残留行对应 S3 对象已不存在(`head_object` 404);`preview_orphan` 对缺失 blob 改回 404 + 文案,前端 modal 显示「文件内容已不在存储中,可直接删除」,删除路径不变。验证:API 200/403/404 + cmux 管理页 1 行 Admin + 孤儿 modal 文案。
- [x] **通用图床适配器 + 转存(Monolith #185/#186 对位)✅(2026-11-19 凌晨)**:`[Imgbed]`/`IMGBED_*` 配置(上传端点/鉴权/响应路径/URL 模板,ImgBed/Lsky/Chevereto 通用);新上传图片/视频直落图床,私有笔记需每用户 `imgbedPrivate` 勾选(公开父节点天然走图床),失败回落 S3;读走后端代理(`/api/resources/:id/file` 不变,鉴权保留,Range 透传)。CDN 页图床面板:私有勾选 + 图片/视频勾选 + 转存本地(复用 sha 去重,S3 留备份)+ 转存外链(md/img/video 解析、SSRF 公网门、改写正文+compiled)。验证:单测 4 项(imgbed 解析/迁移类型过滤/URL 提取)+ Mock 图床全链(新上传落图床、代理读 200、单文件转存、httpbin 外链导入改写;测试数据已删);cmux 验证图床面板渲染;后端清测后重启无图床配置。
- [x] **容器概览加载态** ✅(2026-11-18 晚):`OverviewTables` 两张表查询未返回时显示 `common.status.loading`(原直接 `No results`);`Promise.allSettled` 结束才切空态。
- [x] **代码文件资源 + 文档图标统一** ✅(2026-11-18):代码/文本文件按**扩展名**放行上传(`utils::CODE_EXTS` ×46,浏览器 mime 不可靠;`upload_file`/`update_file` mime 门加扩展名回退);md 引用走带名路由 `/api/resources/{id}/file/{name}`(既有未用路由启用,粘贴/拖拽/复制 embed 同批切换,`resourceEmbedUrl`);`html5MediaPlugin` 新 `CODE_REGEX` → `.code-embed` 占位符 → `code-embed.ts` hydrator(≤256 KiB + UTF-8 校验,hljs 高亮 + copyCodePlugin 同构 wrapper,复制/折叠按钮直接复用事件委托;失败回退链接);CDN preview 页代码分支;MCP 新工具 `read_resource`(第 8 个:role-4 门 + 文本判定 + 2 MiB 双检 + UTF-8 拒二进制,返回 name/filename/language/content)。文档图标统一:`resolveIcon` role 3 恒 `sidebar/file`(删 accessibility 三分支),`SidebarItem` 删粉色 fill + `file_parent` 换图标。**存量探测**:pergame_dev 有 3 个迁移来的 .py 资源但无 md 引用 → 不做旧引用兼容。活测:.py 放行/二进制 400/bare+named 200/MCP 8 工具 read_resource 全链;151 单测 + lint/typecheck 绿。思源对位:fork 只资产化不渲染,本批补渲染 + MCP 读。**补修(2026-11-18 晚)**:图标"仍混"根因非数据(DB icon 全空,resolveIcon 已统一)而是布局——`SidebarItem` `.content` 缺 `overflow:hidden`,nowrap+`min-width:auto` 使长标题撑爆 flex 行,把可收缩的 svg 图标挤成 0 宽(视觉=无图标);补 `overflow:hidden`+`min-width:0`+icon `flex-shrink:0`,活测 1093 图标 0 挤压、长标题正常省略号。**图标二轮统一(2026-11-18 晚)**:概览表/`children` LIST 块的 📁/📎 emoji(macOS 渲成灰 3D 文件夹,与侧栏蓝 sprite 不一致)→ 换 `resolveIcon` 同款 sprite 图标(`iconSvg` 内联 svg 注入块渲染器,OverviewTables 用 `<Icon>` 组件);连带修 `resolveIcon` role 2 的 `'shared' in item` 守卫——DQL 行无 `shared` 字段被放跑成 attachment fallback,放宽为 undefined=普通 folder(eslint/stylelint/typecheck 绿;活测 03-Development:6 行全 `#icon-folder` 与侧栏同源,emoji 清零)。
- [x] **Index 模板 v2 + DQL DIRECT/最近查看 ✅(2026-11-16,思源父目录内容对位)**:自动模板改两段中文表格预设 ` ```children `(直接子级:名称/创建时间/修改时间)+ ` ```recent `(递归最近查看:名称/修改时间/最近查看,未看沉底);DQL 新增 `FROM "id:<uuid>"`(稳定引用)、`DIRECT`(收敛直接子级,无源报错)、`viewed_timestamp` 字段(SORT 恒 NULLS LAST,TABLE 可投影行内回填)+`migration 0012_node_views`(GET 读时 upsert,MCP 不记);前端预设按所在位置解析(文件夹与其 index 文件/父文档均可用,重命名不失效)+中文表头映射。验证:后端单测 152 全绿(含新增 6)+clippy/eslint/typecheck+fmt(基线漂移除外)+真浏览器双表渲染(直接子级无自己人/浏览倒序/未看沉底)。spec = `features/directory-index.md`(`模板 v2`)+`features/dataview.md`。**2026-11-18 后续**:表头/标题「最近查看」改名「最近浏览」,recent 表与块只显示有浏览记录的行(无记录 = 空表,导入迁移的文档无历史不占位)。
- [ ] **自建插件系统**:issue #8(已批准);PS-A..E ≈ 3.5-4.5 周;spec = `plugin-system/DESIGN.md`(§9 已定案)。
  - [x] **PS-A 内核 ✅(2026-11-13)**:`app/types/plugin.ts` 契约 + `services/plugins/{manifest,registry,context,loader}.ts`(manifest 全校验:slug/semver/apiVersion 主版本/entry 路径穿越/capabilities 白名单;DisposableBag LIFO;host 载入-隔离-卸载;ctx L1 = nodes/resources/search/settings(KV)/ui.notice/net 门禁)+ `app/plugins/plugin-host.client.ts` 由 `NUXT_PUBLIC_DEV_PLUGINS` 驱动 + `public/plugins/demo` 参考插件 + docs `5.administer/3.plugins.md`。验证:25 项内核断言全过(含故意坏插件隔离、重复 id、错误主版本、`..` 穿越)+ 真实 HTTP 分发 + ESM 求值 e2e(demo 插件经 dev server 载入/unload)。
  - [x] **PS-B 第一片 ✅(2026-11-13,真浏览器验证)**:`ctx.ui.command`/`ctx.ui.ribbon` + `services/plugins/ui.ts`(反应式注册表、`<pluginId>:` 命名空间、重复 id 抛错、unload 自动回收)+ 命令面板 `Plugins` 分组(Enter/点击执行)+ Dock 渲染插件按钮;demo 插件示范两者。断言 12 项全过;真实浏览器确认:命令面板出现 `Plugins` 分组并执行(notice 回显)、Dock 出现插件按钮。
  - [x] **PS-D 分发 ✅(2026-11-14)**:`migration 0011`(plugins/plugin_grants/plugin_kv)+ 管理员 zip 安装(`POST /api/admin/plugins`,服务端 manifest 全校验、路径穿越/大小/条目上限)+ 产物存 S3 `plugins/<id>/<version>/` 并同源服务 entry/asset + 每用户能力同意(`PUT /plugins/:id/grants`,只接受 manifest 声明的)+ 每插件 KV(`/plugins/:id/kv`,持久化到 pg)+ 审计入 `logs` + `GET /api/plugins` 供 loader + 卸载删产物/授权/KV。**另加 `net.proxy`**(后端代取破 CORS,域名白名单 + 仅 GET/POST + 8 MiB + SSRF 守卫拒绝私网/IP 字面量),替代剪藏对 camoufox 的依赖(DESIGN §11)。验证:集成用例 `plugin_registry_lifecycle` 活环境通过 + API 全链(安装/列表/entry/grant/kv/proxy/SSRF 拒绝/卸载)+ 真浏览器(loader 从注册表加载已装插件、命令面板执行、设置页列出能力)。
  - [x] **PS-B 收尾 ✅(2026-11-14)**:设置页 Plugins(管理员安装/启停/卸载、能力勾选、非管理员只看可加载项)。
  - [x] **PS-C md/编辑器 ✅(2026-11-14)**:`ctx.markdown.registerCodeBlock/registerContainer/registerPostProcessor` + `ctx.editor.registerCmExtension`;注册表 globalThis 单例(避免 bundler 双实例),编译器按 hook 版本重建缓存;容器规则 raw-capture,未认领的 `:::name` 仍归内建容器;fence 回落到宿主渲染器(高亮/复制按钮不受影响)。验证:15 项 bun 断言 + 真浏览器(figure/aside/post-processor 三件套在真实文档渲染)。
  - [x] **PS-E 首批官方插件 ✅(2026-11-14)**:`plugins/official/{recent-files,daily-note}` + **`web-clipper`(ReadItLater 功能对位;命名按功能而非竞品,2026-11-14 用户裁决)** + `scripts/build-plugin-packs.sh`(有 src/ 的插件先 bun build)打包到 `frontend/public/plugin-packs/*.zip`。
  - [x] **ReadItLater 功能对位 ✅(2026-11-14,用户裁决:插件名为 `web-clipper`,功能即 ReadItLater,不再单列移植)**:文章(Readability+Turndown,相对链接绝对化,图片经 `proxyBinary` 存附件)+ 知乎(用户 cookie,失败提示粘贴)+ YouTube/Bilibili/Vimeo/TikTok(oEmbed)+ Stack Exchange/Bluesky/Mastodon/GitHub/Wikipedia(公开 API)+ 纯文本兜底 + 剪贴板/批量/对话框(URL/标题/标签/目录)+ 每类型模板(`{{var|filter}}` 与 `{% for %}`)。宿主新增 `ui.modal`(声明式表单)与 `net.proxyBinary`;代理支持安全 header 白名单(Cookie/UA 等)、默认 UA、`HTTPS_PROXY` 出站。**未移植**:图床上传、YouTube Data API、Twitter/X 与 Pinterest(需认证 API)、插入光标(无编辑器 API)。
  - [ ] 插件系统剩余:固定槽位视图、`ui.modal`、插件设置页声明式 schema、drawio 迁移、ReadItLater 站点 parser 移植。
  - **PS-E 剪藏不走 camoufox MCP**(2026-11-14 用户裁决,详见 `plugin-system/DESIGN.md` §11):思源 ReadItLater 依赖 camoufox 是插件沙箱限制;Pergame 用 `net.fetch`(页面直取)+ 新增 `net.proxy`(后端代取,破 CORS)+ 用户提供 cookie/粘贴兜底。不读其它标签页 DOM(Web 无此权限)。

- [x] **笔记软连接/克隆**(Trilium 对位)✅(2026-11-09,NL-A/B/C 三阶段):克隆表 node_links + 环校验 + 端点 + 树 CTE 扩展 + 前端;规格 = `features/note-links.md`。


## 回收站(用户裁决 2026-11-09:本地误删恢复走回收站,不做 git 级版本)
- [x] **回收站机制** ✅(2026-11-09):软删 + 保留期 + 恢复/彻底删/清空 + 树查询过滤 + 前端视图。spec = `features/trash.md`。
  - 上游 Alexandrie 无回收站(物理 `DELETE FROM nodes` + 级联 + 删 S3 对象),本机实测确认。

## 待人工验证

> **2026-11-10 成熟度评估**:可开始人工测试,定位为「开发验证测试」而非验收测试。(2026-11-13 校数:128 单测 + doc-tests + 18 集成全绿,~21k 行 Rust / 80 文件,10 migration)
> 后端 80 路由 / 218 Vue;
> 认证/2FA/OIDC/节点/软链接/回收站/快照/还原/fan-out/只读锁均已对真实 pg + 对象存储活测。
> 部署步骤 = 思源 `Pergame 外部基础设施部署指南`(id 20261111085400-74zeey6);旧版指南已删。

**拦验收不拦测试的缺口:**

1. **自包含版 compose 仍未跑过**——2026-11-12:external 版随 v0.0.4→v0.0.11 在 QNAP Portainer stack 42 常驻(backend/frontend 双容器,镜像钉 tag),health/静态/SSR/备份全链/WebDAV/CDN 鉴权读均已活测。**仍未知**:自包含版(`docker-compose.yml`,pg + RustFS 一起起)的卷挂载与 healthcheck 依赖链。
2. ~~**dr-backup 真实备份从未跑成**~~ —— **已失效(2026-11-12 快照=全备份,边车删除 `06a30906`)**:实例 DR 改由用户级快照直写 targets 覆盖,本条作废。
3. **前端交互走查 ✅ 2026-11-13(Playwright 真浏览器,本机最新代码 + NAS 基础设施)**——建文档/保存/树内可见、CDN 页上传附件 + 鉴权读 200(匿名 401)、回收站列表 + Restore、零 target 快照一次性 token 链接 + zip 结构(`meta.json`/`manifest.json`/`blobs/`)+ token 单次消费、**快照目标页全链**(保存前 Test 门禁 → Test connection → Save target → 行内 Test → Edit/Cancel → Sync now(`copied 6 skipped 1 pruned 0`)→ 有对象时 Delete 被拒 → **Empty 按钮** → Delete 成功)、**快照列表 + 二次确认还原**(首击 arm 出 plan 统计 `Incoming 2 · Current 2 · Replaced 2 · Added 0 · Removed 0` + 覆盖文件提示 + Profile 提示;勾 force 后二击执行,服务端自动生成 Pre-restore safety snapshot)。**同轮修**:目标行补「Empty」按钮(此前 empty 只有 API,UI 无法自解 delete-check)。测试账号/目录/target/对象已全清。
4. **frontend flags 的 `.env`→compose 断链**(2026-11-11 发现):`CONFIG_HIDE_NATIVE_LOGIN`/`CONFIG_OIDC_PROVIDER_AUTO_REDIRECT` 在 `.env.example` 与 docs 有名,但自包含 compose 未映射成 `NUXT_PUBLIC_CONFIG_HIDE_NATIVE_LOGIN`/`NUXT_PUBLIC_CONFIG_OIDC_PROVIDER_AUTO_REDIRECT` → 设了不生效;OIDC-only 部署会踩。修法 = compose frontend 块补两行映射。
5. ~~**目录索引的点击链**~~ —— **已验(2026-11-13,Playwright + Chromium 真浏览器)**:侧边栏 index 紧跟目录、index 块过滤容器与 index 自身且目录前置、点目录行跳该目录 index、目录页「Open index file」跳转、`displayIndexFiles` 关闭后侧边栏无 index(目录仍在)。另:同轮修掉该批次遗留的 SFC 模板缺闭合标签(设置页整个不可用),并新增 `bun run lint:sfc` 闸门。

6. **线上部署落后 main 30 个提交(2026-11-13 实测)**:QNAP stack 42 仍是 `v0.0.11` 镜像(11-12 16:12 打的 tag),而 `281bf11e`(附件改走后端鉴权读,11-12 17:08)以及其后的配额/去重、Rust 质量、MCP、#4 index、#5 dataview、PS-A、PS-B 全在 tag 之外(`git tag --contains 281bf11e` 为空)。实测证据:同一测试附件在本机最新后端 `GET /api/resources/:id/file` → **200 / 39 B**,线上 `:8200` 同路径 → **404(空 body = 路由不存在)**。即线上附件/CDN 读是坏的,修法 = 用当前 main 打 **v0.0.12** 并 `PUT /api/stacks/42` 更新(NAS 恢复在线后执行)。

**明确未实现(按批次,非 bug)**:批次 A ✅全清(媒体播放器 ✅2026-11-12、Joplin md ✅2026-11-13、M0 缩水 ✅2026-11-13)、Dataview ✅2026-11-13、目录索引 ✅2026-11-13;剩余:插件系统(#8)。(Pergamena 改名 2026-11-14 用户裁决撤销:当前名称已合适。)

### 建议测试顺序

- [x] **1. compose 起全栈**:external 版(2 服务)✅ 2026-11-12 于 QNAP Portainer stack 42(镜像钉 tag,随 v0.0.4→v0.0.11 逐版跟进),`/api/health` → `{"database":true,"status":"ok"}`。自包含版(4 服务,pg+RustFS+backend+frontend)仍待测。
- [x] **2. 核心 CRUD** ✅ 2026-11-13(Playwright,本机最新代码 + NAS 基础设施):注册 201 → 登录进 `/dashboard/home` → 新建文档(标题 + CodeMirror 正文)保存跳 `/dashboard/docs/edit/<id>` → 树内可见 → 正文渲染 → CDN 页上传附件(`AppDrop` file input + Upload)→ `GET /api/resources/:id/file` 带 cookie **200 / 39 B**,匿名 401。
- [x] **3. 回收站** ✅ 2026-11-13(Playwright):`DELETE /api/nodes/:id` → `/dashboard/trash` 列出(名称/大小/删除时间)→ 点 Restore → 节点恢复可读、trash 列表 0 行。
- [x] **4. 快照全链** ✅ 2026-11-13:零 target → 一次性 token 链接 + zip(`meta.json`/`manifest.json`/`blobs/<sha256>`,token 二次访问 404);有 target → 快照进列表(`format v2`、size、memo)→ 首击 arm 显示 plan 统计 + 覆盖文件/Profile 提示 → 勾 force 二击执行 → 服务端自动建 Pre-restore safety snapshot。**未验**:只读提示条(`MaintenanceBanner`,还原太快抓不到窗口)、上传 zip 回灌。
- [x] **5. 异地目标** ✅ 2026-11-13(Playwright):draft probe `Reachable · 0 snapshot object(s) · <fingerprint>` → Save(保存前 Test 门禁生效)→ 行内 Test → Sync now(`copied 6 skipped 1 pruned 0`)→ 有对象时 Delete 被拒(delete-check 409)→ **Empty**(`deleted_objects:10`)→ Delete 成功。
- [x] **6. ~~实例 DR(dr-backup)~~ 已随边车删除作废** → 改为:零 target 快照(S3 inline zip + 有效期 + token 链接下载)✅ 2026-11-13 验证通过。
- [ ] **7. 进阶**:2FA(enroll/verify/recovery)、OIDC(Keycloak 环境已有)、SMTP 邮件。

### 旧条目

- [ ] 自托管 smoke(本机无 docker):`docker compose up --build` 后端正常启动(`S3_*` 链路)、注册/登录可用。改名后默认值已切 `pergame`(DB/桶/CDN 路径),首次起库即新名。
- [ ] GitHub repo secrets:`MINIO_ACCESSKEY`/`MINIO_SECRETKEY` 手动改名 `S3_ACCESSKEY`/`S3_SECRETKEY`(deploy_to_prod.yml 引用已改)。
- [x] ~~`scripts/dr-backup.sh` 实机验证~~ —— **作废**(2026-11-12 边车删除,脚本/镜像/文档同批移除;实例 DR 由快照直写替代)。
- [ ] push 51+ commits 到 origin(kamasylvia/Pergame)——待拍板(公开仓 push 前可按全局规则压缩重组)。(2026-11-09:首 15 个已压缩为 5 主题 commits 并推上 origin/main `6e0abd8a`;此后增量照常。)

## 运维约束(2026-11-12)

- **migration `0007`–`0010` 已在生产库 `pergame` 落库 → 文件冻结**:改动会让下次部署 checksum 校验失败。要调整表结构,只能追加 `0011_*`。
- 生产库当前 4 用户**均非 admin**,`/api/admin/backup/*` 对所有人 403;恢复需 `ADMIN_ACCOUNTS=<uuid>` 或改库。
- 生产回收站存有更早的测试遗留(用户在 2026-11-12 明确:**不用清,以后统一清库**)。

## 维护说明

- 项目约定与决策记录 → 根 `AGENTS.md`。
- 批次/续接点 → `pergame-infra/GOAL-STATE.md`;教训 → `LESSONS.md`;spec → `features/`、`plugin-system/`、`rust-migration/`、`obsidian-compat/`。
- issue 索引:#1 pg · #2 S3 改名(已实施) · #3 MCP · #4 索引 · #5 dataview · #6/#7 批次 A · #8 插件系统。

## 备份子系统(2026-11-10 三批全落地)

- [x] **附件读延迟对位(2026-11-14,除发版外全做)**:①后端磁盘缓存 `src/disk_cache.rs`(`ATTACHMENT_CACHE_DIR` 默认关、`ATTACHMENT_CACHE_MB` 默认 2048,`blobs/{sha}` 一一对应落盘,LRU 按 atime 淘汰,单飞合并并发回源;只缓存内容寻址的不可变 blob,Range 部分响应不回填);②HTTP 缓存语义:`Cache-Control: private, max-age=31536000, immutable` + `ETag: "sha256-<sha>"` + `If-None-Match` → 304,legacy 非 sha 对象退 `max-age=300`;③compose 两版挂 `pergame_cache:/var/cache/pergame` + docs 新增「Attachment cache」节。实测(本机活后端,300 KB 附件):冷取 TTFB 0.316s → 热取 0.013s(24×),字节一致;`If-None-Match` → 304;`Range: bytes=100-199` 从盘上 206 + 字节一致;单测 3/3(存读/超额不缓存/LRU),全套 143/143。
- [x] **附件缓存审阅修复(2026-11-14)**:上条的 `immutable` 头审阅后发现 P1 bug——附件 URL 稳定(`/api/resources/:id/file`,重传后 id 不变),浏览器 HTTP cache 在新鲜期内直接回旧字节不发请求,前端 SW 失效也救不了;修复 = sha 分支 `Cache-Control` 改 `private, no-cache` 保留 ETag(SW 命中 0 网络,SW miss 一次 304)。同批:`object_ref` sha 加 hex 字符集校验(防 `../` 穿越 64 长构造)、多 range(`bytes=0-1,5-6`)不再原样透传 S3(忽略退全量,合规)、启动时清扫 crash 残留 tmp 文件(`DiskCache::cleanup_tmp`)。avatar 不进盘缓存维持现状(URL 带 `?v=` busting + SW 层,可变对象不适用 immutable 盘缓存)。docs「Attachment cache」节同批改口径。单测 +3(no-cache 头/非 hex 拒绝/tmp 清扫),全套 146/146。
- [ ] **v0.0.12 发版 + stack 42 更新**(用户明确暂缓):线上 `GET /api/resources/:id/file` 仍 404,本轮缓存改动与 immutable 修复也需随此版上线才生效。
- [ ] **实机验证**:有 docker 的机器上跑 `docker compose up` → Instance DR 配 conf → 立即备份 / `-l` / `-R`;前端异地方向(settings → Offsite snapshot targets)建 target 跑一次真实异地复制。

## 换机续接(2026-11-09)

- 续接点唯一正本 = `.work/pergame-infra/GOAL-STATE.md` §3(当前精确位置 / 下一步动作 / 环境指引 / 验证门 / 最近批次)。
- 环境重建三件套:`rustup override set 1.95.0`(backend-rs)、`GOPROXY=https://goproxy.cn,direct`、`bun install --registry https://registry.npmmirror.com`。
