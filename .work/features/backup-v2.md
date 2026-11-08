# Feature Spec — Backup v2(全量还原 + 多端备份)

> 日期:2026-11-08。跟踪:issue #9。需求源:用户 2026-11-08。
> 现状审计:zip = documents.json + files/ + user_settings.json + manifest/files_index;**还原实现不存在**(全仓 restore 零命中);范围 = 单用户。

## 0. 定位澄清(2026-11-08,与用户对齐):两级还原,互补不重复

| 层级 | 手段 | 覆盖 |
|---|---|---|
| **实例级 DR**(管理员) | `pg_dump` 全量 + S3 附件桶**异地**副本 + config/.env secrets——**应用外三件套,零代码,今天(mysqldump 版)即可 cron 化** | 整机重建;DB dump 天然含账号/关系/配置 |
| **用户级可移植**(v2 zip) | 本 spec 的归档+还原 | 跨实例搬家、单用户选择性还原、无管理员权限自救 |

- zip 缺的(账号/关系/配置)实例级 DR 由 DB dump 补;zip 独有价值 = **跨实例/细粒度**。
- caveats(实例级):dump 与 S3 时点对齐(还原按 files_index 对账孤儿);S3 桶与 DB 同机 = 单点,桶必须异地副本。
- 现有 v1 zip 的现实定位 = **人工逃生舱**(解开即读,md 正文/附件可人肉捞)。
- 多端备份(§3 多目的地扇出)同时服务两层:DR 目的地(含 NAS WebDAV)与用户导出目的地共用机制。

## 1. 归档格式 v2(解决"能不能完全还原")

`collectanea-backup-v2.zip`:

| 文件 | 内容 | 还原什么 |
|---|---|---|
| `manifest.json` | format_version=2、app_version、user_id、created_at、instance_id、options | 版本闸门(宿主拒绝不兼容格式) |
| `account.json` | 用户档案(username/email/role/type)+ **bcrypt 哈希**(Rust 可直接校验,零迁移)+ totp 状态(不含 secret 明文,含加密 secret 由用户决定) | 账号 |
| `nodes.json` | 全节点(id/parent_id/name/description/tags/role/color/icon/theme/thumbnail/order/content/content_compiled/metadata/时间戳)——**保留原 Snowflake id** | 笔记 + 树结构 |
| `relations.json` | permissions(node↔user↔level)+ node_invitations | **关系** |
| `settings.json` | user_settings 全量 | 配置 |
| `oidc_links.json` | 用户↔provider 关联(不含 provider 密钥——实例级配置不入用户备份) | SSO 关联 |
| `plugins/` | 各插件 KV 命名空间导出(插件系统落地后生效;v2 格式预留目录) | 插件数据 |
| `files/` | 附件二进制(路径规则沿用 `<id><ext>`) | 附件 |
| `files_index.json` | 路径 ↔ node_id ↔ 原名 ↔ size ↔ **sha256** | 完整性校验 + 恢复映射 |
| `checksums.txt` | 全文件 sha256 清单 | 归档级校验 |

边界(明确不进用户备份):实例 config.toml、SMTP/OIDC provider 密钥、sessions(临时态)、stats/logs(遥测)。全实例备份(管理员级)= v2 后评估。

## 2. 还原流程(v1 没有的能力,新增)

1. 入口:`POST /api/backup/restore`(上传 zip)或从备份目的地拉取。
2. 校验:manifest 版本闸门 → checksums 全量校验 → 报告(将创建/将冲突)。
3. **冲突策略**(用户选择):`preserve_ids` 同 id 已存在 → skip / overwrite / duplicate(重映射 id);默认 overwrite 当前账户同名树前先确认。
4. 事务应用:nodes/relations/settings 批量写;附件按 files_index 回传 S3;sha256 校验失败 = 该文件报错不入库。
5. 输出:还原报告(计数/跳过/失败清单)。

### 2.1 新实例还原(2026-11-08 与用户对齐,v2 核心价值)

流程:新实例部署 → 注册新账号 → 登录 → 上传 v2 zip → 映射到该账号。

- 账号:`account.json` 回填档案(username/avatar 等),密码哈希**可选**覆盖(覆盖 = 旧密码直接登录)。
- 笔记树按原 Snowflake id 重建,全新实例基本零冲突;撞 id 走冲突策略。
- 附件回传新实例 S3,逐文件 sha256 对账。
- **单用户数据 100% 可迁;跨用户关系尽力还原**:relations 指向的其他用户在新实例不存在 → 跳过 + 报告;OIDC 关联需新实例配置同名 provider,否则跳过 + 警告;插件数据需同名插件在装。

## 3. 多端备份(需求主项)

配置形态(config.toml;Rust 侧对应结构):

```toml
[Backup]
Destinations = [
  { Name = "primary-s3",  Type = "s3",     Endpoint = "...", Bucket = "...", AccessKey = "...", Secret = "...", Secure = true, PathStyle = false, Prefix = "backups/", RetainDays = 7 },
  { Name = "offsite-s3",  Type = "s3",     ... },
  { Name = "nas-webdav",  Type = "webdav", Url = "https://nas/dav/collectanea/", Username = "...", Password = "...", RetainDays = 30 },
]
```

行为:
- 备份任务 = 本地构建 zip → **扇出**全部启用目的地(串行,逐目的地记录结果)。
- 任务状态含 per-destination 结果(ok/failed/retry);单目的地失败不影响其它目的地与任务成功态(部分成功 = warning)。
- 重试:每目的地固定 N 次(默认 2)指数退避。
- 清理:支持 RetainDays 的目的地(S3 lifecycle / webdav 列举删除)按策略清旧;无支持则跳过并提示。
- 校验:上传后回读 sha256(可选,`VerifyUpload = true` 默认关)。
- WebDAV 实现:reqwest dav 客户端(PUT/MKCOL/PROPFIND/DELETE,手写最小集,无重依赖)。

## 4. 里程碑(随 Rust 重写批次)

| 阶段 | 内容 | 估时 |
|---|---|---|
| BV-A | 归档格式 v2 构建(替换现 zip 逻辑) | 1-2 天 |
| BV-B | 还原端点 + 冲突策略 + 报告 | 2-3 天 |
| BV-C | 多目的地扇出(s3 + webdav)+ 重试/清理 | 3-4 天 |
| 合计 | | 6-9 天,插在 B1(P1 后)与 B3/B4 之间 |

## 5. 开放问题(实施时定)

1. `account.json` 是否含 totp_secret 密文(加密口令从哪来)?
2. 还原到**不同**用户(导入他人备份)是否允许 → 建议 v2 只允许还原到自己,跨用户 = 管理员端点后评估
3. 自动定时备份(现有 jobs 手动触发)是否纳入 v2 → 建议纳入(cron 简版)
4. 旧 v1 zip 是否提供还原兼容 → 建议:读 v1 且仅支持 nodes/files/settings 子集,报告标注"部分还原"
