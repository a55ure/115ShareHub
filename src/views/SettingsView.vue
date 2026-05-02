<script setup lang="ts">
import { NSlider, NCard, NFormItem, NButton, NPopconfirm, useMessage } from 'naive-ui'
import { ref } from 'vue'

const message = useMessage()
const rateLimit = ref(2)

function clearDatabase() {
  message.info('清空数据库功能将在后续版本实现')
}
</script>

<template>
  <div>
    <h2 style="margin-top: 0;">设置</h2>

    <NCard title="API 设置" style="margin-bottom: 16px;">
      <NFormItem label="请求限速 (请求/秒)">
        <NSlider v-model:value="rateLimit" :min="1" :max="10" :step="1" :marks="{ 1: '1', 2: '2 (默认)', 5: '5', 10: '10' }" />
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
