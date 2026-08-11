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
  <section class="mx-auto w-full max-w-6xl px-6 py-8 lg:px-10">
    <div class="mb-8 flex flex-wrap items-end justify-between gap-4">
      <div>
        <p class="mb-2 text-xs font-semibold uppercase tracking-[0.18em] text-teal-700">Discover</p>
        <h1 class="text-3xl font-semibold tracking-tight text-slate-950">技能发现</h1>
        <p class="mt-2 text-sm text-slate-500">优先搜索本地静态索引；需要时再手动查询 SkillsMP。</p>
      </div>
      <button class="inline-flex h-10 items-center gap-2 rounded-lg border border-slate-200 bg-white px-3 text-sm font-medium text-slate-600 shadow-sm transition hover:border-teal-300 hover:text-teal-700 disabled:cursor-not-allowed disabled:opacity-50" type="button" :disabled="store.loading" title="重新加载索引" @click="store.load">
        <RefreshCw class="size-4" :class="store.loading && 'animate-spin'" />刷新索引
      </button>
    </div>

    <SearchBar v-model="store.query" :result-count="store.results.length" :loading="store.loading && !store.index" :searching="remoteLoading" @search="searchRemote" />
    <p v-if="remoteError" class="mt-3 border-l-4 border-amber-400 bg-amber-50 px-3 py-2 text-sm text-amber-900">{{ remoteError }}</p>

    <div class="mt-5 flex flex-wrap items-center gap-2">
      <span class="mr-1 inline-flex items-center gap-1.5 text-xs font-semibold text-slate-500"><SlidersHorizontal class="size-3.5" />标签</span>
      <button v-for="tag in store.availableTags" :key="tag" type="button" class="rounded-full border px-3 py-1 text-xs transition" :class="store.activeTags.includes(tag) ? 'border-teal-600 bg-teal-600 text-white' : 'border-slate-200 bg-white text-slate-500 hover:border-teal-300 hover:text-teal-700'" :aria-pressed="store.activeTags.includes(tag)" @click="store.toggleTag(tag)">{{ tag }}</button>
    </div>

    <div v-if="store.error" class="mt-5 border-l-4 border-amber-400 bg-amber-50 px-4 py-3 text-sm text-amber-900" role="status">{{ store.error }}</div>

    <div v-if="store.loading && !store.index" class="mt-8 space-y-5" aria-live="polite"><div v-for="n in 3" :key="n" class="h-36 animate-pulse rounded-xl bg-slate-200/70" /></div>
    <div v-else-if="!store.error && store.results.length === 0" class="mt-8 rounded-xl border border-dashed border-slate-300 bg-white px-6 py-10 text-center"><h2 class="text-base font-semibold text-slate-800">本地索引没有匹配的 skill</h2><p class="mt-2 text-sm text-slate-500">可用上方按钮按需搜索 SkillsMP。</p></div>
    <div v-else class="mt-8 rounded-xl border border-slate-200 bg-white px-6 shadow-sm"><div class="flex items-center justify-between border-b border-slate-100 py-4 text-xs text-slate-400"><span>{{ store.results.length }} 个本地结果</span><span v-if="store.index?.generatedAt">索引更新于 {{ new Date(store.index.generatedAt).toLocaleString('zh-CN') }}<template v-if="store.index.truncated"> · 当前索引正在扩充</template></span></div><SkillCard v-for="result in store.results" :key="result.skill.id" :result="result" /></div>

    <div v-if="remoteSearched" class="mt-8 rounded-xl border border-teal-200 bg-white px-6 shadow-sm"><div class="flex items-center justify-between border-b border-teal-100 py-4 text-xs text-teal-800"><span>SkillsMP 远程结果</span><span>{{ remoteResults.length }} 个结果</span></div><p v-if="!remoteResults.length && !remoteLoading && !remoteError" class="py-8 text-center text-sm text-slate-500">未找到可解析为 GitHub skill 目录的结果。</p><SkillCard v-for="result in remoteResults" :key="result.skill.id" :result="result" /></div>
  </section>
</template>
