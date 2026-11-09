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
