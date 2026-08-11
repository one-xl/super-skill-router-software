import Database from "@tauri-apps/plugin-sql";
import type { BatchInstallReport, Skill } from "../types/skill";
import type { InstallationRecord } from "../types/skill";

const DATABASE_URL = "sqlite:super-skill-router.db";

export async function recordInstallations(skill: Skill, directoryName: string, report: BatchInstallReport) {
  const database = await Database.load(DATABASE_URL);
  const now = new Date().toISOString();
  for (const result of report.results) {
    if (!result.outcome) continue;
    const status = result.outcome.kind === "installed" ? "installed" : "packaged_for_upload";
    await database.execute(
      `INSERT INTO installation_records
        (skill_name, directory_name, repository, source_url, commit_sha, target, status, installed_path, package_path, installed_at, updated_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
       ON CONFLICT(directory_name, target) DO UPDATE SET
         skill_name = excluded.skill_name,
         repository = excluded.repository,
         source_url = excluded.source_url,
         commit_sha = excluded.commit_sha,
         status = excluded.status,
         installed_path = excluded.installed_path,
         package_path = excluded.package_path,
         updated_at = excluded.updated_at`,
      [skill.name, directoryName, skill.repo, skill.source.rawUrl, skill.commit_sha, result.target, status, result.outcome.path ?? null, result.outcome.zip_path ?? null, now, now],
    );
  }
}

export async function loadInstalledRecords() {
  const database = await Database.load(DATABASE_URL);
  return database.select<InstallationRecord[]>("SELECT skill_name, directory_name, repository, commit_sha, target, status FROM installation_records WHERE status = 'installed'");
}
