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

export type LoginPayload = {
  username: string
  password: string
  remember_me: boolean
}

type LoginResponseData = {
  user: UserInfo
  access_token: string
  refresh_token: string
  expires_in: number
}

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

type CaseListResponse = {
  cases: CaseSummary[]
  total: number
  page: number
  page_size: number
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

async function request<T>(
  path: string,
  init?: RequestInit,
  token?: string,
): Promise<T> {
  const headers = new Headers(init?.headers)

  if (!headers.has('Content-Type') && init?.body) {
    headers.set('Content-Type', 'application/json')
  }
  if (token) {
    headers.set('Authorization', `Bearer ${token}`)
  }

  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...init,
    headers,
  })
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

export async function fetchHealth(): Promise<HealthResponse> {
  return request<HealthResponse>('/health')
}

export async function login(payload: LoginPayload): Promise<Session> {
  const data = await request<LoginResponseData>('/auth/login', {
    method: 'POST',
    body: JSON.stringify(payload),
  })

  return {
    user: data.user,
    accessToken: data.access_token,
    refreshToken: data.refresh_token,
    expiresIn: data.expires_in,
  }
}

export async function fetchCases(token: string): Promise<CaseListResponse> {
  return request<CaseListResponse>('/cases?page=1&page_size=20', undefined, token)
}

export { API_BASE_URL }
