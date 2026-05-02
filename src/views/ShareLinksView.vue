<script setup lang="ts">
import { ref, h, onMounted, onUnmounted } from 'vue'
import {
  NButton, NDataTable, NSpace, NTag, NModal, NForm, NFormItem, NInput,
  NPopconfirm, useMessage, NSpin,
} from 'naive-ui'
import type { DataTableColumns } from 'naive-ui'
import { useShareLinksStore } from '../stores/shareLinks'
import { formatFileSize, formatDate } from '../utils/format'
import type { ShareLink } from '../types'
import { listen } from '@tauri-apps/api/event'

const store = useShareLinksStore()
const message = useMessage()

const showModal = ref(false)
const shareUrl = ref('')
const receiveCode = ref('')
const submitting = ref(false)

onMounted(async () => {
  await store.fetchLinks(1)

  const unlistenProgress = await listen('share-link-progress', (event: any) => {
    store.updateLinkStatus(event.payload.share_link_id, 'parsing')
  })
  const unlistenCompleted = await listen('share-link-completed', (event: any) => {
    store.updateLinkStatus(event.payload.share_link_id, 'completed')
    store.fetchLinks(store.currentPage)
    message.success(`分享链接解析完成，共 ${event.payload.total_files} 个文件`)
  })
  const unlistenError = await listen('share-link-error', (event: any) => {
    store.updateLinkStatus(event.payload.share_link_id, 'error')
    store.fetchLinks(store.currentPage)
    message.error(`解析失败: ${event.payload.error}`)
  })

  onUnmounted(() => {
    unlistenProgress()
    unlistenCompleted()
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
  { title: '文件数', key: 'total_file_count', width: 100 },
  { title: '总大小', key: 'total_size', width: 120, render: (row) => formatFileSize(row.total_size) },
  { title: '状态', key: 'status', width: 100, render: (row) => statusTag(row.status) },
  { title: '添加时间', key: 'added_at', width: 180, render: (row) => formatDate(row.added_at) },
  {
    title: '操作', key: 'actions', width: 180,
    render: (row) =>
      h(NSpace, { size: 'small' }, {
        default: () => [
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
      <NForm>
        <NFormItem label="分享链接">
          <NInput v-model:value="shareUrl" placeholder="https://115cdn.com/s/xxxxx 或分享码" />
        </NFormItem>
        <NFormItem label="提取码">
          <NInput v-model:value="receiveCode" placeholder="选填" />
        </NFormItem>
      </NForm>
    </NModal>
  </div>
</template>
