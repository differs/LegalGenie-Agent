import type {
  CaseDetail,
  CaseListResponse,
  CaseMember,
  CasePerson,
  ChunkListResponse,
  CreateRelationshipRequest,
  DedupeResponse,
  EvidenceFileDetail,
  EvidenceFileSummary,
  ExportHistoryResponse,
  ExportRecord,
  FileListResponse,
  LogListResponse,
  MergeRequest,
  MergeResult,
  MyRoleResponse,
  NodeListResponse,
  PersonListResponse,
  PersonRelationship,
  RelationshipListResponse,
  SearchHistoryItem,
  SearchResponse,
  Session,
  TimelineNode,
  UserInfo,
} from './types'

const API_BASE_URL =
  (import.meta.env.VITE_API_BASE_URL as string | undefined)?.trim() ||
  'http://127.0.0.1:8001/api/v1'

type ApiEnvelope<T> = {
  code: number
  message: string
  data?: T
  timestamp: number
  error_code?: number | null
}

export type HealthResponse = {
  status: string
  version: string
  database: string
}

export type UserInfoResponse = UserInfo

export type LoginPayload = {
  username: string
  password: string
  remember_me: boolean
}

export type RegisterPayload = {
  username: string
  email: string
  password: string
  real_name?: string
}

type LoginResponseData = {
  user: UserInfo
  access_token: string
  refresh_token: string
  expires_in: number
}

export type CreateCasePayload = {
  name: string
  description?: string
}

export type CreateNodePayload = {
  title: string
  description?: string
  event_time: string
  tags?: string[]
}

export type CreatePersonPayload = {
  name: string
  gender?: string
  phone?: string
  email?: string
  organization?: string
  position?: string
  role_type?: string
  role_detail?: string
  involved_date?: string
}

export type LinkEvidencePayload = {
  evidence_id: string
  anchor_type: string
  anchor_data?: Record<string, unknown>
}

export class ApiError extends Error {
  status: number
  errorCode?: number | null

  constructor(message: string, status: number, errorCode?: number | null) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.errorCode = errorCode
  }
}

export const apiBaseUrl = () => API_BASE_URL

async function request<T>(
  path: string,
  init?: RequestInit,
  token?: string,
): Promise<T> {
  const headers = new Headers(init?.headers)

  if (!headers.has('Content-Type') && init?.body && !(init.body instanceof FormData)) {
    headers.set('Content-Type', 'application/json')
  }
  if (token) {
    headers.set('Authorization', `Bearer ${token}`)
  }

  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...init,
    headers,
  })

  // Blob responses (file downloads / previews) bypass the envelope.
  const contentType = response.headers.get('content-type') ?? ''
  if (contentType.includes('application/octet-stream') || contentType.includes('application/pdf')) {
    if (!response.ok) {
      throw new ApiError(`Download failed with status ${response.status}`, response.status)
    }
    return (await response.blob()) as T
  }

  const payload = (await response.json()) as ApiEnvelope<T>

  if (!response.ok || payload.code !== 200 || payload.data === undefined) {
    throw new ApiError(
      payload.message || `Request failed with status ${response.status}`,
      response.status,
      payload.error_code,
    )
  }

  return payload.data
}

function toSession(data: LoginResponseData): Session {
  return {
    user: data.user,
    accessToken: data.access_token,
    refreshToken: data.refresh_token,
    expiresIn: data.expires_in,
  }
}

// ---------- Auth ----------

export async function fetchHealth(): Promise<HealthResponse> {
  return request<HealthResponse>('/health')
}

export async function login(payload: LoginPayload): Promise<Session> {
  const data = await request<LoginResponseData>('/auth/login', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
  return toSession(data)
}

export async function register(payload: RegisterPayload): Promise<Session> {
  const data = await request<LoginResponseData>('/auth/register', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
  return toSession(data)
}

export async function refreshToken(refreshToken: string): Promise<Session> {
  const data = await request<LoginResponseData>('/auth/refresh', {
    method: 'POST',
    body: JSON.stringify({ refresh_token: refreshToken }),
  })
  return toSession(data)
}

export async function logout(token: string): Promise<void> {
  await request<unknown>('/auth/logout', { method: 'POST' }, token)
}

export async function fetchMe(token: string): Promise<UserInfo> {
  return request<UserInfo>('/auth/me', undefined, token)
}

// ---------- Cases ----------

export async function fetchCases(
  token: string,
  page = 1,
  pageSize = 50,
): Promise<CaseListResponse> {
  return request<CaseListResponse>(
    `/cases?page=${page}&page_size=${pageSize}`,
    undefined,
    token,
  )
}

export async function fetchCase(token: string, caseId: string): Promise<CaseDetail> {
  return request<CaseDetail>(`/cases/${caseId}`, undefined, token)
}

export async function createCase(
  token: string,
  payload: CreateCasePayload,
): Promise<CaseDetail> {
  return request<CaseDetail>(
    '/cases',
    { method: 'POST', body: JSON.stringify(payload) },
    token,
  )
}

export async function updateCase(
  token: string,
  caseId: string,
  payload: Partial<CreateCasePayload> & { status?: string },
): Promise<CaseDetail> {
  return request<CaseDetail>(
    `/cases/${caseId}`,
    { method: 'PUT', body: JSON.stringify(payload) },
    token,
  )
}

export async function deleteCase(token: string, caseId: string): Promise<void> {
  await request<unknown>(`/cases/${caseId}`, { method: 'DELETE' }, token)
}

export async function fetchCaseMembers(
  token: string,
  caseId: string,
): Promise<CaseMember[]> {
  const data = await request<{ members: CaseMember[] }>(
    `/cases/${caseId}/members`,
    undefined,
    token,
  )
  return data.members
}

export async function fetchMyRole(token: string, caseId: string): Promise<MyRoleResponse> {
  return request<MyRoleResponse>(`/cases/${caseId}/members/me`, undefined, token)
}
// ---------- Timeline nodes ----------

export async function fetchNodes(
  token: string,
  caseId: string,
  page = 1,
  pageSize = 200,
): Promise<NodeListResponse> {
  return request<NodeListResponse>(
    `/cases/${caseId}/timeline/nodes?page=${page}&page_size=${pageSize}`,
    undefined,
    token,
  )
}

export async function createNode(
  token: string,
  caseId: string,
  payload: CreateNodePayload,
): Promise<TimelineNode> {
  return request<TimelineNode>(
    `/cases/${caseId}/timeline/nodes`,
    { method: 'POST', body: JSON.stringify(payload) },
    token,
  )
}

export async function updateNode(
  token: string,
  nodeId: string,
  payload: Partial<CreateNodePayload>,
): Promise<TimelineNode> {
  return request<TimelineNode>(
    `/timeline/nodes/${nodeId}`,
    { method: 'PUT', body: JSON.stringify(payload) },
    token,
  )
}

export async function deleteNode(token: string, nodeId: string): Promise<void> {
  await request<unknown>(`/timeline/nodes/${nodeId}`, { method: 'DELETE' }, token)
}

export async function moveNode(
  token: string,
  nodeId: string,
  payload: { event_time?: string },
): Promise<TimelineNode> {
  return request<TimelineNode>(
    `/timeline/nodes/${nodeId}/move`,
    { method: 'POST', body: JSON.stringify(payload) },
    token,
  )
}

export async function linkEvidenceToNode(
  token: string,
  nodeId: string,
  payload: LinkEvidencePayload,
): Promise<{ link_id: string }> {
  return request<{ link_id: string }>(
    `/timeline/nodes/${nodeId}/evidence`,
    { method: 'POST', body: JSON.stringify(payload) },
    token,
  )
}

export async function unlinkEvidenceFromNode(
  token: string,
  nodeId: string,
  linkId: string,
): Promise<void> {
  await request<unknown>(
    `/timeline/nodes/${nodeId}/evidence/${linkId}`,
    { method: 'DELETE' },
    token,
  )
}

// ---------- Evidence files ----------

export async function fetchFiles(
  token: string,
  caseId: string,
  page = 1,
  pageSize = 50,
): Promise<FileListResponse> {
  return request<FileListResponse>(
    `/cases/${caseId}/files?page=${page}&page_size=${pageSize}`,
    undefined,
    token,
  )
}

export async function uploadFile(
  token: string,
  caseId: string,
  file: File,
  onProgress?: (percent: number) => void,
): Promise<EvidenceFileSummary> {
  const form = new FormData()
  form.append('file', file)

  const response = await fetch(`${API_BASE_URL}/cases/${caseId}/files`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${token}` },
    body: form,
  })

  if (!response.body) {
    throw new ApiError('Upload stream unavailable', 0)
  }

  // Progress via reader; then parse the JSON envelope.
  const reader = response.body.getReader()
  const chunks: Uint8Array[] = []
  let received = 0
  const total = Number(response.headers.get('content-length') ?? 0)

  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    if (value) {
      chunks.push(value)
      received += value.length
      if (total > 0 && onProgress) {
        onProgress(Math.min(100, Math.round((received / total) * 100)))
      }
    }
  }

  const blob = new Blob(chunks as unknown as BlobPart[])
  const text = await blob.text()
  const payload = JSON.parse(text) as ApiEnvelope<EvidenceFileSummary>

  if (!response.ok || payload.code !== 200 || payload.data === undefined) {
    throw new ApiError(
      payload.message || `Upload failed with status ${response.status}`,
      response.status,
      payload.error_code,
    )
  }
  return payload.data
}

export async function fetchFileDetail(
  token: string,
  fileId: string,
): Promise<EvidenceFileDetail> {
  return request<EvidenceFileDetail>(`/files/${fileId}`, undefined, token)
}

export async function parseFile(token: string, fileId: string): Promise<unknown> {
  return request<unknown>(`/files/${fileId}/parse`, { method: 'POST' }, token)
}

export async function fetchFileChunks(
  token: string,
  fileId: string,
  page = 1,
  pageSize = 100,
): Promise<ChunkListResponse> {
  return request<ChunkListResponse>(
    `/files/${fileId}/chunks?page=${page}&page_size=${pageSize}&view_mode=bilingual`,
    undefined,
    token,
  )
}

export async function retryTranslation(
  token: string,
  fileId: string,
  chunkIds?: string[],
): Promise<unknown> {
  const body = chunkIds?.length ? { chunk_ids: chunkIds } : {}
  return request<unknown>(
    `/files/${fileId}/translate/retry`,
    { method: 'POST', body: JSON.stringify(body) },
    token,
  )
}

export async function downloadFile(
  token: string,
  fileId: string,
): Promise<Blob> {
  return request<Blob>(`/files/${fileId}/download`, undefined, token)
}

export async function downloadParsed(
  token: string,
  fileId: string,
): Promise<Blob> {
  return request<Blob>(`/files/${fileId}/parsed`, undefined, token)
}

// ---------- Persons ----------

export async function fetchPersons(
  token: string,
  caseId: string,
  page = 1,
  pageSize = 100,
): Promise<PersonListResponse> {
  return request<PersonListResponse>(
    `/cases/${caseId}/persons?page=${page}&page_size=${pageSize}`,
    undefined,
    token,
  )
}

export async function createPerson(
  token: string,
  caseId: string,
  payload: CreatePersonPayload,
): Promise<CasePerson> {
  return request<CasePerson>(
    `/cases/${caseId}/persons`,
    { method: 'POST', body: JSON.stringify(payload) },
    token,
  )
}

export async function updatePerson(
  token: string,
  personId: string,
  payload: Partial<CreatePersonPayload>,
): Promise<CasePerson> {
  return request<CasePerson>(
    `/persons/${personId}`,
    { method: 'PUT', body: JSON.stringify(payload) },
    token,
  )
}

export async function fetchDedupeCandidates(
  token: string,
  caseId: string,
): Promise<DedupeResponse> {
  return request<DedupeResponse>(`/cases/${caseId}/persons/dedupe`, undefined, token)
}

export async function mergePersons(
  token: string,
  caseId: string,
  payload: MergeRequest,
): Promise<MergeResult> {
  return request<MergeResult>(
    `/cases/${caseId}/persons/merge`,
    { method: 'POST', body: JSON.stringify(payload) },
    token,
  )
}

export async function fetchRelationships(
  token: string,
  caseId: string,
): Promise<RelationshipListResponse> {
  return request<RelationshipListResponse>(
    `/cases/${caseId}/persons/relationships`,
    undefined,
    token,
  )
}

export async function createRelationship(
  token: string,
  caseId: string,
  payload: CreateRelationshipRequest,
): Promise<PersonRelationship> {
  return request<PersonRelationship>(
    `/cases/${caseId}/persons/relationships`,
    { method: 'POST', body: JSON.stringify(payload) },
    token,
  )
}

export async function deleteRelationship(
  token: string,
  caseId: string,
  relationshipId: string,
): Promise<void> {
  await request<unknown>(
    `/cases/${caseId}/persons/relationships/${relationshipId}`,
    { method: 'DELETE' },
    token,
  )
}

// ---------- Search ----------

export async function search(
  token: string,
  query: string,
  objectTypes?: string[],
  caseId?: string,
  page = 1,
  pageSize = 20,
): Promise<SearchResponse> {
  const params = new URLSearchParams({ keyword: query, page: String(page), page_size: String(pageSize) })
  if (objectTypes?.length) params.set('object_types', objectTypes.join(','))
  if (caseId) params.set('case_id', caseId)
  return request<SearchResponse>(`/search?${params.toString()}`, undefined, token)
}

export async function fetchSearchSuggestions(
  token: string,
  query: string,
): Promise<string[]> {
  const data = await request<{ suggestions: string[] }>(
    `/search/suggestions?keyword=${encodeURIComponent(query)}`,
    undefined,
    token,
  )
  return data.suggestions
}

export async function fetchSearchHistory(token: string): Promise<SearchHistoryItem[]> {
  const data = await request<{ items: SearchHistoryItem[] }>(
    '/search/history?page=1&page_size=20',
    undefined,
    token,
  )
  return data.items
}

// ---------- Exports ----------

export async function exportEvidenceList(token: string, caseId: string): Promise<ExportRecord> {
  return request<ExportRecord>(
    `/cases/${caseId}/exports/evidence-list`,
    { method: 'POST' },
    token,
  )
}

export async function exportTimeline(
  token: string,
  caseId: string,
  format: 'png',
): Promise<ExportRecord> {
  return request<ExportRecord>(
    `/cases/${caseId}/exports/timeline?format=${format}`,
    { method: 'POST' },
    token,
  )
}

export async function exportTimelineReport(
  token: string,
  caseId: string,
  format: 'pdf' | 'html',
): Promise<ExportRecord> {
  return request<ExportRecord>(
    `/cases/${caseId}/exports/timeline-report?format=${format}`,
    { method: 'POST' },
    token,
  )
}

export async function fetchExportHistory(
  token: string,
  caseId: string,
): Promise<ExportHistoryResponse> {
  return request<ExportHistoryResponse>(
    `/cases/${caseId}/exports/history?page=1&page_size=20`,
    undefined,
    token,
  )
}

export async function downloadExport(
  token: string,
  caseId: string,
  exportId: string,
): Promise<Blob> {
  return request<Blob>(
    `/cases/${caseId}/exports/${exportId}/download`,
    undefined,
    token,
  )
}

// ---------- Logs ----------

export async function fetchLogs(
  token: string,
  caseId?: string,
  page = 1,
  pageSize = 50,
): Promise<LogListResponse> {
  const params = new URLSearchParams({ page: String(page), page_size: String(pageSize) })
  if (caseId) params.set('case_id', caseId)
  return request<LogListResponse>(`/logs?${params.toString()}`, undefined, token)
}

export async function exportLogs(
  token: string,
  format: 'csv' | 'xlsx',
): Promise<Blob> {
  return request<Blob>(`/logs/export?format=${format}`, undefined, token)
}

export type { Session } from './types'
export { API_BASE_URL }
