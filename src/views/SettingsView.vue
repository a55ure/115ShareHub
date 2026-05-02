<script setup lang="ts">
import {
  NSlider,
  NCard,
  NFormItem,
  NButton,
  NPopconfirm,
  NSwitch,
  NSelect,
  NInput,
  NInputNumber,
  NSpace,
  NDivider,
  useMessage,
} from 'naive-ui'
import { ref, onMounted } from 'vue'
import type { ProxyConfig } from '../types'
import { getProxyConfigs, saveProxyConfigs, setAppSetting } from '../utils/tauri'

const message = useMessage()
const rateLimit = ref(2)

let rateSaveTimer: ReturnType<typeof setTimeout> | null = null
function handleRateLimitChange(value: number) {
  rateLimit.value = value
  if (rateSaveTimer) clearTimeout(rateSaveTimer)
  rateSaveTimer = setTimeout(() => {
    setAppSetting('rate_limit_rps', String(value)).catch(console.error)
  }, 500)
}

function emptyProxy(): ProxyConfig {
  return { enabled: true, proxyType: 'http', host: '', port: 1080, username: '', password: '' }
}

const proxyConfigs = ref<ProxyConfig[]>([emptyProxy()])

const proxyTypeOptions = [
  { label: 'HTTP', value: 'http' },
  { label: 'HTTPS', value: 'https' },
  { label: 'SOCKS5', value: 'socks5' },
]

const saving = ref(false)

onMounted(async () => {
  try {
    const configs = await getProxyConfigs()
    if (configs.length > 0) {
      proxyConfigs.value = configs.map(c => ({
        enabled: c.enabled ?? true,
        proxyType: c.proxyType || 'http',
        host: c.host || '',
        port: c.port || 1080,
        username: c.username || '',
        password: c.password || '',
      }))
    }
  } catch (e) {
    console.error('Failed to load proxy config:', e)
  }
})

function addProxy() {
  proxyConfigs.value.push(emptyProxy())
}

function removeProxy(index: number) {
  proxyConfigs.value.splice(index, 1)
}

async function handleSaveProxy() {
  saving.value = true
  try {
    await saveProxyConfigs(proxyConfigs.value)
    message.success(`已保存 ${proxyConfigs.value.length} 个代理配置`)
  } catch (e) {
    message.error('保存失败: ' + (e as Error).message)
  } finally {
    saving.value = false
  }
}

function clearDatabase() {
  message.info('清空数据库功能将在后续版本实现')
}
</script>

<template>
  <div>
    <h2 style="margin-top: 0;">设置</h2>

    <NCard title="代理设置" style="margin-bottom: 16px;">
      <p style="color: #999; margin-bottom: 16px;">
        配置多个代理后，当某个代理被115拦截时会自动切换到下一个。至少保留一个直连作为兜底。
      </p>

      <div v-for="(proxy, idx) in proxyConfigs" :key="idx">
        <NDivider v-if="idx > 0">{{ `代理 #${idx + 1}` }}</NDivider>

        <NFormItem label="启用">
          <NSwitch v-model:value="proxy.enabled" />
        </NFormItem>

        <template v-if="proxy.enabled">
          <NFormItem label="代理类型">
            <NSelect
              v-model:value="proxy.proxyType"
              :options="proxyTypeOptions"
              style="width: 200px;"
            />
          </NFormItem>

          <NFormItem label="主机地址">
            <NInput
              v-model:value="proxy.host"
              placeholder="例如: 127.0.0.1"
              style="width: 300px;"
            />
          </NFormItem>

          <NFormItem label="端口">
            <NInputNumber
              v-model:value="proxy.port"
              :min="1"
              :max="65535"
              style="width: 200px;"
            />
          </NFormItem>

          <NFormItem label="用户名 (可选)">
            <NInput
              v-model:value="proxy.username"
              placeholder="留空则不需要认证"
              style="width: 300px;"
            />
          </NFormItem>

          <NFormItem label="密码 (可选)">
            <NInput
              v-model:value="proxy.password"
              type="password"
              show-password-on="click"
              placeholder="留空则不需要认证"
              style="width: 300px;"
            />
          </NFormItem>

          <NFormItem v-if="proxyConfigs.length > 1">
            <NButton size="small" type="error" @click="removeProxy(idx)">
              删除此代理
            </NButton>
          </NFormItem>
        </template>
      </div>

      <NFormItem>
        <NSpace>
          <NButton type="primary" :loading="saving" @click="handleSaveProxy">
            保存代理设置
          </NButton>
          <NButton @click="addProxy">添加代理</NButton>
        </NSpace>
      </NFormItem>
    </NCard>

    <NCard title="API 设置" style="margin-bottom: 16px;">
      <NFormItem label="请求限速 (请求/秒)">
        <NSlider
          v-model:value="rateLimit"
          :min="1"
          :max="10"
          :step="1"
          :marks="{ 1: '1', 2: '2 (默认)', 5: '5', 10: '10' }"
          @update:value="handleRateLimitChange"
        />
      </NFormItem>
    </NCard>

    <NCard title="数据管理">
      <NPopconfirm @positive-click="clearDatabase">
        <template #trigger>
          <NButton type="error">清空数据库</NButton>
        </template>
        确定清空所有分享链接和文件记录？此操作不可撤销。
      </NPopconfirm>
    </NCard>
  </div>
</template>
