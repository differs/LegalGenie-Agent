# API 接口文档汇总

**文档版本**：v1.0  
**编写日期**：2026 年 3 月 14 日  
**开发负责人**：王舟  

---

## 一、API 规范

### 1.1 基础规范

| 项目 | 规范 |
|------|------|
| 基础路径 | `/api/v1` |
| 认证方式 | Bearer Token (JWT) |
| 请求格式 | `application/json` |
| 响应格式 | `application/json` |
| 字符编码 | UTF-8 |

### 1.2 统一响应格式

```json
{
    "code": 200,
    "message": "success",
    "data": { ... },
    "timestamp": 1710403200
}
```

### 1.3 错误码规范

| 错误码 | 说明 |
|--------|------|
| 200 | 成功 |
| 400 | 请求参数错误 |
| 401 | 未认证/Token 无效 |
| 403 | 无权限 |
| 404 | 资源不存在 |
| 500 | 服务器内部错误 |

---

## 二、认证接口

### 2.1 用户登录

```http
POST /api/v1/auth/login
Content-Type: application/json

{
    "username": "zhangsan",
    "password": "password123",
    "remember_me": true
}
```

**响应**：
```json
{
    "code": 200,
    "data": {
        "user": {
            "id": "user_001",
            "username": "zhangsan",
            "email": "zhangsan@example.com",
            "real_name": "张三",
            "roles": ["host_lawyer"]
        },
        "access_token": "eyJhbGciOiJIUzI1NiIs...",
        "refresh_token": "eyJhbGciOiJIUzI1NiIs...",
        "expires_in": 3600
    }
}
```

### 2.2 用户登出

```http
POST /api/v1/auth/logout
Authorization: Bearer {token}
```

### 2.3 刷新 Token

```http
POST /api/v1/auth/refresh
Content-Type: application/json

{
    "refresh_token": "eyJhbGciOiJIUzI1NiIs..."
}
```

### 2.4 用户注册

```http
POST /api/v1/auth/register
Content-Type: application/json

{
    "username": "zhangsan",
    "email": "zhangsan@example.com",
    "password": "password123",
    "real_name": "张三"
}
```

### 2.5 获取当前用户

```http
GET /api/v1/auth/me
Authorization: Bearer {token}
```

### 2.6 修改密码

```http
PUT /api/v1/auth/password
Authorization: Bearer {token}
Content-Type: application/json

{
    "old_password": "old123",
    "new_password": "new123"
}
```

---

## 三、案件接口

### 3.1 案件列表

```http
GET /api/v1/cases?page=1&page_size=20
Authorization: Bearer {token}
```

**响应**：
```json
{
    "code": 200,
    "data": {
        "cases": [
            {
                "id": "case_001",
                "name": "张三诉李四合同纠纷",
                "description": "采购合同违约纠纷",
                "status": "active",
                "created_at": "2026-03-14T10:00:00Z",
                "member_count": 3,
                "evidence_count": 15,
                "node_count": 28
            }
        ],
        "total": 56,
        "page": 1,
        "page_size": 20
    }
}
```

### 3.2 创建案件

```http
POST /api/v1/cases
Authorization: Bearer {token}
Content-Type: application/json

{
    "name": "新案件名称",
    "description": "案件描述",
    "tags": ["合同", "纠纷"]
}
```

### 3.3 案件详情

```http
GET /api/v1/cases/:id
Authorization: Bearer {token}
```

### 3.4 更新案件

```http
PUT /api/v1/cases/:id
Authorization: Bearer {token}
Content-Type: application/json

{
    "name": "更新后的名称",
    "description": "更新后的描述"
}
```

### 3.5 删除案件

```http
DELETE /api/v1/cases/:id
Authorization: Bearer {token}
```

### 3.6 案件成员管理

```http
GET /api/v1/cases/:id/members
Authorization: Bearer {token}

POST /api/v1/cases/:id/members
Authorization: Bearer {token}
Content-Type: application/json

{
    "user_id": "user_002",
    "role_in_case": "member"
}

DELETE /api/v1/cases/:id/members/:user_id
Authorization: Bearer {token}
```

---

## 四、文件接口

### 4.1 上传文件

```http
POST /api/v1/cases/:case_id/files
Authorization: Bearer {token}
Content-Type: multipart/form-data

file: (binary)
```

**响应**：
```json
{
    "code": 200,
    "data": {
        "id": "evidence_001",
        "original_name": "合同.pdf",
        "file_type": "application/pdf",
        "file_size": 1024000,
        "page_count": 10,
        "parsed_text": "合同内容...",
        "storage_path": "files/case_001/uuid.pdf"
    }
}
```

### 4.2 文件列表

```http
GET /api/v1/cases/:case_id/files?page=1&page_size=20
Authorization: Bearer {token}
```

### 4.3 文件详情

```http
GET /api/v1/files/:id
Authorization: Bearer {token}
```

### 4.4 下载文件

```http
GET /api/v1/files/:id/download
Authorization: Bearer {token}
```

### 4.5 文件预览

```http
GET /api/v1/files/:id/preview
Authorization: Bearer {token}
```

### 4.6 删除文件

```http
DELETE /api/v1/files/:id
Authorization: Bearer {token}
```

### 4.7 重新解析

```http
POST /api/v1/files/:id/parse
Authorization: Bearer {token}
```

---

## 五、时间轴节点接口

### 5.1 节点列表

```http
GET /api/v1/cases/:case_id/timeline/nodes?start_date=2024-01-01&end_date=2024-12-31
Authorization: Bearer {token}
```

**响应**：
```json
{
    "code": 200,
    "data": {
        "nodes": [
            {
                "id": "node_001",
                "title": "合同签订",
                "description": "张三与李四签订采购合同",
                "event_time": "2024-01-15",
                "time_precision": "exact",
                "node_type": "event",
                "priority": "high",
                "color": "#f59e0b",
                "evidence_links": [
                    {
                        "id": "link_001",
                        "evidence_id": "evidence_001",
                        "evidence_name": "采购合同.pdf",
                        "anchor_type": "page",
                        "anchor_data": {"page_num": 3}
                    }
                ],
                "tags": ["合同", "重要"],
                "sort_order": 1,
                "is_milestone": true
            }
        ],
        "total": 156
    }
}
```

### 5.2 创建节点

```http
POST /api/v1/cases/:case_id/timeline/nodes
Authorization: Bearer {token}
Content-Type: application/json

{
    "title": "节点标题",
    "description": "节点描述",
    "event_time": "2024-01-15",
    "node_type": "event",
    "priority": "normal",
    "tags": ["标签 1", "标签 2"]
}
```

### 5.3 更新节点

```http
PUT /api/v1/timeline/nodes/:id
Authorization: Bearer {token}
Content-Type: application/json

{
    "title": "更新后的标题",
    "description": "更新后的描述"
}
```

### 5.4 删除节点

```http
DELETE /api/v1/timeline/nodes/:id
Authorization: Bearer {token}
```

### 5.5 移动节点

```http
POST /api/v1/timeline/nodes/:id/move
Authorization: Bearer {token}
Content-Type: application/json

{
    "new_time": "2024-01-20",
    "new_sort_order": 5
}
```

### 5.6 关联证据

```http
POST /api/v1/timeline/nodes/:id/evidence
Authorization: Bearer {token}
Content-Type: application/json

{
    "evidence_id": "evidence_001",
    "anchor_type": "page",
    "anchor_data": {"page_num": 3}
}
```

### 5.7 解除关联

```http
DELETE /api/v1/timeline/nodes/:id/evidence/:link_id
Authorization: Bearer {token}
```

---

## 六、人物接口

### 6.1 人物列表

```http
GET /api/v1/cases/:case_id/persons?page=1&page_size=20
Authorization: Bearer {token}
```

### 6.2 创建人物

```http
POST /api/v1/cases/:case_id/persons
Authorization: Bearer {token}
Content-Type: application/json

{
    "name": "张三",
    "gender": "male",
    "phone": "13800138000",
    "organization": "XXX 公司",
    "position": "总经理"
}
```

### 6.3 人物详情

```http
GET /api/v1/persons/:id
Authorization: Bearer {token}
```

### 6.4 更新人物

```http
PUT /api/v1/persons/:id
Authorization: Bearer {token}
Content-Type: application/json

{
    "phone": "13900139000",
    "organization": "YYY 公司"
}
```

### 6.5 删除人物

```http
DELETE /api/v1/persons/:id
Authorization: Bearer {token}
```

### 6.6 关联案件

```http
POST /api/v1/persons/:id/cases
Authorization: Bearer {token}
Content-Type: application/json

{
    "case_id": "case_001",
    "role_type": "plaintiff"
}
```

### 6.7 人物关系图谱

```http
GET /api/v1/cases/:case_id/persons/graph
Authorization: Bearer {token}
```

---

## 七、搜索接口

### 7.1 全局搜索

```http
GET /api/v1/search?keyword=合同&object_types=evidence,node&page=1&page_size=20
Authorization: Bearer {token}
```

### 7.2 搜索建议

```http
GET /api/v1/search/suggestions?keyword=合
Authorization: Bearer {token}
```

### 7.3 搜索历史

```http
GET /api/v1/search/history?page=1&page_size=20
Authorization: Bearer {token}

DELETE /api/v1/search/history
Authorization: Bearer {token}
```

---

## 八、操作日志接口

### 8.1 日志列表

```http
GET /api/v1/logs?case_id=case_001&user_id=user_001&action=CREATE&page=1
Authorization: Bearer {token}
```

### 8.2 操作历史

```http
GET /api/v1/logs/:target_type/:target_id/history
Authorization: Bearer {token}
```

---

## 九、导出接口

### 9.1 导出证据清单

```http
GET /api/v1/cases/:case_id/exports/evidence-list
Authorization: Bearer {token}
```

### 9.2 导出时间轴

```http
GET /api/v1/cases/:case_id/exports/timeline?format=png
Authorization: Bearer {token}
```

### 9.3 导出记录

```http
GET /api/v1/cases/:case_id/exports/history
Authorization: Bearer {token}
```

---

## 十、完整接口清单

| 模块 | 接口 | 数量 |
|------|------|------|
| 认证 | /auth/* | 6 |
| 案件 | /cases/* | 7 |
| 文件 | /files/* | 7 |
| 时间轴 | /timeline/nodes/* | 7 |
| 人物 | /persons/* | 7 |
| 搜索 | /search/* | 3 |
| 日志 | /logs/* | 2 |
| 导出 | /exports/* | 3 |
| **总计** | | **42** |

---

**文档版本**：v1.0  
**最后更新**：2026 年 3 月 14 日  
**开发负责人**：王舟  
**邮箱**：main@mails.wedevs.org  
**电话**：15378391447
