import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { ShareLink, PaginatedResult } from '../types'
import { listShareLinks, addShareLink, removeShareLink, refreshShareLink } from '../utils/tauri'

export const useShareLinksStore = defineStore('shareLinks', () => {
  const links = ref<ShareLink[]>([])
  const total = ref(0)
  const loading = ref(false)
  const currentPage = ref(1)
  const pageSize = ref(20)

  async function fetchLinks(page?: number) {
    loading.value = true
    try {
      const p = page ?? currentPage.value
      const result: PaginatedResult<ShareLink> = await listShareLinks(p, pageSize.value)
      links.value = result.items
      total.value = result.total
      currentPage.value = p
    } finally {
      loading.value = false
    }
  }

  async function addLink(url: string, receiveCode: string) {
    const link = await addShareLink({ url, receive_code: receiveCode })
    links.value.unshift(link)
    total.value += 1
    return link
  }

  async function deleteLink(id: number) {
    await removeShareLink(id)
    links.value = links.value.filter(l => l.id !== id)
    total.value -= 1
  }

  async function refreshLink(id: number) {
    await refreshShareLink(id)
    const idx = links.value.findIndex(l => l.id === id)
    if (idx >= 0) {
      links.value[idx].status = 'pending'
    }
  }

  function updateLinkStatus(id: number, status: string) {
    const link = links.value.find(l => l.id === id)
    if (link) {
      link.status = status as ShareLink['status']
    }
  }

  return { links, total, loading, currentPage, pageSize, fetchLinks, addLink, deleteLink, refreshLink, updateLinkStatus }
})
