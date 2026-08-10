// Domain types mirroring the LegalGenie backend API contracts under /api/v1.
// Field names follow the server's serde output verbatim.

// ---------- Auth ----------

export type UserInfo = {
  id: string
  username: string
  email: string
  real_name?: string | null
  roles: string[]
}

export type Session = {
  user: UserInfo
  accessToken: string
  refreshToken: string
  expiresIn: number
}

export type CaseRole = 'owner' | 'member' | 'viewer'

// ---------- Case ----------

export type CaseSummary = {
  id: string
  name: string
  description?: string | null
  status: string
  created_at: string
  member_count: number
  evidence_count: number
  node_count: number
}

export type CaseListResponse = {
  cases: CaseSummary[]
  total: number
  page: number
  page_size: number
}

export type CaseDetail = {
  id: string
  name: string
  description?: string | null
  status: string
  created_at: string
  updated_at: string
}

export type CaseMember = {
  user_id: string
  username: string
  email: string
  real_name?: string | null
  role_in_case: CaseRole
  joined_at: string
  joined_by: string
}

export type MyRoleResponse = {
  case_id: string
  user_id: string
  username: string
  role_in_case: CaseRole
}

// ---------- Timeline ----------

export type EvidenceLink = {
  id: string
  evidence_id: string
  anchor_type: string
  anchor_data: Record<string, unknown> | null
}

export type TimelineNode = {
  id: string
  case_id: string
  title: string
  description?: string | null
  event_time: string // YYYY-MM-DD
  sort_order: number
  tags: string[]
  created_at: string
  updated_at: string
  evidence_links: EvidenceLink[]
}

export type NodeListResponse = {
  nodes: TimelineNode[]
  total: number
  page: number
  page_size: number
}

// ---------- Evidence files ----------

export type FileTranslationSummary = {
  translation_status: string
  translation_error?: string | null
  source_language?: string | null
  target_language?: string | null
  chunk_count: number
  translated_chunk_count: number
  failed_chunk_count: number
  translation_provider?: string | null
  translation_model?: string | null
  translation_incomplete: boolean
}

export type EvidenceFileSummary = FileTranslationSummary & {
  id: string
  original_name: string
  file_type: string
  file_size: number
  storage_path: string
  parse_status: string
  parse_error?: string | null
  created_at: string
}

export type EvidenceFileDetail = EvidenceFileSummary & {
  case_id: string
  parsed_text?: string | null
  page_count?: number | null
  duration?: number | null
}

export type FileListResponse = {
  files: EvidenceFileSummary[]
  total: number
  page: number
  page_size: number
}

export type EvidenceChunk = {
  id: string
  chunk_index: number
  page_number?: number | null
  segment_number?: number | null
  chunk_kind: string
  display_label?: string | null
  source_text: string
  translated_text?: string | null
  translation_status: string
  translation_error?: string | null
  anchor_json?: Record<string, unknown> | null
}

export type ChunkListResponse = {
  items: EvidenceChunk[]
  total: number
  page: number
  page_size: number
  view_mode: string
}

// ---------- Persons ----------

export type CasePerson = {
  id: string
  name: string
  gender?: string | null
  phone?: string | null
  email?: string | null
  organization?: string | null
  position?: string | null
  role_type: string
  role_detail?: string | null
  involved_date?: string | null
  created_at: string
  updated_at: string
}

export type PersonListResponse = {
  persons: CasePerson[]
  total: number
  page: number
  page_size: number
}

export type DedupePerson = {
  id: string
  name: string
  phone?: string | null
  email?: string | null
  organization?: string | null
  position?: string | null
  roles: string[]
}

export type DedupeGroup = {
  key: string
  reason: string
  persons: DedupePerson[]
}

export type DedupeResponse = {
  groups: DedupeGroup[]
}

export type MergeRequest = {
  source_person_id: string
  target_person_id: string
}

export type MergeResult = {
  source_person_id: string
  target_person_id: string
  moved_links: number
  updated_links: number
  moved_relationships: number
  dropped_relationships: number
}

export type PersonRelationship = {
  id: string
  case_id: string
  from_person_id: string
  from_person_name: string
  to_person_id: string
  to_person_name: string
  rel_type: string
  rel_detail?: string | null
  created_at: string
  updated_at: string
}

export type RelationshipListResponse = {
  relationships: PersonRelationship[]
  total: number
}

export type CreateRelationshipRequest = {
  from_person_id: string
  to_person_id: string
  rel_type: string
  rel_detail?: string
}

// ---------- Search ----------

export type SearchResult = {
  object_type: string
  object_id: string
  case_id?: string | null
  case_name?: string | null
  title: string
  content?: string | null
  highlight?: string | null
  tags: string[]
  created_at: string
  score: number
  language_mode?: string | null
  file_id?: string | null
  file_name?: string | null
  chunk_id?: string | null
  chunk_index?: number | null
  display_label?: string | null
  anchor_json?: Record<string, unknown> | null
  matched_language?: string | null
  snippet_source?: string | null
  snippet_translated?: string | null
  translation_incomplete?: boolean | null
  source_fallback?: boolean | null
}

export type SearchResponse = {
  results: SearchResult[]
  total: number
  page: number
  page_size: number
  suggestions: string[]
}

export type SearchHistoryItem = {
  id: string
  keyword: string
  object_types?: string | null
  case_id?: string | null
  result_count?: number | null
  searched_at: string
}

// ---------- Exports ----------

export type ExportRecord = {
  id: string
  case_id: string
  export_type: string
  file_name: string
  storage_path: string
  file_size: number
  generated_by: string
  generated_at: string
}

export type ExportHistoryResponse = {
  items: ExportRecord[]
  total: number
  page: number
  page_size: number
}

// ---------- Logs ----------

export type OperationLog = {
  id: string
  user_id: string
  user_name: string
  case_id?: string | null
  action: string
  module: string
  target_type: string
  target_id?: string | null
  target_title?: string | null
  old_value?: Record<string, unknown> | null
  new_value?: Record<string, unknown> | null
  changed_fields?: string | null
  ip_address?: string | null
  created_at: string
}

export type LogListResponse = {
  logs: OperationLog[]
  total: number
  page: number
  page_size: number
}
