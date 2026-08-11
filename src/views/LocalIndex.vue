<script setup lang="ts">
import { onMounted, ref } from "vue";
import { AlertTriangle, Database, ExternalLink, LoaderCircle, RefreshCw, RotateCcw, Trash2 } from "@lucide/vue";
import { openSkillSource } from "../lib/skill-source";
import { useSkillIndexStore } from "../stores";
import type { Skill } from "../types/skill";

type PendingRemoval = { ids: string[]; label: string };

const store = useSkillIndexStore();
const pendingRemoval = ref<PendingRemoval | null>(null);
const openingSource = ref<string | null>(null);
const error = ref<string | null>(null);

function fail(cause: unknown) {
  return typeof cause === "string" ? cause : cause instanceof Error ? cause.message : "操作失败，请重试。";
}

async function openSource(skill: Skill) {
  if (openingSource.value) return;
  openingSource.value = skill.id;
  error.value = null;
  try { await openSkillSource(skill); }
  catch (cause) { error.value = fail(cause); }
  finally { openingSource.value = null; }
}

function requestRemove(skill: Skill) {
  pendingRemoval.value = { ids: [skill.id], label: skill.name };
}

function requestClear() {
  if (store.skills.length) pendingRemoval.value = { ids: store.skills.map((skill) => skill.id), label: `全部 ${store.skills.length} 条索引` };
}

function confirmRemoval() {
  if (!pendingRemoval.value) return;
  store.removeSkills(pendingRemoval.value.ids);
  pendingRemoval.value = null;
}

onMounted(() => { if (!store.index) void store.load(); });
</script>

<template>
  <section class="mx-auto w-full max-w-6xl px-6 py-8 lg:px-10">
    <div class="mb-6 flex flex-wrap items-end justify-between gap-4">
      <div><p class="mb-2 text-xs font-semibold uppercase tracking-[0.18em] text-teal-700">Local Index</p><h1 class="text-3xl font-semibold text-slate-950">本地索引</h1><p class="mt-2 text-sm text-slate-500">管理参与发现搜索和 Prompt 匹配的静态 skill 条目。</p></div>
      <div class="flex flex-wrap gap-2">
        <button v-if="store.removedCount" type="button" class="inline-flex h-10 items-center gap-2 border border-slate-300 bg-white px-3 text-sm text-slate-700 hover:border-teal-400 hover:text-teal-800" @click="store.restoreRemovedSkills"><RotateCcw class="size-4" />恢复已删除项（{{ store.removedCount }}）</button>
        <button type="button" class="inline-flex h-10 items-center gap-2 border border-slate-300 bg-white px-3 text-sm text-slate-700 disabled:opacity-50" :disabled="store.loading" @click="store.load"><RefreshCw class="size-4" :class="store.loading && 'animate-spin'" />刷新索引</button>
        <button type="button" class="inline-flex h-10 items-center gap-2 border border-rose-200 bg-white px-3 text-sm text-rose-700 disabled:opacity-50" :disabled="!store.skills.length" @click="requestClear"><Trash2 class="size-4" />清空索引</button>
      </div>
    </div>

    <p v-if="error || store.error" class="mb-5 border-l-4 border-amber-400 bg-amber-50 px-4 py-3 text-sm text-amber-900">{{ error ?? store.error }}</p>

    <div class="grid border border-slate-200 bg-white sm:grid-cols-3">
      <div class="flex items-center gap-3 border-b border-slate-200 p-4 sm:border-b-0 sm:border-r"><Database class="size-5 text-teal-600" /><div><p class="text-xl font-semibold text-slate-950">{{ store.skills.length }}</p><p class="text-xs text-slate-500">当前可用条目</p></div></div>
      <div class="border-b border-slate-200 p-4 sm:border-b-0 sm:border-r"><p class="text-xl font-semibold text-slate-950">{{ store.index?.sourceMatches ?? 0 }}</p><p class="text-xs text-slate-500">源匹配数量</p></div>
      <div class="p-4"><p class="text-sm font-medium text-slate-900">{{ store.index?.generatedAt ? new Date(store.index.generatedAt).toLocaleString('zh-CN') : '-' }}</p><p class="text-xs text-slate-500">最近生成时间</p></div>
    </div>

    <section class="mt-5 overflow-x-auto border border-slate-200 bg-white">
      <table class="w-full min-w-[48rem] table-fixed text-left text-sm">
        <colgroup><col class="w-[24%]" /><col class="w-[30%]" /><col class="w-[12%]" /><col class="w-[18%]" /><col class="w-[16%]" /></colgroup>
        <thead class="bg-slate-50 text-xs text-slate-500"><tr><th class="px-4 py-3">Skill</th><th class="px-3 py-3">仓库</th><th class="px-3 py-3">文件</th><th class="px-3 py-3">固定版本</th><th class="px-3 py-3">操作</th></tr></thead>
        <tbody>
          <tr v-for="skill in store.skills" :key="skill.id" class="border-t border-slate-200">
            <td class="px-4 py-3 font-medium text-slate-900"><span class="block truncate" :title="skill.name">{{ skill.name }}</span></td>
            <td class="px-3 py-3 text-xs text-slate-600"><span class="block truncate" :title="skill.repo">{{ skill.repo }}</span></td>
            <td class="px-3 py-3 text-xs text-slate-500">{{ skill.files.length }}</td>
            <td class="px-3 py-3 font-mono text-xs text-slate-500">{{ skill.commit_sha ? skill.commit_sha.slice(0, 8) : '-' }}</td>
            <td class="px-3 py-3"><div class="flex gap-2"><button type="button" class="flex size-8 items-center justify-center border border-slate-300 text-slate-600 hover:border-teal-300 hover:text-teal-700 disabled:opacity-50" :disabled="openingSource === skill.id" title="在 GitHub 查看 SKILL.md" @click="openSource(skill)"><LoaderCircle v-if="openingSource === skill.id" class="size-4 animate-spin" /><ExternalLink v-else class="size-4" /></button><button type="button" class="flex size-8 items-center justify-center border border-rose-200 text-rose-700" title="从本地索引删除" @click="requestRemove(skill)"><Trash2 class="size-4" /></button></div></td>
          </tr>
          <tr v-if="!store.loading && !store.skills.length"><td colspan="5" class="px-4 py-12 text-center text-slate-500">本地索引为空。<button v-if="store.removedCount" type="button" class="ml-2 text-teal-700 underline" @click="store.restoreRemovedSkills">恢复已删除项</button></td></tr>
        </tbody>
      </table>
    </section>

    <div v-if="pendingRemoval" class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/35 p-5" role="dialog" aria-modal="true" aria-label="确认删除索引">
      <section class="w-full max-w-md border border-slate-200 bg-white p-6 shadow-xl">
        <div class="flex items-center gap-2 text-base font-semibold text-slate-950"><AlertTriangle class="size-5 text-rose-600" />确认删除索引</div>
        <p class="mt-3 text-sm leading-6 text-slate-600">将从本机搜索与 Prompt 匹配中移除 <strong class="text-slate-900">{{ pendingRemoval.label }}</strong>。不会删除已安装的 skill，也不会修改远程静态索引源。</p>
        <p class="mt-2 text-xs text-slate-500">删除后可通过“恢复已删除项”重新加入。</p>
        <div class="mt-5 flex justify-end gap-2"><button type="button" class="h-9 border border-slate-300 bg-white px-3 text-sm text-slate-700" @click="pendingRemoval = null">取消</button><button type="button" class="inline-flex h-9 items-center gap-2 bg-rose-600 px-3 text-sm font-medium text-white hover:bg-rose-700" @click="confirmRemoval"><Trash2 class="size-4" />确认删除</button></div>
      </section>
    </div>
  </section>
</template>
