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
  <section class="page-shell">
    <div class="page-header">
      <div><p class="page-kicker">Local Index</p><h1 class="page-title">本地索引</h1><p class="page-description">管理参与发现搜索和 Prompt 匹配的静态 skill 条目。</p></div>
      <div class="flex flex-wrap gap-2">
        <button v-if="store.removedCount" type="button" class="button-secondary" @click="store.restoreRemovedSkills"><RotateCcw class="size-4" />恢复已删除项（{{ store.removedCount }}）</button>
        <button type="button" class="button-secondary" :disabled="store.loading" @click="store.load"><RefreshCw class="size-4" :class="store.loading && 'animate-spin'" />刷新索引</button>
        <button type="button" class="button-danger" :disabled="!store.skills.length" @click="requestClear"><Trash2 class="size-4" />清空索引</button>
      </div>
    </div>

    <p v-if="error || store.error" class="notice-warning mb-5">{{ error ?? store.error }}</p>

    <div class="surface grid overflow-hidden sm:grid-cols-3">
      <div class="flex items-center gap-3 border-b border-stone-200 p-4 sm:border-b-0 sm:border-r"><span class="flex size-9 items-center justify-center rounded-md bg-teal-50 text-teal-700"><Database class="size-[18px]" /></span><div><p class="text-lg font-semibold text-stone-950">{{ store.skills.length }}</p><p class="text-[11px] text-stone-500">当前可用条目</p></div></div>
      <div class="border-b border-stone-200 p-4 sm:border-b-0 sm:border-r"><p class="text-lg font-semibold text-stone-950">{{ store.index?.sourceMatches ?? 0 }}</p><p class="text-[11px] text-stone-500">源匹配数量</p></div>
      <div class="p-4"><p class="text-[13px] font-medium text-stone-900">{{ store.index?.generatedAt ? new Date(store.index.generatedAt).toLocaleString('zh-CN') : '-' }}</p><p class="text-[11px] text-stone-500">最近生成时间</p></div>
    </div>

    <section class="table-shell mt-5 overflow-x-auto">
      <table class="data-table min-w-[48rem]">
        <colgroup><col class="w-[24%]" /><col class="w-[30%]" /><col class="w-[12%]" /><col class="w-[18%]" /><col class="w-[16%]" /></colgroup>
        <thead><tr><th class="px-4 py-3">Skill</th><th class="px-3 py-3">仓库</th><th class="px-3 py-3">文件</th><th class="px-3 py-3">固定版本</th><th class="px-3 py-3">操作</th></tr></thead>
        <tbody>
          <tr v-for="skill in store.skills" :key="skill.id">
            <td class="px-4 py-3 font-medium text-stone-900"><span class="block truncate" :title="skill.name">{{ skill.name }}</span></td>
            <td class="px-3 py-3 text-[11px] text-stone-600"><span class="block truncate" :title="skill.repo">{{ skill.repo }}</span></td>
            <td class="px-3 py-3 text-[11px] text-stone-500">{{ skill.files.length }}</td>
            <td class="px-3 py-3 font-mono text-[11px] text-stone-500">{{ skill.commit_sha ? skill.commit_sha.slice(0, 8) : '-' }}</td>
            <td class="px-3 py-3"><div class="flex gap-2"><button type="button" class="icon-button size-8" :disabled="openingSource === skill.id" title="在 GitHub 查看 SKILL.md" @click="openSource(skill)"><LoaderCircle v-if="openingSource === skill.id" class="size-4 animate-spin" /><ExternalLink v-else class="size-4" /></button><button type="button" class="icon-button size-8 border-rose-200 text-rose-700 hover:border-rose-300 hover:bg-rose-50 hover:text-rose-800" title="从本地索引删除" @click="requestRemove(skill)"><Trash2 class="size-4" /></button></div></td>
          </tr>
          <tr v-if="!store.loading && !store.skills.length"><td colspan="5" class="px-4 py-12 text-center text-slate-500">本地索引为空。<button v-if="store.removedCount" type="button" class="ml-2 text-teal-700 underline" @click="store.restoreRemovedSkills">恢复已删除项</button></td></tr>
        </tbody>
      </table>
    </section>

    <div v-if="pendingRemoval" class="modal-backdrop" role="dialog" aria-modal="true" aria-label="确认删除索引">
      <section class="modal-panel max-w-md">
        <div class="modal-icon-title"><span class="modal-icon bg-rose-50 text-rose-700"><AlertTriangle class="size-5" /></span>确认删除索引</div>
        <p class="mt-4 text-sm leading-6 text-stone-600">将从本机搜索与 Prompt 匹配中移除 <strong class="text-stone-900">{{ pendingRemoval.label }}</strong>。不会删除已安装的 skill，也不会修改远程静态索引源。</p>
        <p class="mt-2 text-[11px] text-stone-500">删除后可通过“恢复已删除项”重新加入。</p>
        <div class="mt-6 flex justify-end gap-2"><button type="button" class="button-ghost" @click="pendingRemoval = null">取消</button><button type="button" class="button-danger border-rose-600 bg-rose-600 text-white hover:bg-rose-700 hover:text-white" @click="confirmRemoval"><Trash2 class="size-4" />确认删除</button></div>
      </section>
    </div>
  </section>
</template>
