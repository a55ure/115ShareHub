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
  ProxyConfig,
  AppSettings,
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

export async function pollQrcodeLogin(token: string): Promise<PollResponse> {
  return invoke<PollResponse>('poll_qrcode_login', { token })
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

// Settings
export async function getProxyConfig(): Promise<ProxyConfig> {
  return invoke<ProxyConfig>('get_proxy_config')
}

export async function saveProxyConfig(config: ProxyConfig): Promise<void> {
  return invoke('save_proxy_config', { config })
}

export async function getProxyConfigs(): Promise<ProxyConfig[]> {
  return invoke<ProxyConfig[]>('get_proxy_configs')
}

export async function saveProxyConfigs(configs: ProxyConfig[]): Promise<void> {
  return invoke('save_proxy_configs', { configs })
}

export async function getAppSettings(): Promise<AppSettings> {
  return invoke<AppSettings>('get_app_settings')
}

export async function setAppSetting(key: string, value: string): Promise<void> {
  return invoke('set_app_setting', { key, value })
}

// Share receive
export async function receiveShareFile(fileId: string, shareLinkId: number, cid: string): Promise<string> {
  return invoke<string>('receive_share_file', { request: { file_id: fileId, share_link_id: shareLinkId, cid } })
}
