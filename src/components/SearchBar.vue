<script setup lang="ts">
import { LoaderCircle, Search, X } from "@lucide/vue";

const props = defineProps<{ modelValue: string; resultCount: number; loading: boolean; searching?: boolean }>();
const emit = defineEmits<{ "update:modelValue": [value: string]; search: [] }>();

function submit() {
  if (!props.searching && props.modelValue.trim()) emit("search");
}
</script>

<template>
  <form class="flex items-stretch gap-2" role="search" @submit.prevent="submit">
    <div class="relative min-w-0 flex-1">
      <Search class="pointer-events-none absolute left-3.5 top-1/2 size-[18px] -translate-y-1/2 text-stone-400" :stroke-width="1.8" />
      <input
        :value="modelValue"
        class="h-11 w-full rounded-md border border-stone-300 bg-white pl-11 pr-12 text-sm text-stone-900 shadow-[0_1px_2px_rgba(28,25,23,0.03)] outline-none transition duration-150 placeholder:text-stone-400 hover:border-stone-400 focus:border-teal-600 focus:ring-2 focus:ring-teal-100"
        type="search"
        inputmode="search"
        placeholder="搜索技能名称、用途或描述"
        aria-label="搜索技能"
        @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
      />
      <span v-if="loading" class="absolute right-4 top-1/2 -translate-y-1/2 text-[11px] text-stone-400">加载中</span>
      <button v-else-if="modelValue" class="absolute right-2 top-1/2 flex size-8 -translate-y-1/2 items-center justify-center rounded-md text-stone-400 transition hover:bg-stone-100 hover:text-stone-700" type="button" title="清空搜索" aria-label="清空搜索" @click="emit('update:modelValue', '')"><X class="size-4" :stroke-width="2" /></button>
      <span v-else class="absolute right-4 top-1/2 -translate-y-1/2 text-[11px] text-stone-400">{{ resultCount }} 条</span>
    </div>
    <button class="button-primary h-11 px-5" type="submit" :disabled="searching || !modelValue.trim()"><LoaderCircle v-if="searching" class="size-4 animate-spin" /><Search v-else class="size-4" />{{ searching ? '搜索中' : '搜索' }}</button>
  </form>
</template>
