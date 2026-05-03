<script setup lang="ts">
import { ref, h, computed } from 'vue'
import {
  NCard, NGrid, NGi, NTag, NButton, NDataTable, NSpace, NInput, NSelect,
  NSpin, NPagination, NBreadcrumb, NBreadcrumbItem, NEmpty, useMessage,
} from 'naive-ui'
import type { DataTableColumns } from 'naive-ui'
import { useShareLinksStore } from '../stores/shareLinks'
import { formatFileSize, FILE_TYPE_COLOR, FILE_TYPE_LABEL } from '../utils/format'
import { browseShareDir, receiveShareFile, receiveShareFolder, getLoginStatus } from '../utils/tauri'
import type { FileEntry, PaginatedResult, ShareLink } from '../types'

const store = useShareLinksStore()
const message = useMessage()

// Navigation state
const selectedLink = ref<ShareLink | null>(null)
const currentParentId = ref('')
const breadcrumb = ref<{ id: string; name: string }[]>([])

// File list state
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
  { label: 'ISO原盘', value: 'iso' },
  { label: '字幕', value: 'subtitle' },
  { label: '文件夹', value: 'folder' },
  { label: '其他', value: 'other' },
]

// Load share links on mount
import { onMounted } from 'vue'
onMounted(async () => {
  await store.fetchLinks(1)
  try {
    const status = await getLoginStatus()
    loggedIn.value = status.logged_in
  } catch { /* ignore */ }
})

// Filter only completed links
const completedLinks = computed(() =>
  store.links.filter(l => l.status === 'completed')
)

async function openLink(link: ShareLink) {
  selectedLink.value = link
  currentParentId.value = ''
  breadcrumb.value = [{ id: '', name: link.title || link.share_code }]
  await fetchFiles('')
}

function backToList() {
  selectedLink.value = null
  files.value = []
  total.value = 0
}

async function navigateToDir(item: FileEntry) {
  currentParentId.value = item.file_id
  breadcrumb.value.push({ id: item.file_id, name: item.name })
  currentPage.value = 1
  await fetchFiles(item.file_id)
}

async function navigateToBreadcrumb(index: number) {
  const crumb = breadcrumb.value[index]
  breadcrumb.value = breadcrumb.value.slice(0, index + 1)
  currentParentId.value = crumb.id
  currentPage.value = 1
  await fetchFiles(crumb.id)
}

async function fetchFiles(parentId?: string, page?: number) {
  if (!selectedLink.value) return
  loading.value = true
  try {
    const p = page ?? currentPage.value
    const result: PaginatedResult<FileEntry> = await browseShareDir({
      share_link_id: selectedLink.value.id,
      parent_id: parentId ?? currentParentId.value,
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

async function handleSaveToCloud(row: FileEntry) {
  if (!loggedIn.value) {
    message.warning('请先在账号管理页面登录115账号')
    return
  }
  try {
    if (row.is_dir) {
      const msg = await receiveShareFolder(row.file_id, row.share_link_id)
      message.success(msg)
    } else {
      const msg = await receiveShareFile(row.file_id, row.share_link_id, row.parent_id || '0')
      message.success(msg)
    }
  } catch (e: any) {
    message.error(`转存失败: ${e}`)
  }
}

function handleFilter() {
  currentPage.value = 1
  fetchFiles()
}

function handlePageChange(p: number) {
  fetchFiles(undefined, p)
}

function handlePageSizeChange(ps: number) {
  pageSize.value = ps
  currentPage.value = 1
  fetchFiles()
}

const fileColumns: DataTableColumns<FileEntry> = [
  {
    title: '', key: 'actions', width: 70,
    render: (row) => h(NButton, {
      size: 'tiny', type: 'primary', ghost: true,
      onClick: () => handleSaveToCloud(row),
      disabled: !loggedIn.value,
    }, { default: () => row.is_dir ? '转存文件夹' : '转存' }),
  },
  {
    title: '名称', key: 'name', minWidth: 300,
    render: (row) => {
      if (row.is_dir) {
        return h('span', {
          style: { color: '#18a058', cursor: 'pointer', fontWeight: 500 },
          onClick: () => navigateToDir(row),
        }, '📁 ' + row.name)
      }
      return h('span', { style: { fontWeight: 400 } }, row.name)
    },
  },
  {
    title: '大小', key: 'size', width: 120, sorter: (a, b) => a.size - b.size,
    render: (row) => row.is_dir ? '-' : formatFileSize(row.size),
  },
  {
    title: '类型', key: 'file_type', width: 90,
    render: (row) => {
      const type = row.is_dir ? 'folder' : row.file_type
      return h(NTag, {
        size: 'small', round: true,
        color: { color: FILE_TYPE_COLOR[type] || '#95a5a6', textColor: '#fff' },
      }, { default: () => FILE_TYPE_LABEL[type] || type })
    },
  },
]
</script>

<template>
  <div>
    <!-- List view: show share links as cards -->
    <template v-if="!selectedLink">
      <h2 style="margin-top: 0; margin-bottom: 16px;">资源库</h2>

      <div v-if="completedLinks.length === 0" style="text-align: center; padding: 80px 0;">
        <NEmpty description="暂无已完成的分享链接，请先在分享链接页面添加并解析" />
      </div>

      <NGrid :x-gap="16" :y-gap="16" :cols="3" responsive="screen" v-else>
        <NGi v-for="link in completedLinks" :key="link.id">
          <NCard
            hoverable
            style="cursor: pointer; height: 100%;"
            @click="openLink(link)"
          >
            <template #header>
              <div style="display: flex; align-items: center; gap: 8px;">
                <span style="font-size: 20px;">🎬</span>
                <span style="font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                  {{ link.title || link.share_code }}
                </span>
              </div>
            </template>
            <NSpace vertical :size="8">
              <div style="display: flex; justify-content: space-between; align-items: center;">
                <NTag size="small" type="info">{{ link.total_file_count }} 个文件</NTag>
                <span style="color: #999; font-size: 13px;">{{ formatFileSize(link.total_size) }}</span>
              </div>
              <div v-if="link.share_user_name" style="color: #666; font-size: 12px;">
                分享者: {{ link.share_user_name }}
              </div>
            </NSpace>
          </NCard>
        </NGi>
      </NGrid>
    </template>

    <!-- Detail view: browse files inside a share link -->
    <template v-else>
      <div style="display: flex; align-items: center; gap: 12px; margin-bottom: 16px;">
        <NButton text @click="backToList" style="font-size: 18px;">← 返回</NButton>
        <NBreadcrumb>
          <NBreadcrumbItem
            v-for="(crumb, idx) in breadcrumb"
            :key="idx"
            :clickable="idx < breadcrumb.length - 1"
            @click="idx < breadcrumb.length - 1 && navigateToBreadcrumb(idx)"
          >
            {{ crumb.name }}
          </NBreadcrumbItem>
        </NBreadcrumb>
      </div>

      <NCard>
        <template #header>
          <span style="font-weight: 600;">{{ selectedLink.title || selectedLink.share_code }}</span>
        </template>
        <template #header-extra>
          <NSpace>
            <NSelect v-model:value="fileTypeFilter" :options="typeOptions" style="width: 120px;" @update:value="handleFilter" />
            <NInput v-model:value="keyword" placeholder="搜索文件名..." clearable style="width: 200px;"
              @keydown.enter="handleFilter" @clear="handleFilter" />
          </NSpace>
        </template>

        <NSpin :show="loading">
          <NDataTable
            :columns="fileColumns"
            :data="files"
            :bordered="false"
            :scroll-x="700"
            :row-key="(row: FileEntry) => row.id"
          />
          <div style="display: flex; justify-content: flex-end; margin-top: 16px;">
            <NPagination
              :page="currentPage"
              :page-size="pageSize"
              :item-count="total"
              :page-sizes="[20, 50, 100, 200]"
              show-size-picker
              @update:page="handlePageChange"
              @update:page-size="handlePageSizeChange"
            />
          </div>
          <div v-if="files.length === 0 && !loading" style="text-align: center; color: #999; padding: 40px;">
            此目录为空
          </div>
        </NSpin>
      </NCard>
    </template>
  </div>
</template>
