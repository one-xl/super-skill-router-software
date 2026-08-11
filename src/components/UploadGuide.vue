<script setup lang="ts">
import { ref } from "vue";
import { FolderOpen, LoaderCircle, Settings2, Upload } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";

const props = defineProps<{ packages: Array<{ targetName: string; zipPath: string }> }>();
const opening = ref<string | null>(null);
const error = ref<string | null>(null);

async function reveal(zipPath: string) {
  opening.value = zipPath;
  error.value = null;
  try {
    await invoke("reveal_packaged_skill", { zipPath });
  } catch (cause) {
    error.value = typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "无法打开上传包所在目录。";
  } finally {
    opening.value = null;
  }
}
</script>

<template>
  <section class="mt-4 border border-amber-200 bg-amber-50 p-4 text-slate-800">
    <div class="flex items-center gap-2 text-sm font-semibold text-amber-950"><Upload class="size-4" />Claude Desktop：待上传</div>
    <div class="mt-3 grid gap-2 text-sm sm:grid-cols-3">
      <div class="flex items-center gap-2"><FolderOpen class="size-4 text-amber-700" /><span>打开 zip 所在目录</span></div>
      <div class="flex items-center gap-2"><Settings2 class="size-4 text-amber-700" /><span>Settings → Capabilities</span></div>
      <div class="flex items-center gap-2"><Upload class="size-4 text-amber-700" /><span>Skills → Upload zip</span></div>
    </div>
    <div class="mt-3 space-y-2">
      <div v-for="item in props.packages" :key="item.zipPath" class="flex flex-wrap items-center justify-between gap-2 border border-amber-200 bg-white px-3 py-2 text-xs">
        <span class="min-w-0 truncate text-slate-600">{{ item.zipPath }}</span>
        <button class="inline-flex h-8 shrink-0 items-center gap-2 border border-amber-300 bg-white px-2 text-xs font-medium text-amber-900 hover:bg-amber-100 disabled:cursor-not-allowed" type="button" :disabled="opening === item.zipPath" @click="reveal(item.zipPath)"><LoaderCircle v-if="opening === item.zipPath" class="size-3.5 animate-spin" /><FolderOpen v-else class="size-3.5" />打开目录</button>
      </div>
    </div>
    <p v-if="error" class="mt-3 text-xs text-rose-800">{{ error }}</p>
  </section>
</template>
