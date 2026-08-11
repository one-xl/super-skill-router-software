<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { RefreshCw, SlidersHorizontal } from "@lucide/vue";
import { invoke } from "@tauri-apps/api/core";
import SearchBar from "../components/SearchBar.vue";
import SkillCard from "../components/SkillCard.vue";
import { useSkillIndexStore } from "../stores";
import type { Skill, SkillSearchResult } from "../types/skill";

const store = useSkillIndexStore();
const remoteSkills = ref<Skill[]>([]);
const remoteLoading = ref(false);
const remoteError = ref<string | null>(null);
const remoteSearched = ref(false);
const remoteResults = computed<SkillSearchResult[]>(() => remoteSkills.value.map((skill) => ({ skill, score: 0, matchedFields: [] })));

function fail(cause: unknown) {
  return typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "操作失败，请重试。";
}

async function searchRemote() {
  if (remoteLoading.value) return;
  remoteError.value = null;
  if (store.query.trim().length < 2) {
    remoteError.value = "请输入至少两个字符后再搜索 SkillsMP。";
    return;
  }
  remoteLoading.value = true;
  remoteSearched.value = true;
  try {
    remoteSkills.value = await invoke<Skill[]>("search_skillsmp", { request: { query: store.query, limit: 20 } });
  } catch (cause) {
    remoteError.value = fail(cause);
  } finally {
    remoteLoading.value = false;
  }
}

onMounted(() => { void store.load(); });
</script>

<template>
  <section class="page-shell">
    <div class="page-header">
      <div>
        <p class="page-kicker">Discover</p>
        <h1 class="page-title">技能发现</h1>
        <p class="page-description">搜索本地静态索引，按需扩展到 SkillsMP。</p>
      </div>
      <button class="button-secondary" type="button" :disabled="store.loading" title="重新加载索引" @click="store.load">
        <RefreshCw class="size-4" :class="store.loading && 'animate-spin'" />刷新索引
      </button>
    </div>

    <SearchBar v-model="store.query" :result-count="store.results.length" :loading="store.loading && !store.index" :searching="remoteLoading" @search="searchRemote" />
    <p v-if="remoteError" class="notice-warning mt-3">{{ remoteError }}</p>

    <div class="mt-5 flex flex-wrap items-center gap-2">
      <span class="mr-1 inline-flex items-center gap-1.5 text-[11px] font-medium text-stone-500"><SlidersHorizontal class="size-3.5" />筛选</span>
      <button v-for="tag in store.availableTags" :key="tag" type="button" class="rounded-full border px-2.5 py-1 text-[11px] font-medium transition duration-150" :class="store.activeTags.includes(tag) ? 'border-teal-700 bg-teal-700 text-white' : 'border-stone-200 bg-white text-stone-500 hover:border-stone-300 hover:text-stone-800'" :aria-pressed="store.activeTags.includes(tag)" @click="store.toggleTag(tag)">{{ tag }}</button>
    </div>

    <div v-if="store.error" class="notice-warning mt-5" role="status">{{ store.error }}</div>

    <div v-if="store.loading && !store.index" class="mt-6 space-y-3" aria-live="polite"><div v-for="n in 3" :key="n" class="h-32 animate-pulse rounded-lg border border-stone-200 bg-white" /></div>
    <div v-else-if="!store.error && store.results.length === 0" class="surface mt-6 border-dashed px-6 py-10 text-center"><h2 class="text-sm font-semibold text-stone-800">本地索引没有匹配的 skill</h2><p class="mt-2 text-[13px] text-stone-500">可用搜索按钮按需查询 SkillsMP。</p></div>
    <div v-else class="surface mt-6 px-5"><div class="flex items-center justify-between border-b border-stone-100 py-3.5 text-[11px] text-stone-400"><span>{{ store.results.length }} 个本地结果</span><span v-if="store.index?.generatedAt">索引更新于 {{ new Date(store.index.generatedAt).toLocaleString('zh-CN') }}<template v-if="store.index.truncated"> · 当前索引正在扩充</template></span></div><SkillCard v-for="result in store.results" :key="result.skill.id" :result="result" /></div>

    <div v-if="remoteSearched" class="surface mt-6 border-teal-200 px-5"><div class="flex items-center justify-between border-b border-teal-100 py-3.5 text-[11px] font-medium text-teal-800"><span>SkillsMP 远程结果</span><span>{{ remoteResults.length }} 个结果</span></div><p v-if="!remoteResults.length && !remoteLoading && !remoteError" class="py-8 text-center text-[13px] text-stone-500">未找到可解析为 GitHub skill 目录的结果。</p><SkillCard v-for="result in remoteResults" :key="result.skill.id" :result="result" /></div>
  </section>
</template>
