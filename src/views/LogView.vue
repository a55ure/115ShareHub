<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue'
import { NCard, NButton, NTag, NSpace, NEmpty } from 'naive-ui'
import { listen } from '@tauri-apps/api/event'

interface LogEntry {
  id: number
  timestamp: string
  level: string
  message: string
  share_link_id: number
}

const logs = ref<LogEntry[]>([])
const maxLogs = 500
let logId = 0
const logContainer = ref<HTMLElement | null>(null)
const autoScroll = ref(true)

const levelTypeMap: Record<string, 'default' | 'info' | 'success' | 'warning' | 'error'> = {
  info: 'info',
  scan: 'default',
  progress: 'info',
  warn: 'warning',
  error: 'error',
  success: 'success',
}

const levelLabelMap: Record<string, string> = {
  info: 'INFO',
  scan: 'SCAN',
  progress: 'PROGRESS',
  warn: 'WARN',
  error: 'ERROR',
  success: 'DONE',
}

const unlisteners: (() => void)[] = []

onMounted(async () => {
  const unlistenLog = await listen('share-link-log', (event: any) => {
    const p = event.payload
    logs.value.push({
      id: ++logId,
      timestamp: p.timestamp,
      level: p.level,
      message: p.message,
      share_link_id: p.share_link_id,
    })
    if (logs.value.length > maxLogs) {
      logs.value = logs.value.slice(-maxLogs)
    }
    if (autoScroll.value) {
      nextTick(() => {
        if (logContainer.value) {
          logContainer.value.scrollTop = logContainer.value.scrollHeight
        }
      })
    }
  })

  const unlistenProgress = await listen('share-link-progress', (event: any) => {
    const p = event.payload
    logs.value.push({
      id: ++logId,
      timestamp: new Date().toISOString(),
      level: 'progress',
      message: `进度更新: ${p.current_path} — ${p.files_found} 文件, ${p.dirs_found} 目录`,
      share_link_id: p.share_link_id,
    })
    if (logs.value.length > maxLogs) {
      logs.value = logs.value.slice(-maxLogs)
    }
  })

  const unlistenWarn = await listen('share-link-warn', (event: any) => {
    logs.value.push({
      id: ++logId,
      timestamp: new Date().toISOString(),
      level: 'warn',
      message: event.payload.message,
      share_link_id: event.payload.share_link_id,
    })
  })

  const unlistenCompleted = await listen('share-link-completed', (event: any) => {
    logs.value.push({
      id: ++logId,
      timestamp: new Date().toISOString(),
      level: 'success',
      message: `解析完成: ${event.payload.total_files} 个文件`,
      share_link_id: event.payload.share_link_id,
    })
  })

  const unlistenError = await listen('share-link-error', (event: any) => {
    logs.value.push({
      id: ++logId,
      timestamp: new Date().toISOString(),
      level: 'error',
      message: `解析失败: ${event.payload.error}`,
      share_link_id: event.payload.share_link_id,
    })
  })

  unlisteners.push(unlistenLog, unlistenProgress, unlistenWarn, unlistenCompleted, unlistenError)
})

onUnmounted(() => {
  unlisteners.forEach(fn => fn())
  unlisteners.length = 0
})

function clearLogs() {
  logs.value = []
}

function formatTime(ts: string): string {
  try {
    const d = new Date(ts)
    return d.toLocaleTimeString('zh-CN', { hour12: false })
  } catch {
    return ts
  }
}
</script>

<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
      <h2 style="margin: 0;">解析日志</h2>
      <NSpace>
        <NButton size="small" @click="autoScroll = !autoScroll">
          {{ autoScroll ? '停止滚动' : '自动滚动' }}
        </NButton>
        <NButton size="small" @click="clearLogs">清空</NButton>
      </NSpace>
    </div>

    <NCard v-if="logs.length === 0" style="text-align: center; padding: 80px 0;">
      <NEmpty description="暂无日志，解析分享链接时会显示详细过程" />
    </NCard>

    <NCard v-else size="small" style="max-height: calc(100vh - 180px); overflow: hidden;">
      <div ref="logContainer" style="overflow-y: auto; max-height: calc(100vh - 220px); font-family: 'SF Mono', 'Fira Code', monospace; font-size: 12px; line-height: 1.8;">
        <div v-for="log in logs" :key="log.id"
          :style="{
            padding: '2px 8px',
            borderBottom: '1px solid #f0f0f0',
            display: 'flex',
            gap: '12px',
            alignItems: 'flex-start',
            backgroundColor: log.level === 'error' ? '#fff0f0' : log.level === 'warn' ? '#fffbe6' : 'transparent',
          }">
          <span style="color: #999; white-space: nowrap; min-width: 70px;">{{ formatTime(log.timestamp) }}</span>
          <span style="min-width: 80px;">
            <NTag :type="levelTypeMap[log.level] || 'default'" size="tiny" round>
              {{ levelLabelMap[log.level] || log.level }}
            </NTag>
          </span>
          <span style="flex: 1; word-break: break-all;">{{ log.message }}</span>
        </div>
      </div>
    </NCard>
  </div>
</template>
