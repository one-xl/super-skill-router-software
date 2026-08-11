<script lang="ts">
const translationCache = new Map<string, string>();
</script>

<script setup lang="ts">
import { ref } from "vue";
import { ChevronUp, Languages, LoaderCircle } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import MarkdownPreview from "./MarkdownPreview.vue";

const props = defineProps<{ directoryName: string; name: string; content: string }>();
const emit = defineEmits<{ close: [] }>();
const translatedContent = ref<string | null>(translationCache.get(cacheKey()) ?? null);
const showTranslated = ref(false);
const translating = ref(false);
const error = ref<string | null>(null);

function cacheKey() {
  let hash = 0;
  for (let index = 0; index < props.content.length; index += 1) hash = ((hash << 5) - hash + props.content.charCodeAt(index)) | 0;
  return `${props.directoryName}:${hash}`;
}

function fail(cause: unknown) {
  return typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "翻译失败，请稍后重试。";
}

async function toggleTranslation() {
  if (translating.value) return;
  if (translatedContent.value) {
    showTranslated.value = !showTranslated.value;
    return;
  }
  translating.value = true;
  error.value = null;
  try {
    const translated = await invoke<string>("translate_markdown", { request: { markdown: props.content } });
    translationCache.set(cacheKey(), translated);
    translatedContent.value = translated;
    showTranslated.value = true;
  } catch (cause) { error.value = fail(cause); }
  finally { translating.value = false; }
}
</script>

<template>
  <div class="min-w-0 max-w-full overflow-hidden border-l-4 border-teal-500 bg-white">
    <div class="flex flex-wrap items-center justify-between gap-3 border-b border-slate-200 px-4 py-3">
      <h3 class="min-w-0 [overflow-wrap:anywhere] font-semibold text-slate-900">{{ name }} / SKILL.md</h3>
      <div class="flex shrink-0 items-center gap-2">
        <button type="button" class="inline-flex h-8 items-center gap-2 border border-teal-300 bg-white px-2.5 text-xs font-medium text-teal-800 hover:bg-teal-50 disabled:opacity-50" :disabled="translating" @click="toggleTranslation"><LoaderCircle v-if="translating" class="size-3.5 animate-spin" /><Languages v-else class="size-3.5" />{{ translating ? '翻译中' : translatedContent ? (showTranslated ? '查看原文' : '查看中文') : '翻译为中文' }}</button>
        <button type="button" class="inline-flex h-8 items-center gap-1.5 px-2 text-sm text-slate-500 hover:text-slate-900" aria-label="收起 SKILL.md 预览" @click.stop="emit('close')"><ChevronUp class="size-4" />收起预览</button>
      </div>
    </div>
    <p v-if="error" class="border-b border-rose-200 bg-rose-50 px-4 py-2 text-xs text-rose-800">{{ error }}</p>
    <MarkdownPreview :content="showTranslated && translatedContent ? translatedContent : content" />
  </div>
</template>
