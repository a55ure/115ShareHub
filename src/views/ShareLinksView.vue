<script setup lang="ts">
import { ref, h, onMounted, onUnmounted } from 'vue'
import {
  NButton, NDataTable, NSpace, NTag, NModal, NForm, NFormItem, NInput,
  NPopconfirm, useMessage, NSpin, NAlert, NCard, NProgress,
} from 'naive-ui'
import type { DataTableColumns } from 'naive-ui'
import { useShareLinksStore } from '../stores/shareLinks'
import { formatFileSize, formatDate } from '../utils/format'
import { updateShareLink } from '../utils/tauri'
import type { ShareLink } from '../types'
import { listen } from '@tauri-apps/api/event'

const store = useShareLinksStore()
const message = useMessage()

const showModal = ref(false)
const showEditModal = ref(false)
const shareUrl = ref('')
const receiveCode = ref('')
const submitting = ref(false)

const editingId = ref(0)
const editTitle = ref('')
const editReceiveCode = ref('')

function extractPasswordFromUrl(url: string): string {
  try {
    const u = new URL(url.trim())
    const password = u.searchParams.get('password') || u.searchParams.get('receive_code') || u.searchParams.get('code')
    return password || ''
  } catch {
    const match = url.match(/[?&](?:password|receive_code|code)=([^&#]*)/i)
    return match ? decodeURIComponent(match[1]) : ''
  }
}

function onUrlInput(val: string) {
  shareUrl.value = val
  if (!receiveCode.value && val.includes('?')) {
    const code = extractPasswordFromUrl(val)
    if (code) receiveCode.value = code
  }
}

interface ParseProgress {
  share_link_id: number
  title: string
  current_path: string
  files_found: number
  dirs_found: number
  warn_message: string
}

const parseProgress = ref<ParseProgress | null>(null)

onMounted(async () => {
  await store.fetchLinks(1)

  const unlistenProgress = await listen('share-link-progress', (event: any) => {
    const p = event.payload
    store.updateLinkStatus(p.share_link_id, 'parsing')
    // Find the link title
    const link = store.links.find(l => l.id === p.share_link_id)
    parseProgress.value = {
      share_link_id: p.share_link_id,
      title: link?.title || link?.share_code || `Link #${p.share_link_id}`,
      current_path: p.current_path,
      files_found: p.files_found,
      dirs_found: p.dirs_found,
      warn_message: '',
    }
  })

  const unlistenCompleted = await listen('share-link-completed', (event: any) => {
    store.updateLinkStatus(event.payload.share_link_id, 'completed')
    store.fetchLinks(store.currentPage)
    message.success(`解析完成，共 ${event.payload.total_files} 个文件`)
    if (parseProgress.value?.share_link_id === event.payload.share_link_id) {
      parseProgress.value = null
    }
  })

  const unlistenWarn = await listen('share-link-warn', (event: any) => {
    if (parseProgress.value) {
      parseProgress.value.warn_message = event.payload.message
    }
    message.warning(event.payload.message, { duration: 8000 })
  })

  const unlistenError = await listen('share-link-error', (event: any) => {
    store.updateLinkStatus(event.payload.share_link_id, 'error')
    store.fetchLinks(store.currentPage)
    message.error(`解析失败: ${event.payload.error}`)
    if (parseProgress.value?.share_link_id === event.payload.share_link_id) {
      parseProgress.value = null
    }
  })

  onUnmounted(() => {
    unlistenProgress()
    unlistenCompleted()
    unlistenWarn()
    unlistenError()
  })
})

async function handleAdd() {
  if (!shareUrl.value.trim()) {
    message.warning('请输入分享链接')
    return
  }
  submitting.value = true
  try {
    await store.addLink(shareUrl.value.trim(), receiveCode.value.trim())
    message.success('分享链接已添加，开始解析...')
    showModal.value = false
    shareUrl.value = ''
    receiveCode.value = ''
  } catch (e: any) {
    message.error(`添加失败: ${e}`)
  } finally {
    submitting.value = false
  }
}

function openEdit(link: ShareLink) {
  editingId.value = link.id
  editTitle.value = link.title
  editReceiveCode.value = link.receive_code
  showEditModal.value = true
}

async function handleEdit() {
  try {
    await updateShareLink(editingId.value, editTitle.value, editReceiveCode.value)
    message.success('已更新')
    showEditModal.value = false
    store.fetchLinks(store.currentPage)
  } catch (e: any) {
    message.error(`更新失败: ${e}`)
  }
}

async function handleDelete(id: number) {
  try {
    await store.deleteLink(id)
    message.success('已删除')
  } catch (e: any) {
    message.error(`删除失败: ${e}`)
  }
}

async function handleRefresh(id: number) {
  try {
    await store.refreshLink(id)
    message.info('开始重新解析...')
  } catch (e: any) {
    message.error(`刷新失败: ${e}`)
  }
}

function statusTag(status: string) {
  const map: Record<string, { type: 'default' | 'info' | 'success' | 'warning' | 'error', label: string }> = {
    pending: { type: 'default', label: '等待中' },
    parsing: { type: 'info', label: '解析中' },
    completed: { type: 'success', label: '已完成' },
    error: { type: 'error', label: '失败' },
  }
  const s = map[status] || { type: 'default' as const, label: status }
  return h(NTag, { type: s.type, size: 'small' }, { default: () => s.label })
}

const columns: DataTableColumns<ShareLink> = [
  { title: '标题', key: 'title', ellipsis: { tooltip: true }, render: (row) => row.title || row.share_code },
  { title: '文件数', key: 'total_file_count', width: 90 },
  { title: '总大小', key: 'total_size', width: 110, render: (row) => formatFileSize(row.total_size) },
  { title: '状态', key: 'status', width: 90, render: (row) => statusTag(row.status) },
  { title: '添加时间', key: 'added_at', width: 160, render: (row) => formatDate(row.added_at) },
  {
    title: '操作', key: 'actions', width: 240,
    render: (row) =>
      h(NSpace, { size: 'small' }, {
        default: () => [
          h(NButton, { size: 'small', onClick: () => openEdit(row) }, { default: () => '编辑' }),
          h(NButton, { size: 'small', onClick: () => handleRefresh(row.id) }, { default: () => '刷新' }),
          h(NPopconfirm, { onPositiveClick: () => handleDelete(row.id) }, {
            trigger: () => h(NButton, { size: 'small', type: 'error' }, { default: () => '删除' }),
            default: () => '确定删除该分享链接及其所有文件记录？',
          }),
        ],
      }),
  },
]
</script>

<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
      <h2 style="margin: 0;">分享链接管理</h2>
      <NButton type="primary" @click="showModal = true">添加链接</NButton>
    </div>

    <!-- Parse Progress Panel -->
    <NCard v-if="parseProgress" title="解析进度" size="small" style="margin-bottom: 16px;"
      :bordered="true">
      <template #header-extra>
        <NTag type="info" size="small">解析中</NTag>
      </template>
      <Space vertical :size="8" style="width: 100%;">
        <div style="display: flex; justify-content: space-between;">
          <span style="font-weight: 600;">{{ parseProgress.title }}</span>
          <span style="color: #999; font-size: 13px;">
            {{ parseProgress.files_found }} 个文件 / {{ parseProgress.dirs_found }} 个目录
          </span>
        </div>
        <div style="color: #666; font-size: 13px; font-family: monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
          当前: {{ parseProgress.current_path || '/' }}
        </div>
        <NProgress type="line" :show-indicator="false" status="info" :height="6" />
        <div v-if="parseProgress.warn_message" style="color: #f0a020; font-size: 12px;">
          {{ parseProgress.warn_message }}
        </div>
      </Space>
    </NCard>

    <NSpin :show="store.loading">
      <NDataTable
        :columns="columns"
        :data="store.links"
        :pagination="{ page: store.currentPage, pageSize: store.pageSize, itemCount: store.total, onChange: (p: number) => store.fetchLinks(p) }"
        :bordered="false"
      />
    </NSpin>

    <NModal v-model:show="showModal" preset="dialog" title="添加115分享链接" positive-text="添加" negative-text="取消"
      :loading="submitting" @positive-click="handleAdd">
      <NAlert type="warning" :bordered="false" style="margin-bottom: 12px;">
        解析请求采用1~4秒随机间隔，大型分享可能需要较长时间，请耐心等待。
      </NAlert>
      <NForm>
        <NFormItem label="分享链接">
          <NInput :value="shareUrl" @update:value="onUrlInput" placeholder="https://115cdn.com/s/xxxxx 或分享码" />
        </NFormItem>
        <NFormItem label="提取码">
          <NInput v-model:value="receiveCode" placeholder="选填" />
        </NFormItem>
      </NForm>
    </NModal>

    <NModal v-model:show="showEditModal" preset="dialog" title="编辑分享链接" positive-text="保存" negative-text="取消"
      @positive-click="handleEdit">
      <NForm>
        <NFormItem label="标题">
          <NInput v-model:value="editTitle" placeholder="分享链接标题" />
        </NFormItem>
        <NFormItem label="提取码">
          <NInput v-model:value="editReceiveCode" placeholder="提取码" />
        </NFormItem>
      </NForm>
    </NModal>
  </div>
</template>
