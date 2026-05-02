export interface ShareLink {
  id: number
  share_code: string
  receive_code: string
  title: string
  share_user_id: string
  share_user_name: string
  total_file_count: number
  total_size: number
  status: 'pending' | 'parsing' | 'completed' | 'error'
  error_message: string | null
  last_parsed_at: string | null
  added_at: string
}

export interface AddShareLinkRequest {
  url: string
  receive_code: string
}

export interface FileEntry {
  id: number
  share_link_id: number
  file_id: string
  parent_id: string
  name: string
  size: number
  sha1: string
  is_dir: boolean
  file_type: string
  full_path: string
  depth: number
  thumbnail_url: string
}

export interface SearchParams {
  query?: string
  file_type?: string
  size_min?: number
  size_max?: number
  share_link_id?: number
  is_dir?: boolean
  sort_by?: 'name' | 'size' | 'date' | 'relevance'
  sort_order?: 'asc' | 'desc'
  page?: number
  page_size?: number
}

export interface SearchResult {
  items: FileEntry[]
  total_count: number
  page: number
  page_size: number
}

export interface AppStats {
  total_share_links: number
  total_files: number
  total_size: number
  files_by_type: Record<string, number>
  parsing_count: number
  completed_count: number
  error_count: number
}

export interface PaginatedResult<T> {
  items: T[]
  total: number
  page: number
  page_size: number
}

export interface ShareLinkDetail {
  share_link: ShareLink
  files_by_type: Record<string, number>
  top_level_dirs: string[]
}

export interface LoginStatus {
  logged_in: boolean
  user_name: string
  user_id: string
  face: string
  login_time: string | null
}

export interface QrCodeResponse {
  token: string
  qr_url: string
}

export interface PollResponse {
  status: number
  logged_in: boolean
}

export interface ProxyConfig {
  enabled: boolean
  proxyType: 'http' | 'https' | 'socks5'
  host: string
  port: number
  username?: string
  password?: string
}

export interface AppSettings {
  rate_limit_rps: number
  page_size: number
  theme: string
  language: string
  proxy: ProxyConfig
}
