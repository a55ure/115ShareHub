import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { FileEntry, SearchParams } from '../types'
import { searchFiles } from '../utils/tauri'

export const useSearchStore = defineStore('search', () => {
  const results = ref<FileEntry[]>([])
  const totalCount = ref(0)
  const loading = ref(false)
  const currentPage = ref(1)
  const pageSize = ref(50)
  const currentParams = ref<SearchParams>({})

  async function doSearch(params: SearchParams) {
    loading.value = true
    currentParams.value = { ...params }
    try {
      const result = await searchFiles({
        ...params,
        page: params.page ?? currentPage.value,
        page_size: params.page_size ?? pageSize.value,
      })
      results.value = result.items
      totalCount.value = result.total_count
      currentPage.value = result.page
    } finally {
      loading.value = false
    }
  }

  async function changePage(page: number) {
    await doSearch({ ...currentParams.value, page, page_size: pageSize.value })
  }

  return { results, totalCount, loading, currentPage, pageSize, currentParams, doSearch, changePage }
})
