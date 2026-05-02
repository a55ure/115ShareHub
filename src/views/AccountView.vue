<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import {
  NCard, NTabs, NTabPane, NButton, NInput, NForm, NFormItem, NSpace,
  NAvatar, NDescriptions, NDescriptionsItem, NPopconfirm, NAlert,
  NSpin, NIcon, useMessage,
} from 'naive-ui'
import { ChevronRight, ChevronDown } from '@vicons/ionicons5'
import QrcodeVue from 'qrcode.vue'
import type { LoginStatus } from '../types'
import {
  initQrcodeLogin, pollQrcodeLogin, loginByCookie, getLoginStatus, logout,
  fetchUserDirectoryTree, getReceiveTarget, setReceiveTarget,
} from '../utils/tauri'

const message = useMessage()
const loginStatus = ref<LoginStatus | null>(null)
const loading = ref(true)

const qrUrl = ref('')
const qrToken = ref('')
const qrLoading = ref(false)
const qrPolling = ref(false)
const qrStatusText = ref('')
let pollTimer: ReturnType<typeof setInterval> | null = null

const cookieInput = ref('')
const cookieLoading = ref(false)

interface FolderNode {
  cid: string
  name: string
  children: FolderNode[]
  expanded: boolean
  loading: boolean
  loaded: boolean
  depth: number
}

const rootFolder = ref<FolderNode>({ cid: '0', name: '根目录', children: [], expanded: false, loading: false, loaded: false, depth: 0 })
const targetCid = ref('0')
const targetName = ref('根目录')

const loggedIn = computed(() => loginStatus.value?.logged_in ?? false)

// Flat list of visible nodes (non-recursive)
const flatNodes = computed(() => {
  const result: FolderNode[] = []
  function walk(node: FolderNode) {
    result.push(node)
    if (node.expanded) {
      for (const child of node.children) {
        walk(child)
      }
    }
  }
  walk(rootFolder.value)
  return result
})

onMounted(async () => {
  try {
    loginStatus.value = await getLoginStatus()
    if (loginStatus.value?.logged_in) {
      await expandNode(rootFolder.value)
      await loadTarget()
    }
  } catch (e: any) {
    message.error(`获取登录状态失败: ${e}`)
  } finally {
    loading.value = false
  }
})

async function expandNode(node: FolderNode) {
  if (node.loaded) {
    node.expanded = !node.expanded
    return
  }
  node.loading = true
  try {
    const items = await fetchUserDirectoryTree(node.cid)
    node.children = items.map(f => ({
      cid: f.cid, name: f.name, children: [],
      expanded: false, loading: false, loaded: false, depth: node.depth + 1,
    }))
    node.loaded = true
    node.expanded = true
  } catch (e: any) {
    message.error(`获取目录失败: ${e}`)
  } finally {
    node.loading = false
  }
}

async function loadTarget() {
  try {
    const tid = await getReceiveTarget()
    if (tid && tid !== '0') {
      targetCid.value = tid
      targetName.value = tid
    }
  } catch { /* ignore */ }
}

async function selectTarget(node: FolderNode) {
  targetCid.value = node.cid
  targetName.value = node.name
  try {
    await setReceiveTarget(node.cid, node.name)
    message.success(`转存目标已设为: ${node.name}`)
  } catch (e: any) {
    message.error(`保存失败: ${e}`)
  }
}

onUnmounted(() => stopPolling())

async function handleLogout() {
  try {
    await logout()
    loginStatus.value = { logged_in: false, user_name: '', user_id: '', face: '', login_time: null }
    message.success('已退出登录')
  } catch (e: any) { message.error(`退出失败: ${e}`) }
}

async function startQrLogin() {
  qrLoading.value = true; qrStatusText.value = ''; stopPolling()
  try {
    const resp = await initQrcodeLogin()
    qrToken.value = resp.token; qrUrl.value = resp.qr_url
    qrStatusText.value = '请使用115网盘APP扫描二维码'
    startPolling()
  } catch (e: any) { message.error(`获取二维码失败: ${e}`) }
  finally { qrLoading.value = false }
}

function startPolling() {
  stopPolling(); qrPolling.value = true
  pollTimer = setInterval(async () => {
    try {
      const result = await pollQrcodeLogin(qrToken.value)
      if (result.status === 0) qrStatusText.value = '等待扫码...'
      else if (result.status === 1) qrStatusText.value = '已扫码，请在手机上确认登录'
      else if (result.status === 2 && result.logged_in) {
        stopPolling(); loginStatus.value = await getLoginStatus(); message.success('登录成功')
      } else if (result.status === -1) {
        stopPolling(); qrStatusText.value = '二维码已过期，请重新获取'; message.warning('二维码已过期')
      }
    } catch (e: any) { stopPolling(); qrStatusText.value = `轮询出错: ${e}` }
  }, 2000)
}

function stopPolling() {
  if (pollTimer) { clearInterval(pollTimer); pollTimer = null }
  qrPolling.value = false
}

async function handleCookieLogin() {
  if (!cookieInput.value.trim()) { message.warning('请输入Cookie'); return }
  cookieLoading.value = true
  try {
    loginStatus.value = await loginByCookie(cookieInput.value.trim())
    message.success('Cookie登录成功'); cookieInput.value = ''
  } catch (e: any) { message.error(`登录失败: ${e}`) }
  finally { cookieLoading.value = false }
}
</script>

<template>
  <div>
    <h2 style="margin-top: 0;">账号管理</h2>
    <NSpin :show="loading">
      <NCard v-if="loggedIn && loginStatus" title="当前账号">
        <NSpace vertical align="center" :size="16">
          <NAvatar :size="64" :src="loginStatus.face || undefined" round>
            {{ loginStatus.user_name?.charAt(0) || '?' }}
          </NAvatar>
          <NDescriptions label-placement="left" :column="1" bordered>
            <NDescriptionsItem label="用户名">{{ loginStatus.user_name }}</NDescriptionsItem>
            <NDescriptionsItem label="用户ID">{{ loginStatus.user_id }}</NDescriptionsItem>
            <NDescriptionsItem label="登录时间">{{ loginStatus.login_time || '未知' }}</NDescriptionsItem>
          </NDescriptions>
          <NPopconfirm @positive-click="handleLogout">
            <template #trigger><NButton type="error">退出登录</NButton></template>
            确定退出115账号登录？退出后部分功能可能不可用。
          </NPopconfirm>
        </NSpace>
      </NCard>

      <NCard v-if="loggedIn" title="转存目标目录" style="margin-top: 16px;">
        <NAlert v-if="targetCid !== '0'" type="success" :bordered="false" style="margin-bottom: 12px;">
          当前转存目标: {{ targetName }}
        </NAlert>
        <div v-if="targetCid === '0'" style="color: #999; margin-bottom: 12px;">点击下方目录旁的「选择」设置转存目标</div>

        <div v-for="node in flatNodes" :key="node.cid"
          class="tree-row" :class="{ 'tree-row--sel': targetCid === node.cid }"
          :style="{ paddingLeft: node.depth * 20 + 'px' }">
          <span class="tree-arrow" @click="expandNode(node)">
            <NSpin :size="14" v-if="node.loading" />
            <NIcon v-else :size="14">
              <ChevronDown v-if="node.expanded" />
              <ChevronRight v-else />
            </NIcon>
          </span>
          <span class="tree-icon">📁</span>
          <span class="tree-name">{{ node.name }}</span>
          <NButton size="tiny" @click="selectTarget(node)"
            :type="targetCid === node.cid ? 'primary' : 'default'">
            {{ targetCid === node.cid ? '已选择' : '选择' }}
          </NButton>
        </div>
      </NCard>

      <template v-else-if="!loading">
        <NAlert type="info" :bordered="false" style="margin-bottom: 16px;">
          登录115网盘账号后可使用文件转存等高级功能。请选择扫码登录或手动输入Cookie。
        </NAlert>
        <NTabs type="card">
          <NTabPane name="qrcode" tab="扫码登录">
            <NSpace vertical align="center" :size="16" style="padding: 24px 0;">
              <div v-if="qrUrl" style="text-align: center;">
                <div style="display: inline-block; padding: 16px; background: white; border-radius: 8px; border: 1px solid #e0e0e0;">
                  <QrcodeVue :value="qrUrl" :size="200" level="M" />
                </div>
                <p style="margin-top: 12px; color: #666;">{{ qrStatusText }}</p>
              </div>
              <div v-else style="padding: 40px 0; text-align: center; color: #999;">点击下方按钮获取登录二维码</div>
              <NSpace>
                <NButton type="primary" :loading="qrLoading" @click="startQrLogin">
                  {{ qrPolling ? '重新获取' : '获取二维码' }}
                </NButton>
                <NButton v-if="qrPolling" @click="stopPolling">停止轮询</NButton>
              </NSpace>
            </NSpace>
          </NTabPane>
          <NTabPane name="cookie" tab="Cookie登录">
            <NForm style="padding: 24px 0; max-width: 600px;">
              <NFormItem label="Cookie">
                <NInput v-model:value="cookieInput" type="textarea"
                  placeholder="从浏览器开发者工具中复制115的Cookie粘贴到此处" :rows="4" />
              </NFormItem>
              <NFormItem label="获取方法">
                <NAlert type="info" :bordered="false">
                  1. 在浏览器中登录 115.com<br/>
                  2. 按 F12 打开开发者工具 → Network 标签<br/>
                  3. 刷新页面，点击任意请求 → Headers → Cookie<br/>
                  4. 复制完整 Cookie 字符串粘贴到上方输入框
                </NAlert>
              </NFormItem>
              <NButton type="primary" :loading="cookieLoading" @click="handleCookieLogin">验证并登录</NButton>
            </NForm>
          </NTabPane>
        </NTabs>
      </template>
    </NSpin>
  </div>
</template>

<style scoped>
.tree-row {
  display: flex; align-items: center; padding: 4px 8px; border-radius: 4px; transition: background 0.15s;
}
.tree-row:hover { background: #f5f7fa; }
.tree-row--sel { background: #e8f4ff; }
.tree-row--sel:hover { background: #d6ecff; }
.tree-arrow { flex-shrink: 0; cursor: pointer; width: 20px; display: inline-flex; align-items: center; justify-content: center; }
.tree-icon { margin-right: 4px; flex-shrink: 0; }
.tree-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
