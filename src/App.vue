<script setup lang="ts">
import { Database, Network } from "@lucide/vue";
import Discover from "./views/Discover.vue";
</script>

<template>
  <div class="min-h-screen bg-slate-50 text-slate-900">
    <header class="border-b border-slate-200 bg-white">
      <div class="mx-auto flex h-16 max-w-6xl items-center justify-between px-6 lg:px-10">
        <div class="flex items-center gap-3">
          <div class="flex size-8 items-center justify-center rounded-lg bg-teal-600 text-white"><Network class="size-4" :stroke-width="2" /></div>
          <span class="text-sm font-semibold text-slate-950">Super Skill Router</span>
        </div>
        <div class="inline-flex items-center gap-2 text-xs text-slate-500"><Database class="size-3.5 text-teal-600" />本地索引</div>
      </div>
    </header>
    <main><Discover /></main>
  </div>
</template>
