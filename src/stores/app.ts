import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { AppStats } from '../types'
import { getFileStats } from '../utils/tauri'

export const useAppStore = defineStore('app', () => {
  const stats = ref<AppStats | null>(null)
  const loading = ref(false)

  async function fetchStats() {
    loading.value = true
    try {
      stats.value = await getFileStats()
    } finally {
      loading.value = false
    }
  }

  return { stats, loading, fetchStats }
})
