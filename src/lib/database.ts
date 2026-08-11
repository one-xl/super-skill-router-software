import Database from "@tauri-apps/plugin-sql";
import type { BatchInstallReport, Skill } from "../types/skill";

const DATABASE_URL = "sqlite:super-skill-router.db";

export async function recordInstallations(skill: Skill, directoryName: string, report: BatchInstallReport) {
  const database = await Database.load(DATABASE_URL);
  const now = new Date().toISOString();
  for (const result of report.results) {
    if (!result.outcome || result.outcome.kind !== "installed") continue;
    await database.execute(
      `INSERT INTO installation_records
        (skill_name, directory_name, repository, source_url, commit_sha, target, status, installed_path, installed_at, updated_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
       ON CONFLICT(directory_name, target) DO UPDATE SET
         skill_name = excluded.skill_name,
         repository = excluded.repository,
         source_url = excluded.source_url,
         commit_sha = excluded.commit_sha,
         status = excluded.status,
         installed_path = excluded.installed_path,
         updated_at = excluded.updated_at`,
      [skill.name, directoryName, skill.repo, skill.source.rawUrl, skill.commit_sha, result.target, "installed", result.outcome.path ?? null, now, now],
    );
  }
}
