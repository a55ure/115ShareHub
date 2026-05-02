<script setup lang="ts">
import { ref, onMounted, computed, h } from 'vue'
import {
  NInput, NDataTable, NSpace, NSelect, NButton, NGrid, NGi, NFormItem,
  NInputGroup, NTag, NInputNumber, NCollapse, NCollapseItem, NSpin, NPagination, useMessage,
} from 'naive-ui'
import type { DataTableColumns } from 'naive-ui'
import { useSearchStore } from '../stores/search'
import { useShareLinksStore } from '../stores/shareLinks'
import { receiveShareFile, getLoginStatus } from '../utils/tauri'
import { formatFileSize, FILE_TYPE_OPTIONS, FILE_TYPE_COLOR } from '../utils/format'
import type { FileEntry } from '../types'

const searchStore = useSearchStore()
const linksStore = useShareLinksStore()
const message = useMessage()

const query = ref('')
const selectedFileType = ref<string | null>(null)
const sizeMin = ref<number | null>(null)
const sizeMax = ref<number | null>(null)
const selectedShareLinkId = ref<number | null>(null)
const sortBy = ref<string>('name')
const sortOrder = ref<string>('asc')
const loggedIn = ref(false)

const sortOptions = [
  { label: '名称', value: 'name' },
  { label: '大小', value: 'size' },
  { label: '日期', value: 'date' },
]

const orderOptions = [
  { label: '升序', value: 'asc' },
  { label: '降序', value: 'desc' },
]

const sizePresets = [
  { label: '< 100MB', min: null, max: 100 * 1024 * 1024 },
  { label: '100MB-1GB', min: 100 * 1024 * 1024, max: 1024 * 1024 * 1024 },
  { label: '1GB-10GB', min: 1024 * 1024 * 1024, max: 10 * 1024 * 1024 * 1024 },
  { label: '> 10GB', min: 10 * 1024 * 1024 * 1024, max: null },
]

onMounted(async () => {
  await linksStore.fetchLinks(1)
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

const linkOptions = computed(() => [
  { label: '全部来源', value: null as any },
  ...linksStore.links.map(l => ({ label: l.title || l.share_code, value: l.id as any })),
])

async function doSearch(page = 1) {
  try {
    await searchStore.doSearch({
      query: query.value || undefined,
      file_type: selectedFileType.value || undefined,
      size_min: sizeMin.value ?? undefined,
      size_max: sizeMax.value ?? undefined,
      share_link_id: selectedShareLinkId.value ?? undefined,
      is_dir: false,
      sort_by: sortBy.value as any,
      sort_order: sortOrder.value as any,
      page,
      page_size: searchStore.pageSize,
    })
  } catch (e: any) {
    message.error(`搜索失败: ${e}`)
  }
}

function handleSearchPageChange(p: number) {
  doSearch(p)
}

function handleSearchPageSizeChange(ps: number) {
  searchStore.pageSize = ps
  doSearch(1)
}

function applySizePreset(preset: typeof sizePresets[0]) {
  sizeMin.value = preset.min
  sizeMax.value = preset.max
}

const columns: DataTableColumns<FileEntry> = [
  {
    title: '', key: 'actions', width: 70,
    render: (row) => h(NButton, {
      size: 'tiny', type: 'primary', ghost: true,
      onClick: () => handleSaveToCloud(row),
      disabled: !loggedIn.value,
    }, { default: () => '转存' }),
  },
  {
    title: '文件名', key: 'name', ellipsis: { tooltip: true },
    render: (row) => h('span', { style: { fontWeight: row.is_dir ? 'bold' : 'normal' } }, row.name),
  },
  { title: '路径', key: 'full_path', ellipsis: { tooltip: true } },
  { title: '大小', key: 'size', width: 120, render: (row) => formatFileSize(row.size) },
  {
    title: '类型', key: 'file_type', width: 100,
    render: (row) => h(NTag, {
      size: 'small', round: true,
      color: { color: FILE_TYPE_COLOR[row.file_type] || '#95a5a6', textColor: '#fff' },
    }, { default: () => row.file_type }),
  },
  {
    title: 'SHA1', key: 'sha1', width: 160, ellipsis: { tooltip: true },
    render: (row) => row.sha1 ? h('span', {
      style: { cursor: 'pointer', fontSize: '12px', fontFamily: 'monospace' },
      onClick: () => { navigator.clipboard.writeText(row.sha1); message.success('已复制SHA1') },
    }, row.sha1) : '-',
  },
]
</script>

<template>
  <div>
    <h2 style="margin-top: 0;">搜索文件</h2>

    <NInputGroup style="margin-bottom: 16px;">
      <NInput v-model:value="query" placeholder="输入文件名关键词..." clearable size="large"
        @keydown.enter="doSearch(1)" />
      <NButton type="primary" size="large" @click="doSearch(1)">搜索</NButton>
    </NInputGroup>

    <NCollapse style="margin-bottom: 16px;">
      <NCollapseItem title="高级筛选" name="filters">
        <NGrid :x-gap="16" :y-gap="8" :cols="4">
          <NGi>
            <NFormItem label="文件类型">
              <NSelect v-model:value="selectedFileType" :options="FILE_TYPE_OPTIONS as any" clearable placeholder="全部类型" />
            </NFormItem>
          </NGi>
          <NGi>
            <NFormItem label="来源链接">
              <NSelect v-model:value="selectedShareLinkId" :options="linkOptions" clearable placeholder="全部来源" />
            </NFormItem>
          </NGi>
          <NGi>
            <NFormItem label="最小大小(bytes)">
              <NInputNumber v-model:value="sizeMin" clearable placeholder="0" style="width: 100%;" />
            </NFormItem>
          </NGi>
          <NGi>
            <NFormItem label="最大大小(bytes)">
              <NInputNumber v-model:value="sizeMax" clearable placeholder="不限" style="width: 100%;" />
            </NFormItem>
          </NGi>
        </NGrid>
        <NSpace style="margin-bottom: 8px;">
          <span style="line-height: 34px; color: #999; font-size: 13px;">快速大小:</span>
          <NButton v-for="p in sizePresets" :key="p.label" size="small" @click="applySizePreset(p)">{{ p.label }}</NButton>
        </NSpace>
        <NSpace>
          <NFormItem label="排序">
            <NSelect v-model:value="sortBy" :options="sortOptions" style="width: 120px;" />
          </NFormItem>
          <NFormItem label="顺序">
            <NSelect v-model:value="sortOrder" :options="orderOptions" style="width: 100px;" />
          </NFormItem>
        </NSpace>
      </NCollapseItem>
    </NCollapse>

    <NSpin :show="searchStore.loading">
      <NDataTable
        :columns="columns"
        :data="searchStore.results"
        :bordered="false"
      />
      <div style="display: flex; justify-content: flex-end; margin-top: 16px;">
        <NPagination
          :page="searchStore.currentPage"
          :page-size="searchStore.pageSize"
          :item-count="searchStore.totalCount"
          :page-sizes="[10, 20, 50, 100]"
          show-size-picker
          @update:page="handleSearchPageChange"
          @update:page-size="handleSearchPageSizeChange"
        />
      </div>
      <div v-if="searchStore.results.length === 0 && !searchStore.loading" style="text-align: center; color: #999; padding: 40px;">
        输入关键词开始搜索
      </div>
    </NSpin>
  </div>
</template>
