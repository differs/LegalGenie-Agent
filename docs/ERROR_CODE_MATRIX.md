# 错误码矩阵（Error Code Matrix）

Last updated: 2026-08-10（P0 审计）
基准：`Requirement-Analysis-and-Quotation/错误处理规范.md`

## 约定

- `ApiEnvelope.code` = HTTP 状态码；`error_code` = 业务错误码（6 位，模块前缀）
- 前缀：`40xxxx` 认证、`41xxxx` 案件、`42xxxx` 文件、`43xxxx` 时间轴、`44xxxx` 人物、`45xxxx` 搜索（预留）、`46xxxx` 导出（预留）、`47xxxx` 日志（预留）

## 已实现（代码中实际使用）

| error_code | 名称 | HTTP | 使用位置 |
|---|---|---|---|
| 400000 | BAD_REQUEST | 400 | 兜底（无专属码时） |
| 400101 | AUTH_INVALID_CREDENTIALS | 401 | auth login/register |
| 400103 | AUTH_USER_INACTIVE | 403 | auth 用户停用（2 处） |
| 400104 | AUTH_TOO_MANY_ATTEMPTS | 429 | 登录限流（带 Retry-After） |
| 400105 | AUTH_PASSWORD_WEAK | 400 | 密码强度校验 |
| 401000 | UNAUTHORIZED | 401 | 兜底 |
| 401001 | TOKEN_INVALID | 401 | JWT 校验（access/refresh） |
| 403000 | FORBIDDEN | 403 | 兜底 |
| 404000 | NOT_FOUND | 404 | 兜底 |
| 409000 | CONFLICT | 409 | 兜底 |
| 410101 | CASE_NOT_FOUND | 404 | cases 查询 |
| 410102 | CASE_NO_PERMISSION | 403 | access.rs 权限 |
| 420101 | FILE_NOT_FOUND | 404 | files/persons 共用文件查询 |
| 420102 | FILE_TOO_LARGE | 400 | 上传流式中断 |
| 420103 | FILE_TYPE_NOT_ALLOWED | 400 | file_security 校验 |
| 422000 | UNPROCESSABLE | 422 | 参数验证兜底 |
| 430101 | NODE_NOT_FOUND | 404 | timeline 节点 |
| 440101 | PERSON_NOT_FOUND | 404 | persons 查询 |
| 500000 | INTERNAL | 500 | 兜底 |

## 与规范文档的差距（P0 审计结论）

### 已定义但代码未使用
- `400102 AUTH_USER_NOT_FOUND`（login 统一返回 400101，避免用户枚举——**有意为之，保持现状**）
- `410103 CASE_ALREADY_DELETED`（删除案件被统一为 410101 NOT_FOUND，避免泄露——**有意为之，保持现状**）
- `420104 FILE_PARSE_FAILED`（解析失败通过 `parse_status=failed` + `parse_error` 呈现，非同步错误——保持现状）
- `430102 NODE_TIME_CONFLICT`（暂无时间冲突检测逻辑，预留）
- `440102 PERSON_ALREADY_EXISTS`（无唯一性约束，预留）

### 未定义（模块无专属错误码）
| 模块 | 建议前缀 | 建议码 | 说明 |
|---|---|---|---|
| 搜索 | 45xxxx | 450101 SEARCH_INVALID_QUERY | FTS 查询语法错误时返回 |
| 导出 | 46xxxx | 460101 EXPORT_NOT_FOUND / 460102 EXPORT_IN_PROGRESS | 导出历史/下载 |
| 日志 | 47xxxx | 470101 LOGS_EXPORT_FAILED | 日志导出失败 |
| 成员 | 41xxxx | 410104 MEMBER_NOT_FOUND / 410105 MEMBER_ROLE_INVALID | 成员管理 |

> 补齐建议：随 P1/P2 端点重构一并落地，避免为单一错误码引入大量机械改动。

## 统计口径

- 代码中实际出现 26 个 6 位数字（含测试数据中的 UUID 片段，已人工过滤）
- 生产路径错误码 18 个（上表）
- 兜底使用占比可接受：各通用码仅 1 处，模块码覆盖高频路径
