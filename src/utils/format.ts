export function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB']
  const k = 1024
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  const value = bytes / Math.pow(k, i)
  return `${value.toFixed(i > 0 ? 2 : 0)} ${units[i]}`
}

export function formatDate(dateStr: string | null): string {
  if (!dateStr) return '-'
  try {
    return new Date(dateStr + 'Z').toLocaleString('zh-CN')
  } catch {
    return dateStr
  }
}

export const FILE_TYPE_OPTIONS = [
  { label: '视频', value: 'video' },
  { label: '音频', value: 'audio' },
  { label: '图片', value: 'image' },
  { label: '文档', value: 'document' },
  { label: '压缩包', value: 'archive' },
  { label: '软件', value: 'software' },
  { label: '电子书', value: 'book' },
  { label: '其他', value: 'other' },
] as const

export const FILE_TYPE_COLOR: Record<string, string> = {
  video: '#e74c3c',
  audio: '#3498db',
  image: '#2ecc71',
  document: '#f39c12',
  archive: '#9b59b6',
  software: '#1abc9c',
  book: '#e67e22',
  other: '#95a5a6',
}
