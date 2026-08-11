<script setup lang="ts">
import { computed } from "vue";
import { Monitor, SquareTerminal } from "@lucide/vue";
import { faOpenai } from "@fortawesome/free-brands-svg-icons";
import { siClaude, siClaudecode } from "simple-icons";
import type { TargetId } from "../types/skill";

const props = defineProps<{ target: TargetId; compact?: boolean }>();

const isClaude = computed(() => props.target.startsWith("claude"));
const isCli = computed(() => props.target === "claude_code" || props.target === "codex_cli");
const brand = computed(() => {
  if (props.target === "claude_code") return { path: siClaudecode.path, viewBox: "0 0 24 24" };
  if (props.target === "claude_desktop") return { path: siClaude.path, viewBox: "0 0 24 24" };
  return { path: faOpenai.icon[4] as string, viewBox: `0 0 ${faOpenai.icon[0]} ${faOpenai.icon[1]}` };
});
</script>

<template>
  <span class="target-icon" :class="[isClaude ? 'target-icon-claude' : 'target-icon-codex', compact && 'target-icon-compact']" aria-hidden="true">
    <svg class="target-brand" :viewBox="brand.viewBox" focusable="false"><path fill="currentColor" :d="brand.path" /></svg>
    <span v-if="!compact" class="target-kind"><SquareTerminal v-if="isCli" class="size-2.5" :stroke-width="2.2" /><Monitor v-else class="size-2.5" :stroke-width="2.2" /></span>
  </span>
</template>

<style scoped>
.target-icon {
  position: relative;
  display: inline-flex;
  width: 2rem;
  height: 2rem;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border: 1px solid;
  border-radius: 0.375rem;
  background: #fff;
}

.target-icon-compact {
  width: 1.25rem;
  height: 1.25rem;
  border: 0;
  background: transparent;
}

.target-icon-claude {
  color: #c15f3c;
  border-color: #f2d4c7;
  background: #fff8f5;
}

.target-icon-codex {
  color: #292524;
  border-color: #d6d3d1;
  background: #fafaf9;
}

.target-brand {
  width: 1.05rem;
  height: 1.05rem;
}

.target-icon-compact .target-brand {
  width: 0.95rem;
  height: 0.95rem;
}

.target-kind {
  position: absolute;
  right: -0.3rem;
  bottom: -0.3rem;
  display: inline-flex;
  width: 1rem;
  height: 1rem;
  align-items: center;
  justify-content: center;
  border: 1px solid #d6d3d1;
  border-radius: 999px;
  color: #57534e;
  background: #fff;
  box-shadow: 0 1px 2px rgb(28 25 23 / 0.08);
}
</style>
