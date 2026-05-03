<script setup lang="ts">
import { h, ref, computed } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { NLayout, NLayoutSider, NMenu, NIcon, NLayoutContent, NConfigProvider, NMessageProvider, zhCN } from 'naive-ui'
import {
  HomeOutline,
  FilmOutline,
  LinkOutline,
  SearchOutline,
  PersonCircleOutline,
  SettingsOutline,
} from '@vicons/ionicons5'
import type { MenuOption } from 'naive-ui'

const router = useRouter()
const route = useRoute()
const collapsed = ref(false)

function renderIcon(icon: any) {
  return () => h(NIcon, null, { default: () => h(icon) })
}

const menuOptions: MenuOption[] = [
  { label: '仪表盘', key: 'dashboard', icon: renderIcon(HomeOutline) },
  { label: '资源库', key: 'library', icon: renderIcon(FilmOutline) },
  { label: '分享链接', key: 'links', icon: renderIcon(LinkOutline) },
  { label: '搜索', key: 'search', icon: renderIcon(SearchOutline) },
  { label: '账号管理', key: 'account', icon: renderIcon(PersonCircleOutline) },
  { label: '设置', key: 'settings', icon: renderIcon(SettingsOutline) },
]

const activeKey = computed(() => route.name as string)

function handleMenuUpdate(key: string) {
  router.push({ name: key })
}
</script>

<template>
  <NConfigProvider :locale="zhCN">
    <NLayout has-sider style="height: 100vh">
      <NLayoutSider
        bordered
        collapse-mode="width"
        :collapsed-width="64"
        :width="200"
        :collapsed="collapsed"
        show-trigger
        @collapse="collapsed = true"
        @expand="collapsed = false"
      >
        <div style="padding: 16px; font-weight: bold; font-size: 16px; text-align: center; white-space: nowrap; overflow: hidden;">
          {{ collapsed ? '115' : '115资源库' }}
        </div>
        <NMenu
          :collapsed="collapsed"
          :collapsed-width="64"
          :collapsed-icon-size="22"
          :options="menuOptions"
          :value="activeKey"
          @update:value="handleMenuUpdate"
        />
      </NLayoutSider>
      <NLayoutContent content-style="padding: 24px;">
        <NMessageProvider>
          <router-view />
        </NMessageProvider>
      </NLayoutContent>
    </NLayout>
  </NConfigProvider>
</template>
