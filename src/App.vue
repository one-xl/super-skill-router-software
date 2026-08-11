<script setup lang="ts">
import { Database, LayoutList, Network, Settings, Wand2 } from "@lucide/vue";
import { ref } from "vue";
import Discover from "./views/Discover.vue";
import Compose from "./views/Compose.vue";
import Manage from "./views/Manage.vue";
import SettingsView from "./views/Settings.vue";

const view = ref<"discover" | "compose" | "manage" | "settings">("discover");
</script>

<template>
  <div class="min-h-screen bg-slate-50 text-slate-900">
    <header class="border-b border-slate-200 bg-white">
      <div class="mx-auto flex h-16 max-w-6xl items-center justify-between px-6 lg:px-10">
        <div class="flex items-center gap-3">
          <div class="flex size-8 items-center justify-center rounded-lg bg-teal-600 text-white"><Network class="size-4" :stroke-width="2" /></div>
          <span class="text-sm font-semibold text-slate-950">Super Skill Router</span>
        </div>
        <div class="flex items-center gap-4"><nav class="flex items-center gap-1 text-sm"><button type="button" class="h-8 px-2 text-slate-500 hover:text-teal-700" :class="view === 'discover' && 'font-semibold text-teal-700'" @click="view = 'discover'">发现</button><button type="button" class="inline-flex h-8 items-center gap-1 px-2 text-slate-500 hover:text-teal-700" :class="view === 'compose' && 'font-semibold text-teal-700'" @click="view = 'compose'"><Wand2 class="size-3.5" />转换器</button><button type="button" class="inline-flex h-8 items-center gap-1 px-2 text-slate-500 hover:text-teal-700" :class="view === 'manage' && 'font-semibold text-teal-700'" @click="view = 'manage'"><LayoutList class="size-3.5" />管理</button><button type="button" class="flex size-8 items-center justify-center text-slate-500 hover:text-teal-700" :class="view === 'settings' && 'text-teal-700'" title="设置" @click="view = 'settings'"><Settings class="size-4" /></button></nav><div class="inline-flex items-center gap-2 text-xs text-slate-500"><Database class="size-3.5 text-teal-600" />本地索引</div></div>
      </div>
    </header>
    <main><Discover v-if="view === 'discover'" /><Compose v-else-if="view === 'compose'" /><Manage v-else-if="view === 'manage'" /><SettingsView v-else /></main>
  </div>
</template>
