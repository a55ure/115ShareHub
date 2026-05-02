<script setup lang="ts">
import { ref, h, onMounted, computed } from 'vue'
import {
  NGrid, NGi, NStatistic, NCard, NSpin, NTag, NDataTable, NSpace, NSelect, NInput,
  NButton, useMessage,
} from 'naive-ui'
import type { DataTableColumns } from 'naive-ui'
import { useAppStore } from '../stores/app'
import { useShareLinksStore } from '../stores/shareLinks'
import { formatFileSize, FILE_TYPE_COLOR } from '../utils/format'
import { listFiles, receiveShareFile, getLoginStatus } from '../utils/tauri'
import type { FileEntry, PaginatedResult } from '../types'

const appStore = useAppStore()
const linksStore = useShareLinksStore()
const message = useMessage()

const files = ref<FileEntry[]>([])
const total = ref(0)
const loading = ref(false)
const currentPage = ref(1)
const pageSize = ref(50)
const fileTypeFilter = ref<string | null>(null)
const keyword = ref('')
const loggedIn = ref(false)

const typeOptions = [
  { label: '全部', value: '' },
  { label: '视频', value: 'video' },
  { label: '音频', value: 'audio' },
  { label: '图片', value: 'image' },
  { label: '文档', value: 'document' },
  { label: '压缩包', value: 'archive' },
  { label: '其他', value: 'other' },
]

async function fetchFiles(page?: number) {
  loading.value = true
  try {
    const p = page ?? currentPage.value
    const result: PaginatedResult<FileEntry> = await listFiles({
      file_type: fileTypeFilter.value || undefined,
      keyword: keyword.value || undefined,
      page: p,
      page_size: pageSize.value,
    })
    files.value = result.items
    total.value = result.total
    currentPage.value = p
  } catch (e: any) {
    console.error('Failed to fetch files:', e)
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  await Promise.all([
    appStore.fetchStats(),
    linksStore.fetchLinks(1),
    fetchFiles(1),
  ])
  try {
    const status = await getLoginStatus()
    loggedIn.value = status.logged_in
  } catch { /* ignore */ }
})

async function handleSaveToCloud(row: FileEntry) {
  if (!loggedIn.value) {
    message.warning('请先在账号管理页面登录115账号')
    return
  }
  try {
    const msg = await receiveShareFile(row.file_id, row.share_link_id, row.parent_id || '0')
    message.success(msg)
  } catch (e: any) {
    message.error(`转存失败: ${e}`)
  }
}

function handlePageChange(p: number) {
  fetchFiles(p)
}

function handleFilter() {
  fetchFiles(1)
}

const linkMap = computed(() => {
  const m: Record<number, string> = {}
  for (const l of linksStore.links) {
    m[l.id] = l.title || l.share_code
  }
  return m
})

const columns: DataTableColumns<FileEntry> = [
  {
    title: '', key: 'actions', width: 70,
    render: (row) => row.is_dir ? null : h(NButton, {
      size: 'tiny', type: 'primary', ghost: true,
      onClick: () => handleSaveToCloud(row),
      disabled: !loggedIn.value,
    }, { default: () => '转存' }),
  },
  {
    title: '文件名', key: 'name', minWidth: 300,
    render: (row) => h('span', { style: { fontWeight: 500 } }, row.name),
  },
  {
    title: '路径', key: 'full_path', minWidth: 200,
    render: (row) => {
      const parts = row.full_path.split('/')
      if (parts.length <= 1) return row.full_path || '-'
      const parent = parts.slice(0, -1).join(' / ')
      return h('span', { style: { color: '#999', fontSize: '12px' } }, parent || '-')
    },
  },
  {
    title: '大小', key: 'size', width: 120, sorter: (a, b) => a.size - b.size,
    render: (row) => formatFileSize(row.size),
  },
  {
    title: '类型', key: 'file_type', width: 90,
    render: (row) => h(NTag, {
      size: 'small', round: true,
      color: { color: FILE_TYPE_COLOR[row.file_type] || '#95a5a6', textColor: '#fff' },
    }, { default: () => row.file_type }),
  },
  {
    title: '来源', key: 'share_link_id', width: 150, ellipsis: { tooltip: true },
    render: (row) => linkMap.value[row.share_link_id] || '-',
  },
]
</script>

<template>
  <div>
    <h2 style="margin-top: 0;">仪表盘</h2>

    <NGrid :x-gap="16" :y-gap="16" :cols="4" v-if="appStore.stats" style="margin-bottom: 20px;">
      <NGi>
        <NCard size="small">
          <NStatistic label="分享链接" :value="appStore.stats.total_share_links" />
        </NCard>
      </NGi>
      <NGi>
        <NCard size="small">
          <NStatistic label="文件总数" :value="appStore.stats.total_files" />
        </NCard>
      </NGi>
      <NGi>
        <NCard size="small">
          <NStatistic label="总大小" :value="formatFileSize(appStore.stats.total_size)" />
        </NCard>
      </NGi>
      <NGi>
        <NCard size="small">
          <NStatistic label="解析中" :value="appStore.stats.parsing_count" />
        </NCard>
      </NGi>
    </NGrid>

    <NCard title="文件列表">
      <template #header-extra>
        <NSpace>
          <NSelect v-model:value="fileTypeFilter" :options="typeOptions" style="width: 120px;" @update:value="handleFilter" />
          <NInput v-model:value="keyword" placeholder="搜索文件名..." clearable style="width: 200px;"
            @keydown.enter="handleFilter" @clear="handleFilter" />
        </NSpace>
      </template>

      <NSpin :show="loading">
        <NDataTable
          :columns="columns"
          :data="files"
          :pagination="{
            page: currentPage,
            pageSize: pageSize,
            itemCount: total,
            onChange: handlePageChange,
          }"
          :bordered="false"
          :scroll-x="900"
        />
        <div v-if="files.length === 0 && !loading" style="text-align: center; color: #999; padding: 40px;">
          暂无文件数据，请先添加分享链接
        </div>
      </NSpin>
    </NCard>
  </div>
</template>
