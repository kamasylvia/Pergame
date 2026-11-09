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
