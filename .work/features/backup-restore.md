# Feature Spec — 备份快照管理 + 双入口还原 + dr-backup restore

> 状态:实施中(2026-11-09,用户拍板「都做,按推荐来」;还原 = 快照管理内还原 + 上传 zip 还原双入口)。
> 关联:NL-C(备份 zip 含 links,`4da3779e`→压缩后 `6e0abd8a`);B6 遗产(dr-backup.sh 运维件)。

## 1 快照格式与存储(回答「快照一定是 zip 吗」)

分两层,格式各取所需:

| 层 | 载体 | 格式 | 理由 |
|---|---|---|---|
| 应用内快照(设置页备份) | S3 `pergame-backups` 桶 | **zip**(v1.2.0 格式不变) | 单文件、可下载离线保存、可上传还原、跨实例搬家;内部 documents.json/files/user_settings/manifest 自由演化 |
| DR 快照(服务器级) | rclone 目的地(S3/WebDAV) | **pg_dump custom 格式**(`.dump`,自带压缩,**非 zip**) | 二进制库级快照,pg_restore 原生还原;zip 反而多一层 |

⚠️ 现状缺陷(本批修):备份对象 key 固定 `{uid}/backup.zip` **每次覆盖** + 桶 lifecycle 24h 全删 = 实际只有一个短命快照。快照管理的前提是**多版本化**。

## 2 P1 应用内快照多版本化

- key:`{uid}/snapshots/{epoch_ms}.zip`(时间戳即快照 id,字典序 = 时间序);弃用旧覆盖 key
- 桶 lifecycle:**移除** 24h 全删(storage.rs `set_backup_lifecycle` 调用撤除);保留策略改应用内:默认保留最近 **10** 份,创建成功后 prune 超出部分
- API(均鉴权):
  - `GET /api/backup/snapshots` → 列表 `[{id(epoch_ms), size, last_modified}]` desc
  - `GET /api/backup/snapshots/{id}/download` → presigned GET(文件名 pergame-snapshot-<ts>.zip)
  - `DELETE /api/backup/snapshots/{id}` → 删对象
  - `POST /api/backup/snapshots/{id}/restore` → 见 P2
  - `POST /api/backup/restore/upload`(multipart zip)→ 见 P2
- job 状态端点(`/api/backup/{jobId}`)保留;job 完成后 download_url 指向新快照 presign

## 3 P2 还原引擎(双入口,同一引擎)

入口 A:`POST /api/backup/snapshots/{id}/restore`(桶内快照);入口 B:`POST /api/backup/restore/upload`(用户上传 zip,≤256MB body limit)。

**策略:replace(灾难恢复语义)**——在事务内删除当前用户全部 nodes(级联 node_links/permissions/user_settings FK)后按 zip 重建;不做 merge(语义复杂,v1 不做)。

处理序:
1. manifest.json 校验:`version ≤ 1.2.0`,user_id 字段仅记录不强校验(跨实例搬家场景允许换 user)
2. documents.json:兼容 1.1.0(裸数组=仅 nodes)与 1.2.0({nodes, links});nodes 按原 id 重插(snowflake 冲突 = replace 已清,无冲突);links 重插,**指向缺失 node 的行跳过并进报告**
3. user_settings.json(存在则 upsert general/editor/advanced)
4. files/(存在且带 files_index.json):逐条从 zip `files/` 读字节 → `put_user_object`(key = index 的 transformed_path)还原对象;index 缺对象/字节缺失 → 报告 skipped(不中断)
5. 返回 `RestoreReport { nodes, links, links_skipped, files, files_skipped, settings, warnings[] }`

zip 解析复用 zip crate;内存上限:zip 字节 ≤256MB(axum DefaultBodyLimit),解包单文件 ≤100MB 防zip炸弹。

## 4 P3 dr-backup.sh restore 子命令(运维件)

- `dr-backup.sh -l`:列快照(每源 × 每 ACTIVE 目的地,时间戳目录 desc)
- `dr-backup.sh -R <source> [-t <STAMP>] [-y]`:还原——默认最新快照,rclone copy 到 tmp → `PGPASSWORD=... pg_restore --clean --if-exists -h -p -U -d`(目标 = conf SOURCES 里同名源记录的连接信息);`--clean` 覆盖现库 → tty 交互确认,非 tty 必须 `-y`;`-n` dry-run 打印计划
- 快照布局(既有):`<prefix>/<source>/<STAMP>/<source>.dump`

## 5 前端(设置 → 备份视图)

- 快照列表(时间/大小,desc)+ 行操作:下载 / **还原**(确认框:replace 警示文案)/ 删除
- 「创建快照」(既有 create job)+ 上传 zip 还原(file picker → multipart)
- 还原结果报告展示(含 skipped 列表)

## 6 验收

- 集成测试:备份 → 改库 → 快照还原 → 断言 nodes/links 重建与报告;上传还原同引擎
- dr-backup:-n dry-run 走查 restore 计划;实机验证仍待人工(TODO)
- 前端 lint 绿 + 浏览器走查快照列表/还原流

## 7 批次 A(2026-11-09 已实施,思源对位)

- **还原前安全快照**(SiYuan "Backup before checkout" 对位):两个还原入口执行前同步跑一次 documents+settings 备份(memo = "Pre-restore safety snapshot",约 30s 上限),id 写入 RestoreReport.warnings。
- **快照 memo**:BackupOptions.memo(≤200 字符,trim)→ manifest.json + sidecar `{uid}/snapshots/<ts>.json`;列表联 sidecar 回显;prune 同删。
- **还原预检**:POST /backup/snapshots/:id/preview、POST /backup/restore/upload/preview → RestorePlan(format、incoming/current/replaced/added/removed nodes、孤儿 parent、不可解析 links、files 碰撞[按 transformed_path 集合比对]、settings、warnings)。前端 arm 还原时异步调取并在警示区展示统计。

实施 commits:后端 `8ed0d261`、前端 `2d0d9d55`。

## 8 批次 B:快照存储 v2 — 内容寻址去重 + 可选端到端加密(思源 kernel/backup 对位)

**动机**:zip 全量快照重复打包附件、无去重、明文落桶。v2 采用思源 backup 包模式(manifest + content-addressed blobs),非 chunk 级(避免重造 dejavu)。

**布局**(桶 `pergame-backups`,用户前缀下):
- `{uid}/manifests/{ts}.json` — 快照清单:id/version/created_at/memo/options/statistics/encryption/entries[]/files[]
- `{uid}/blobs/{sha256}`(明文)或 `{uid}/blobs-e2e/{sha256}`(密文)— 内容寻址对象,**跨快照去重**(PUT 前 head 存在即跳过)
- legacy `snapshots/*.zip`(v1)继续可读可还原(列表合并标注 legacy)

**成员**:documents.json、user_settings.json、files/(逐附件,manifest.files[] 记 id/transformed_path/sha256/size)。manifest 自身不进 blobs。

**加密**(可选):env `PERGAME_SNAPSHOT_PASSPHRASE` 非空即启用。AES-256-GCM,key = scrypt(passphrase, salt, N=2^15,r=8,p=1);salt 随机存于 manifest.encryption;blob key 用**明文** sha256(去重语义保持,首写版本为准)。manifest 本身不加密(无敏感内容,不含 passphrase)。还原/下载时按 manifest.encryption 解密 blobs。

**组装兼容**:还原/下载/预检对 v2 = 先按 manifest 拉 blobs(解密)组装成 v1 形态 zip **内存流**,再喂既有 `restore_from_zip`/`plan_from_zip` 引擎(引擎零改动,已测路径全复用)。旧 zip 快照原样支持。

**prune/GC**:keep-10 manifests;GC 删除不被任何保留 manifest 引用的 blobs(引用集 = 保留 manifests 的 entries∪files sha 并集)。

**下载**:统一端点直出 bytes(Content-Disposition attachment;legacy 读 zip 对象,v2 组装),前端改 `window.location = API 下载地址`(cookie 鉴权,弃 presign 路径;job 完成的 presign 保留不变)。

**新增依赖**:scrypt、aes-gcm、rand、hex(小而标准)。

## 9 批次 B 实施(2026-11-09 已落地)

- v2 快照 = `{uid}/manifests/{ts}.json` + `{uid}/blobs[-e2e]/{sha256}`;新快照一律 v2,legacy v1 zip 继续可读可还原可下载(列表合并,`format` 字段区分)。
- 加密:env `PERGAME_SNAPSHOT_PASSPHRASE` 启用;blob 名 `blobs-e2e/`,key 仍为明文 sha256(去重保持);AES-256-GCM + scrypt(N=2^15),nonce 前缀,manifest.encryption 存 salt/KDF 参数;manifest 明文。加密快照解密需同 env。
- 组装:还原/下载/预检对 v2 = 按 manifest 拉 blobs(解密)→ 内存组装 v1 形态 zip → 喂既有引擎(引擎零改动);组装含 manifest.json 成员自身(引擎格式门 MAX_FORMAT=2.0)。
- 下载统一直出(`GET /backup/snapshots/:id/download` → zip bytes + Content-Disposition;cookie 鉴权),前端 `window.location` 到端点;job 完成的 presign 字段退役(前端不再依赖)。
- 删除:v2 删 manifest(blobs 由备份时的 GC 按「保留集引用」清理);v1 删 zip + sidecar。
- 集成测试改走组装下载端点;8/8 绿;24/24 单测(含 snapcrypto 加解密/错密码/salt 唯一)。

实施 commits:后端(本批)、前端(本批)。

## 10 批次 C:实例 DR 管理面板(方案 B:sidecar + admin API + 前端)

**架构**:dr-backup 执行保持进程外(故障域分离不变),新增 sidecar 容器 `pergame-dr-backup`(postgres:17-alpine + rclone + bash + dr-backup.sh)与 backend 通过**两个共享卷**协作,零新增网络面:

- `dr_conf` 卷 → backend `/conf`(rw)、sidecar `/conf`(rw):dr-backup.conf 唯一事实源,backend admin API 读写
- `dr_jobs` 卷 → backend `/jobs`、sidecar `/jobs`:job 协议文件
  - backend 写 `pending/<id>.json` `{id, action: backup|list|restore, source?, ts?}`
  - sidecar 每 2s 扫 pending → `dr-backup.sh -c /conf/dr-backup.conf [flags]`(restore 附 `-y`,admin API + 前端二次确认前置)→ 捕 stdout tail 64KB + exit → 写 `done/<id>.json` → 删 pending
- sidecar 定时备份:SCHEDULE_HOURS env(默认 0=仅手动;>0 每 N 小时自动 backup)

**API**(全部 `is_app_admin`,role=2):
- `GET /api/admin/dr-backup/config` → {exists, content(**敏感字段掩码**:pg 记录第 6 段、s3 第 6/7 段、webdav 第 5 段 → `***`)}
- `PUT /api/admin/dr-backup/config` {content} → `***` 段回填旧值后写盘(0600);bash 可用时附加 `bash -n` 语法校验
- `GET /api/admin/dr-backup/config/structured` → {exists, config}(结构化 JSON:源/目的地/保留期,**密钥一律 `***`**)
- `PUT /api/admin/dr-backup/config/structured` {config} → 校验(禁止 `|`/引号/换行 → 防记录格式损坏)→ 从磁盘原文回填 `***` 密钥 → 渲染 bash conf → 同一 `bash -n` 门 → 写盘(0600)
- `POST /api/admin/dr-backup/jobs` → 创建 pending job
- `GET /api/admin/dr-backup/jobs/{id}` → done 输出或 pending 状态
- 未部署(卷不存在)时全部 503,提示「DR sidecar not deployed」

> **结构化配置(2026-11-09 补)**:conf 仍是 bash(文件被 sidecar `source`),后端 `routes/dr_config.rs` 在 JSON ⇄ conf 之间转换,**部署契约不变**。前端默认走表单;原始 conf 编辑器降级为「高级」折叠区,仅供表单未覆盖的字段使用。新增记录若密钥仍为 `***` 直接报错(否则会写进字面量星号)。

**前端**:settings 新增 admin 段(role=2 可见)「Instance」→ Instance DR 视图:**结构化表单**(PostgreSQL 源增删 / S3 与 WebDAV 目的地增删 / 保留天数 / 上传校验;密钥字段 `***` 不碰即保留)+ 折叠的原始 conf 编辑器 + 保存 + 立即备份 / 列出快照 / 还原(source + ts,二次确认)+ job 输出区。

**cron 语义变化**:原宿主 cron 改由 sidecar 内置循环(SCHEDULE_HOURS)或保留宿主 cron(直接 `docker exec pergame-dr-backup /opt/dr/dr-backup.sh -c /conf/dr-backup.conf`),二选一,文档两种都给。

**已知边界**:sidecar 内 conf 的 pg host 用容器服务名 `postgres`(非 localhost);目的地密钥经 admin API 可写(掩码读)——管理员本就是服务器密钥的持有者,风险同级。

## 11 定时自动快照(2026-11-09 已实施)

`SNAPSHOT_INTERVAL_HOURS`(env,整数小时;未设/0 = 关闭)驱动 backend 内 tokio ticker:每 N 小时为**有数据的每个用户**创建一次快照(memo = "Scheduled snapshot"),复用 `run_backup` 全流程(含 v2 blob 去重、加密、keep-10 保留、blob GC)。首 tick 跳过避免启动风暴;用户有进行中作业时跳过该轮(不抢占)。**层次定位**(用户裁决):应用内快照 = 用户级误删恢复;实例级 3-2-1 异地备份 = dr-backup;不做 git 级版本控制。

## 12 定位澄清(用户裁决 2026-11-09)

备份主要目标是**异地 3-2-1**:dr-backup(整库 pg_dump → 多目的地)是主力;应用内快照是辅助(用户级、可还原、可下载);本地误删恢复靠**回收站机制**(待做,进 TODO);**git 级版本控制不做**(明确否决,不再评估内容寻址版本历史)。

## 13 批次 D:DR 层 Siyuan 对位补齐(2026-11-09 已实施)

对位 `siyuan/kernel/backup` 包,补齐 5 项差距:

| # | 能力 | 实施 |
|---|---|---|
| 1 | 目的地/源连通性探测 | `dr-backup.sh -p <name>` → 单行 `PROBE|kind|name|reachable=|snapshots=|fingerprint=|note=`;`POST jobs {action:"probe", name}` + `get_job` 解析出结构化 `probe` 字段;表单「Test」按钮(源与目的地都有)显示状态条。目的地探测 = rclone lsf -R 目的地根,快照数 = `.dump` 文件计数;源探测 = `psql -c 'SELECT 1'`(PGCONNECT_TIMEOUT=10) |
| 2 | DR 备份加密 | 每目的地 `crypt_pass`(空=明文);rclone **crypt remote** 包裹目的地(`[drb_crypt_<name>]` section,`remote = <base>:<bucket或根>`),上传/列表/剪枝/还原统一走加密层;crypt 启用时 size 校验跳过(密文尺寸≠明文,rclone crypt 内部 nonce+MAC 保证完整性)。**注意:启用 crypt = 全新对象命名空间,旧明文快照不可经 crypt remote 见**(spec §13 同 Siyuan blobs-e2e 语义) |
| 3 | keep-last 保留维度 | 记录尾追加 `retain_days|keep_last`(0=继承全局/关);`prune_dest` = 年龄剪枝(全局 RETAIN_DAYS 兜底)+ keep-last(rclone lsf 目录名倒序,超出 N 个 rclone purge;时间戳目录字典序=时间序) |
| 4 | cron 调度 | 每目的地 `cron`(5 段);**后端调度器**(`spawn_dr_cron_scheduler`,croner 解析,每 60s 重读 conf)对到期目的地写 `dr-backup.sh -T <name>` job;错过不补跑 + 重启不补跑(标准 cron 语义,Siyuan scheduler 对位);pending 已有同目标 job 时跳过(防排队风暴)。sidecar 的 `SCHEDULE_HOURS` 全局循环保留兼容但弃用 |
| 5 | 删除门禁 + 指纹 | 探测输出含**存储指纹**(host:port|bucket|prefix,逗号分隔);表单删除目的地:探测到快照 >0 → **输错名称即拒**(Siyuan deletecheck 简化版),未探测/0 快照 → confirm 警示「远端快照不会被删除」;conf 校验拒绝完全重复的存储位置(同 type+endpoint+bucket+prefix) |

**关键修复(探测首跑即抓到)**:rclone ≥ 1.76 不再从 `RCLONE_CONFIG_<name>_*` 环境变量段解析 remote type("didn't find section in config file")——原脚本 env-section 方案在真机上会全盘失效。改为**运行时生成私有 rclone config 文件**(`$WORKDIR/rclone.conf` + `export RCLONE_CONFIG`),全版本稳定。

**记录格式(v2,向后兼容)**:
```
s3|name|endpoint|bucket|prefix|ak|sk|secure|region|provider[|retain_days|keep_last|cron|crypt_pass]
webdav|name|url|user|password|prefix[|retain_days|keep_last|cron|crypt_pass]
```
旧格式记录(无尾部字段)继续可读(=继承/关),新旧脚本都能 source。

**验证**:41 单测(含 cron 校验/重复指纹/掩码往返/PROBE 解析)全绿;clippy/eslint/stylelint 0;浏览器活测(假 sidecar 消费器 + 真打 NAS QuObjects):探测 reachable=true 且指纹正确、源探测诚实失败(本机无 psql)、conf 全字段落盘、删除 confirm 文案正确、掩码往返保真。**待人工**:crypt 加密路径的真机 dump/restore、cron 触发的实际点火(逻辑已单测)、有快照目的地的删除门禁分支。
## 14 多目标备份(思源 fork kernel/backup 对位;2026-11-10 开工,权威输入 = kamasylvia/siyuan#15 + 本地 Go 实现 kernel/backup/)

### 13.1 语义翻译表(思源工作区 → Pergame DB+S3)

| 思源概念 | Pergame 对应 | 说明 |
|---|---|---|
| 工作区 `data/`(全部文档) | **target 归属用户集合**的 v2 快照对象集(每用户 manifests+blobs,键布局同构) | **裁决 2026-11-10:按用户**——target 上记录 `users[]`(admin 勾选);勾选全部用户 ≡ 实例级。备份时只复制所选用户的对象;还原粒度 = 单 manifest |
| `conf/` | backup_targets 配置本身(排除,不进快照) | 同思源排除 backup.json |
| `history/` | 无(git 级版本已否决) | 不进快照 |
| SQLite 索引重建 | 无需(还原即重建 DB 行) | 内在 |
| 快照 conf.json 身份重置 | N/A(单实例无 system.id;owner claim 保留接口,优先级低) | |
| force 收敛(删快照外文件) | 删目标用户当前非快照内 S3 对象(nodes 天然收敛于 DB 替换) | |
| ReindexRestoredWorkspace | 无需 | |

### 13.2 对象布局(target 上,与本地桶同键)

- `<prefix若配>/manifests/<ts>.json`、`<prefix>/blobs[-e2e]/{sha256}`、`<prefix>/refs/owner.json`、`<prefix>/refs/latest.json`
- 内容 = 本地桶全体用户 v2 对象的字节级副本(fan-out 逐对象 HEAD→PUT 去重复制;加密态原样,e2e blob 不解密)

### 14.3 Store 抽象

```
trait BackupStore: id() / fingerprint() / has(path) / put(path, bytes) / get(path) / delete(path) / list(prefix)
```
- **S3**:endpoint/bucket/prefix/access/secret/secure/region/provider(path-style)
- **WebDAV**:url/user/password/prefix;PROPFIND(depth 1)list、MKCOL 建目录、PUT/GET/DELETE
- **PG**:外部库单表 `<schema>.pergame_backup_objects(path pk, data bytea, size, updated_at)`;fingerprint = `system_identifier`(pg_controldata/函数)+ db name + schema(PG<14 回退 oid)
- 指纹(target 身份):S3 = endpoint|bucket 归一(小写/去尾斜杠);WebDAV = url 归一 + 归属标记内容哈希(从未写入 = 空);PG = system_identifier|db|schema

### 14.4 配置与门禁

**裁决(2026-11-10)**:① target 归属 = **按用户**(admin 勾选,全选=实例级);② 目标 blob = **密文原样复制**(字节级,不解密,不重加密);③ 触发 = **快照成功后自动复制**(增量:仅传新增 blob);④ 删除/改连接 = **完整门禁**(思源 parity)。

- **migration 0006 增列** `users BIGINT[]`(归属用户集合;空数组 = 未指定,拒绝启用)

- **migration 0005**:`backup_targets`(id uuid、kind、name、endpoint/url、bucket_or_schema、prefix、access_key、secret_key、secure、region、provider、schedule cron(P3)、keep_last、max_age_days、enabled、时间戳)
- 身份唯一:保存/编辑按 fingerprint 查重,同存储拒绝双配置
- **凭据掩码**:GET 时 access/secret/password → `***`;PUT `***` 回填库内旧值;新增记录带 `***` = 400
- **testTarget 保存门禁**:connect(列目录或 SELECT 1)+ 掩码回填后探测;连接字段一变即失效(前端 + 后端双检)
- **deleteCheck**:探测连通 + 快照数(目标上有对象 → 拒绝,先 emptyTarget);不可达且无法证明已清空 → 拒绝;指纹一致的重复配置一个可连且已清空 → 另一个放行
- **editCheck**:连接字段变更时同款(指纹一致 或 原存储已清空);名称/保留/启停跳过探测
- **emptyTarget**:两步确认,删全部 manifests+blobs,保留 owner 标记

### 14.5 fan-out 备份与还原

- **fan-out** = 对象级复制:对 target 的**归属用户集合**逐个取本地桶对象(`{uid}/manifests/*` + `{uid}/blobs[-e2e]/*` + refs)逐对象复制(HEAD→PUT 去重;加密 blob **原样密文**复制,不解密)。**自动触发**:每次快照成功后(run_backup 尾部)对所有启用且归属该用户的 target 自动增量复制;另提供 admin 手动「立即同步」
- **还原自 target** = 把 target 上该用户 manifest 及其引用 blobs **拉回本地桶**(缺失才拉),然后走既有 `restore_from_zip` 组装引擎——引擎零改动
- 锁:P1 用现有单作业互斥(同用户);target 级共享/还原独占锁 → P2(与只读中间件同批)

### 14.6 分阶段

- **P1(本批)**:Store 三实现 + 0005 + admin API(config CRUD 掩码/testTarget/deleteCheck/editCheck)+ fan-out 备份 + 每目标快照列表/单删(引用计数)/还原拉回 + owner/latest refs
- **P2**:target 级锁 + 工作区只读中间件 + 前端只读态 + emptyTarget 二步确认强化 + maxAgeDays
- **P3**:zip 同构格式(meta+manifest+blobs)三入口、per-target cron 表达式调度

## 15 P1/P2/P3 全量实施结果(2026-11-10)

**P1 多目标 fan-out ✅**(commits `0fc5b2c4`→`6bdb75b6`→`73addc6b`):Store 三实现(S3/WebDAV/PG)+ 指纹身份去重 + 掩码回填 + deleteCheck/editCheck 门禁 + 按用户 target + 密文原样复制 + 快照后自动复制(增量去重)+ target 侧 keep_last prune/blob GC + fetch 回拉 + admin UI(CRUD/用户勾选/测试/同步/删除/探针)。

**P2 锁与只读 ✅**(`aebfdc01`):备份共享锁(并发允许)/还原独占锁(互斥 + 等备份 60s 排空);中间件对工作区写请求返回 **423**;豁免 `/api/backup`、`/api/admin/dr-backup`、`/api/health`、`/api/auth`、`/api/users/@me`(避免锁死备份机制自身);`GET /admin/dr-backup/status` 暴露 `{backups_in_flight, restore_in_flight, read_only}`;前端 `MaintenanceBanner` 每 4s 轮询显示只读提示(i18n×8)。活测:还原中 status `read_only:true`,写请求 423,还原后恢复 200。

**P3 cron + portable zip ✅**(`646654d6`):5 段 cron 解析(`*`/`*/n`/`a-b`/`a-b/n`/逗号列表,dom|dow 同时受限时按 cron 约定取 OR,dow 0-7),每分钟 tick + 错过的分钟不补跑 + 锁持有期间跳过;`backup_targets.schedule`(migration 0007)。portable zip = `meta.json`(format `pergame-backup/object-set`)+ `manifest.json` + `blobs/<sha256>`,条目名全 ASCII(规避 CP437/UTF-8 乱码),密文原样且不压缩;快照下载对 v2 输出该格式,上传还原/预检自动识别并转换为 v1 形态复用既有引擎;新增 `GET /targets/:id/export/:snapshotId` 直接从 target 组包。

**测试**:单测 49/49(cron 6 + portable 2 + locks 2 + 既有);集成 10/10(新增 `snapshot_targets_lifecycle` 覆盖创建/掩码/去重/增量/门禁/清空/删除);clippy 0;eslint/stylelint 0。

**踩坑**:①已应用的 migration 不可再编辑——新增列/字段必须拆新迁移(sqlx 报 `migration N was previously applied but has been modified`);②软删除批次落地后,还原预检的 `current_nodes` 不再等于"物理节点数",断言应比较关系而非绝对值;③JWT 携带 role 快照,DB 提权后必须重新登录才生效(测试辅助需显式重登)。

## 14 批次 E:Siyuan issue #15 借鉴(2026-11-10)

对位 siyuan fork issue #15(备份子系统设计权威文档),补齐 3 项、评估否决 1 项:

| # | 能力 | 实施 |
|---|---|---|
| 1 | **namespace 作用域**(§1) | `migration 0008` 加 `backup_targets.namespace`;**由内核在创建目标时自动分配 ULID**(26 位 Crockford Base32,前 10 位毫秒时间戳),用户既不可输入也不可修改、前端不显示(siyuan 2026-11-10 定稿口径)。`effective_prefix()` 把 namespace 并入 store prefix,`manifests/ blobs/ refs/` 全落各自前缀——多实例共用一个桶时谱系/认领/保留互不干扰。存量行(该列引入前创建)在每次 list 时自愈补发 ULID。PG 目标无 prefix 概念,由 `PgTarget::ns_key` 在 key 前加一段,list 时剥回,调用方永远只看到逻辑 key |
| 2 | **归属认领**(§3) | `target/owner.rs`:`refs/owner.json` 记 `{instance_id, host, claimed_at}`。实例身份 = `INSTANCE_ID` env,缺省 hostname/库名。写路径(fan-out/定时)先 `claim(force=false)`:无主则认领、同名则刷新、异主则**拒绝写入**并在 `last_result` 记原因。显式接管 = `POST targets/:id/take-ownership`(「last configured instance wins」但必须由人触发)。读路径(还原/管理/导出)**不要求认领**。probe 输出新增 `owner_instance` / `owned_by_me` |
| 3 | **保存前连接测试门禁**(§9) | `POST targets/probe-draft`:对未保存的表单字段探测,`***` 凭据按 `id` 从库中回填真值;返回 `connection_token`(连接身份字段的 sha256 前 12 字节,不含密钥、不含名称/保留/定时)。保存时服务端**重算 token 并比对**,不符即 409「字段已变,请重测」;新建或连接字段变过而未带 token → 409「先测试连接」。前端 Test 按钮 → 通过后 Save 才可用,任一连接字段编辑立刻失效 |
| — | **还原身份重置**(§5) | **判定不适用**。Siyuan 的 system.id/锁屏口令属于实例级配置,其快照会整份覆盖 conf 目录;Pergame 快照按 `user_id` 隔离且**只含 general/editor/advanced 三类偏好**(不含凭据、不含实例身份),跨实例迁移不存在身份继承。强行加 reset 是无对象的重构。记录于此供将来若引入实例级配置快照时复查 |

**测试**:3 个新单测(connection_token 忽略展示/保留字段、不泄露密钥、namespace 字符集)+ 既有 54 全绿;clippy/eslint/stylelint 0;typecheck 无新增。

**待人工**:两实例共用一桶的实机认领验证、namespace 隔离的对象树核对、保存门禁的浏览器走查(UI 已接但本机 IAB 按钮 actionability 受限)。

## 16 全备份重构(2026-11-12,用户裁决:快照 = 全备份,dr-backup 删除)

**背景**:快照只导用户文档/附件/偏好,权限/邀请/回收站/档案全不在备份里;实例灾备靠 dr-backup 边车(pg_dump 整库)。用户裁决:快照扩容成全备份,边车删除;备份默认无——只在用户配了自己的 S3/WebDAV 后才有异地,服务 S3 零备份字节。

### 16.1 UUIDv7 id 统一(破坏性,借清库窗口)

- 三选一(Snowflake/ULID/UUIDv7,不考虑存量):用户先要求可读性→ ULID;去掉可读性后改判 **UUIDv7**(pg 原生 16B 类型 + RFC 标准 + 零协调;Snowflake 两种情况都不选:8 字节省的量级无意义,买来机器 id 协调 + JS 精度坑)。
- 全表主键/外键 BIGINT → UUID;`types/snowflake.rs` + `models::sf/sf_opt` 删除;`types::new_id()` = `Uuid::now_v7()` 唯一生成器;JWT sub/ADMIN_ACCOUNTS 走 UUID 字符串。
- migration 压缩:0001/0003 改 UUID;0006 吸收 0007/0008(users UUID[]/schedule/namespace 进 baseline);0007/0008 文件删除。`_sqlx_migrations` 已清,新链当全新库跑。
- 前端零改(id 线上本就是 string),除 target users 复选框 `Number(u.id)` → 直传 string。

### 16.2 快照直写用户 target(替代 fan-out)

- `run_backup`:收集不变,写段改逐 target 全写(claim → replicate → prune,失败记 warning 继续;全失败则 job 失败)。服务桶不再写任何快照字节。
- 零 target = fork §4「save zip locally」:内存组 portable zip → `BackupStore.inline_zips`(256MB 上限) → `GET /api/backup/:jobId/download` 一次性下载。
- 读路径(list/download/delete/restore/preview)读首个可达 target;delete 删全部可达 target;全不可达 = 快照不可见(空列表,不报错)。
- 删除:`fanout_user`/`fetch_user_from_target` + 两边 fetch 路由;`spawn_target_scheduler` 改直写快照;`export_portable_zip`(S3 版)删除,代之以 conn 版 + 内存版。
- 存量服务桶快照成孤儿:不迁移、不回落(用户明确)。

### 16.3 用户级快照密码(方案 A:口令传参)

- 口令只活在用户手里:`POST /backup` 传 passphrase → 加密;还原/下载/组装同样传参;后端永不存储。定时/安全快照传 env fallback(`PERGAME_SNAPSHOT_PASSPHRASE`,一般未设 = 明文):「没口令就跳过加密快照」。
- 附带修复:`safety_snapshot` 之前 create job 后从不 spawn runner,永远超时、从未真正执行。改为 inline 同步跑 + `BackupStore::register`(不抢用户当前 job)。

### 16.4 全量导出(六成员)

- `permissions.json`:(node_id, grantee_username, permission)。存用户名不存 id:被授权人 id 跨实例悬空,纯垃圾占带宽;用户名可解析、可跳过。
- `invitations.json`:整行(邀请码自包含,无悬空引用)。
- `trash.json`:{nodes}(带 deleted_at),与活 documents.json 分离,还原 merge 语义。
- `profile.json`:展示字段(username/names/avatar/email),无密码/TOTP/role/type。
- 还原:permissions 按用户名回查(skip unknown + ON CONFLICT 去重);invitations 按 code 去重;trash ON CONFLICT(id) DO NOTHING;profile 逐字段 collision 门(username/email 撞则保留现状 + warning)。
- preview 加四组成员计数器 + profile_in_archive;restore 加 force 门(非空工作区无 force 拒绝,fork §5;trash 不计入非空)。

### 16.5 Target 配置权下放 + PG 删除

- 新开 `/api/backup/targets/*`(10 路由):同 store,归属鉴权(users 含我或 admin);普通用户创建/编辑强制 users=[self];takeover 留 admin 独占。前端视图搬出 admin 区到工具区,users 框/takeover 按钮 admin 可见。
- PG target 删除(fork §1 已删:与主库同故障域):`target/pg.rs` + 全部分支 + 前端 kind 选项 + 8 语言 schema 键/subtitle。存量 pg 行因 kind() 拒绝而惰性失效(可列可删,不可连)。
- 启动不再建 `pergame-backups` 桶(主桶保留,附件用)。

### 16.6 账户删除(补缺失的 handler)

- `DELETE /users/:id` 之前根本不存在(前端调进 404)。补 handler:self 或 admin;三门禁:①任一归属 target 有 manifest → 409(先 empty);②不可达 target 同样拒绝(无法证明清空,fork §8);③不能删最后一个 admin。
- 删除清主桶 `{uid}/` 前缀 + users 行(FK 级联清其余);target 对象不动(用户自己的 S3)。

### 16.7 fork §4 锁对齐

- 同 target 重入拒绝:`acquire_backup(targets)` 登记在飞 target 集,交集非空返回 TargetBusy(列冲突 id);guard drop 时同步释放。`LockSnapshot` 加 busy_targets。
- 只读收紧:删 `/api/users/@me` 豁免(备份时改设置 = 快照不一致;GET 照旧放行)。
- 删除锁:delete_snapshot + 两边 empty_target 在 restoring 时 409。

### 16.8 dr-backup 删除

- 快照全备份后,边车存在的唯一理由(补 pg 那一半)消失。删除:`scripts/dr-backup.sh` + sidecar Dockerfile/entrypoint/conf.example + `routes/dr_config.rs` + admin conf/job/cron handler + `instance-dr.vue` + 菜单 + compose 服务 + DR env + docs 相关节 + spec §10/§13(标已移除,历史保留)。
- 失去的能力:整库单事务一致性点(逐用户快照是 N 个时间点;单用户实例无影响)。

### 16.9 前端

- targets 视图搬迁 + API 换前缀 + admin 元素隐藏;backups 页:无 target 提示条 + 跳转、快照密码框、加密锁标、还原口令/force 确认框、plan 新行、full 明细 toast、upload 强制 force。
- i18n:12 新键 en + zh-CN + zh-TW(余者 fallback en)。
- **2026-11-13 补:targets 行「Empty」按钮**(`emptyTarget` + `confirm()` + toast 显示 `deleted_objects`)。此前 `POST /backup/targets/:id/empty` 只有 API,前端无入口 → 目标上只要有对象,`delete-check` 永远拒绝,用户在 UI 里无法自解,只能手敲 API。i18n 4 键 × 8 语言(含撇号语言用双引号包裹)。
