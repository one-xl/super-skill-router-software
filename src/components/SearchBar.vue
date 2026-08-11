<script setup lang="ts">
import { Search, X } from "@lucide/vue";

defineProps<{ modelValue: string; resultCount: number; loading: boolean }>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
</script>

<template>
  <div class="relative">
    <Search class="pointer-events-none absolute left-4 top-1/2 size-5 -translate-y-1/2 text-slate-400" :stroke-width="1.8" />
    <input
      :value="modelValue"
      class="h-14 w-full rounded-xl border border-slate-200 bg-white pl-12 pr-24 text-base text-slate-900 shadow-sm outline-none transition placeholder:text-slate-400 focus:border-teal-500 focus:ring-4 focus:ring-teal-500/10"
      type="text"
      inputmode="search"
      placeholder="搜索技能名称、用途或描述"
      aria-label="搜索技能"
      @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
    />
    <span v-if="loading" class="absolute right-4 top-1/2 -translate-y-1/2 text-xs text-slate-400">加载中</span>
    <button
      v-else-if="modelValue"
      class="absolute right-3 top-1/2 flex size-8 -translate-y-1/2 items-center justify-center rounded-lg text-slate-400 transition hover:bg-slate-100 hover:text-slate-700"
      type="button"
      title="清空搜索"
      aria-label="清空搜索"
      @click="emit('update:modelValue', '')"
    >
      <X class="size-4" :stroke-width="2" />
    </button>
    <span v-else class="absolute right-4 top-1/2 -translate-y-1/2 text-xs text-slate-400">{{ resultCount }} 条</span>
  </div>
</template>
