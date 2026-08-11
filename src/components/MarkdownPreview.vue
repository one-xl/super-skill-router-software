<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{ content: string }>();
type Block = { kind: "heading" | "code" | "list" | "paragraph"; value: string; level?: number };

const blocks = computed<Block[]>(() => {
  const result: Block[] = []; let paragraph: string[] = []; let list: string[] = []; let code: string[] = []; let inCode = false;
  const flush = () => { if (paragraph.length) result.push({ kind: "paragraph", value: paragraph.join(" ") }); if (list.length) result.push({ kind: "list", value: list.join("\n") }); paragraph = []; list = []; };
  for (const line of props.content.replace(/^---[\s\S]*?---\s*/, "").split("\n")) {
    if (line.startsWith("```")) { if (inCode) { result.push({ kind: "code", value: code.join("\n") }); code = []; } else flush(); inCode = !inCode; continue; }
    if (inCode) { code.push(line); continue; }
    const heading = /^(#{1,3})\s+(.+)$/.exec(line); if (heading) { flush(); result.push({ kind: "heading", value: heading[2], level: heading[1].length }); continue; }
    const item = /^[-*]\s+(.+)$/.exec(line); if (item) { list.push(item[1]); continue; }
    if (!line.trim()) { flush(); continue; } paragraph.push(line.trim());
  }
  flush(); if (code.length) result.push({ kind: "code", value: code.join("\n") }); return result;
});
</script>
<template><article class="max-h-[32rem] min-w-0 max-w-full overflow-y-auto overflow-x-hidden bg-white p-5 text-[13px] leading-6 text-stone-700 [overflow-wrap:anywhere]"><template v-for="(block, index) in blocks" :key="index"><h3 v-if="block.kind === 'heading'" class="mt-5 font-semibold text-stone-950 first:mt-0" :class="block.level === 1 ? 'text-lg' : 'text-sm'">{{ block.value }}</h3><pre v-else-if="block.kind === 'code'" class="my-3 max-w-full overflow-x-auto rounded-md border border-stone-200 bg-stone-950 p-3 font-mono text-[11px] text-stone-200">{{ block.value }}</pre><ul v-else-if="block.kind === 'list'" class="my-3 list-disc space-y-1 pl-5 marker:text-teal-600"><li v-for="item in block.value.split('\n')" :key="item">{{ item }}</li></ul><p v-else class="my-3">{{ block.value }}</p></template></article></template>
