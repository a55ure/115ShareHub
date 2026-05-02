<script setup lang="ts">
import { NButton, NIcon, NSpin } from 'naive-ui'
import { ChevronForward, ChevronDown } from '@vicons/ionicons5'

interface FolderNode {
  cid: string
  name: string
  children: FolderNode[]
  expanded: boolean
  loading: boolean
  loaded: boolean
}

const props = defineProps<{
  node: FolderNode
  targetCid: string
  depth?: number
}>()

const emit = defineEmits<{
  expand: [node: FolderNode]
  select: [node: FolderNode]
}>()

const d = props.depth ?? 0
const isSelected = () => props.targetCid === props.node.cid
</script>

<template>
  <div>
    <div
      class="tree-row"
      :class="{ 'tree-row--selected': isSelected() }"
      :style="{ paddingLeft: d * 20 + 'px' }"
    >
      <span class="tree-arrow" @click="emit('expand', node)">
        <NSpin :size="14" v-if="node.loading" />
        <NIcon v-else :size="14">
          <ChevronDown v-if="node.expanded" />
          <ChevronForward v-else />
        </NIcon>
      </span>
      <span class="tree-icon">📁</span>
      <span class="tree-name">{{ node.name }}</span>
      <NButton size="tiny" @click="emit('select', node)"
        :type="isSelected() ? 'primary' : 'default'">
        {{ isSelected() ? '已选择' : '选择' }}
      </NButton>
    </div>

    <template v-if="node.expanded && node.children.length > 0">
      <FolderTreeNode
        v-for="child in node.children"
        :key="child.cid"
        :node="child"
        :target-cid="targetCid"
        :depth="d + 1"
        @expand="emit('expand', $event)"
        @select="emit('select', $event)"
      />
    </template>
    <div v-if="node.expanded && node.children.length === 0 && node.loaded"
      class="tree-empty" :style="{ paddingLeft: (d + 1) * 20 + 'px' }">
      <span class="tree-empty-text">空目录</span>
    </div>
  </div>
</template>

<style scoped>
.tree-row {
  display: flex;
  align-items: center;
  padding: 4px 8px;
  border-radius: 4px;
  transition: background 0.15s;
  cursor: default;
}
.tree-row:hover { background: #f5f7fa; }
.tree-row--selected { background: #e8f4ff; }
.tree-row--selected:hover { background: #d6ecff; }
.tree-arrow { flex-shrink: 0; cursor: pointer; width: 20px; display: inline-flex; align-items: center; justify-content: center; }
.tree-icon { margin-right: 4px; flex-shrink: 0; }
.tree-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tree-empty { padding: 2px 8px; }
.tree-empty-text { color: #ccc; font-size: 12px; }
</style>
