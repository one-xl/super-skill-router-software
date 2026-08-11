<script setup lang="ts">
import { ExternalLink, FileText, GitBranch } from "@lucide/vue";
import type { SkillSearchResult } from "../types/skill";

defineProps<{ result: SkillSearchResult }>();
</script>

<template>
  <article class="group border-b border-slate-200 py-6 first:pt-2 last:border-b-0">
    <div class="flex items-start justify-between gap-5">
      <div class="min-w-0">
        <div class="mb-2 flex flex-wrap items-center gap-2">
          <h2 class="truncate text-lg font-semibold text-slate-950">{{ result.skill.name }}</h2>
          <span v-for="field in result.matchedFields" :key="field" class="rounded-full bg-teal-50 px-2 py-0.5 text-[11px] font-medium text-teal-700">
            命中{{ field === 'name' ? '名称' : field === 'whenToUse' ? '触发场景' : '描述' }}
          </span>
        </div>
        <p class="mb-3 max-w-3xl text-sm leading-6 text-slate-600">{{ result.skill.description }}</p>
        <p v-if="result.skill.whenToUse" class="mb-3 max-w-3xl text-sm leading-6 text-slate-500">
          <span class="font-medium text-slate-700">适用场景：</span>{{ result.skill.whenToUse }}
        </p>
        <div class="flex flex-wrap items-center gap-x-4 gap-y-2 text-xs text-slate-400">
          <span class="inline-flex items-center gap-1.5"><GitBranch class="size-3.5" />{{ result.skill.repo }}</span>
          <span class="inline-flex items-center gap-1.5"><FileText class="size-3.5" />{{ result.skill.files.length }} 个文件</span>
          <span>固定版本 {{ result.skill.commit_sha.slice(0, 8) }}</span>
          <span v-for="tag in result.skill.tags" :key="tag" class="rounded bg-slate-100 px-2 py-0.5 text-slate-500">#{{ tag }}</span>
        </div>
      </div>
      <a
        class="flex size-9 shrink-0 items-center justify-center rounded-lg border border-slate-200 text-slate-400 transition hover:border-teal-300 hover:bg-teal-50 hover:text-teal-700"
        :href="result.skill.source.rawUrl"
        target="_blank"
        rel="noreferrer"
        title="在 GitHub 查看 SKILL.md"
        aria-label="在 GitHub 查看 SKILL.md"
      >
        <ExternalLink class="size-4" :stroke-width="1.8" />
      </a>
    </div>
  </article>
</template>
