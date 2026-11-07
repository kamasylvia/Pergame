# Feature Spec — Backup v2(全量还原 + 多端备份)

> 日期:2026-11-08。跟踪:issue #9。需求源:用户 2026-11-08。
> 现状审计:zip = documents.json + files/ + user_settings.json + manifest/files_index;**还原实现不存在**(全仓 restore 零命中);范围 = 单用户。

## 1. 归档格式 v2(解决"能不能完全还原")

`alexandrie-backup-v2.zip`:

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

## 3. 多端备份(需求主项)

配置形态(config.toml;Rust 侧对应结构):

```toml
[Backup]
Destinations = [
  { Name = "primary-s3",  Type = "s3",     Endpoint = "...", Bucket = "...", AccessKey = "...", Secret = "...", Secure = true, PathStyle = false, Prefix = "backups/", RetainDays = 7 },
  { Name = "offsite-s3",  Type = "s3",     ... },
  { Name = "nas-webdav",  Type = "webdav", Url = "https://nas/dav/alexandrie/", Username = "...", Password = "...", RetainDays = 30 },
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
