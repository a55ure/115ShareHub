import { invoke } from '@tauri-apps/api/core'
import type {
  AddShareLinkRequest,
  ShareLink,
  ShareLinkDetail,
  SearchParams,
  SearchResult,
  AppStats,
  PaginatedResult,
  FileEntry,
  LoginStatus,
  QrCodeResponse,
  PollResponse,
} from '../types'

export async function addShareLink(request: AddShareLinkRequest): Promise<ShareLink> {
  return invoke<ShareLink>('add_share_link', { request })
}

export async function removeShareLink(id: number): Promise<void> {
  return invoke('remove_share_link', { id })
}

export async function listShareLinks(page: number, pageSize: number): Promise<PaginatedResult<ShareLink>> {
  return invoke<PaginatedResult<ShareLink>>('list_share_links', { page, pageSize })
}

export async function refreshShareLink(id: number): Promise<void> {
  return invoke('refresh_share_link', { id })
}

export async function updateShareLink(id: number, title: string, receiveCode: string): Promise<void> {
  return invoke('update_share_link', { id, title, receiveCode })
}

export async function getShareLinkDetail(id: number): Promise<ShareLinkDetail> {
  return invoke<ShareLinkDetail>('get_share_link_detail', { id })
}

export async function searchFiles(params: SearchParams): Promise<SearchResult> {
  return invoke<SearchResult>('search_files', { params })
}

export async function getFileStats(): Promise<AppStats> {
  return invoke<AppStats>('get_file_stats')
}

export async function listFiles(params: {
  file_type?: string
  keyword?: string
  share_link_id?: number
  page?: number
  page_size?: number
}): Promise<PaginatedResult<FileEntry>> {
  return invoke<PaginatedResult<FileEntry>>('list_files', { params })
}

// Auth
export async function initQrcodeLogin(): Promise<QrCodeResponse> {
  return invoke<QrCodeResponse>('init_qrcode_login')
}

export async function pollQrcodeLogin(uid: string): Promise<PollResponse> {
  return invoke<PollResponse>('poll_qrcode_login', { uid })
}

export async function loginByCookie(cookie: string): Promise<LoginStatus> {
  return invoke<LoginStatus>('login_by_cookie', { request: { cookie } })
}

export async function getLoginStatus(): Promise<LoginStatus> {
  return invoke<LoginStatus>('get_login_status')
}

export async function logout(): Promise<void> {
  return invoke('logout')
}
