import { openUrl } from "@tauri-apps/plugin-opener";
import type { Skill } from "../types/skill";

export function skillSourceUrl(skill: Skill) {
  const source = new URL(skill.source.rawUrl);
  if (source.hostname !== "raw.githubusercontent.com") return source.toString();

  const [owner, repository, reference, ...path] = source.pathname.split("/").filter(Boolean);
  if (!owner || !repository || !reference || path.length === 0) return source.toString();
  return `https://github.com/${owner}/${repository}/blob/${reference}/${path.join("/")}`;
}

export async function openSkillSource(skill: Skill) {
  await openUrl(skillSourceUrl(skill));
}
