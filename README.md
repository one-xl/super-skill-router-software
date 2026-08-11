# Super Skill Router

Windows desktop application for discovering, scanning, deploying, and managing AI agent skills.

## Current milestone

M1 provides a Tauri 2, Vue 3, TypeScript, TailwindCSS, and Pinia foundation. The discover page loads the bundled static `index.json`, caches it locally, and searches it with uFuzzy. Search priority is `name` (3.0), `whenToUse` (2.5), then `description` (2.0). Interactive search never calls GitHub Code Search.

## Run

```powershell
npm install
npm run tauri dev
```

## Indexer

`indexer/crawl.py` builds the static index every day through `.github/workflows/update-index.yml`. Each record contains the immutable commit SHA plus a complete skill-directory manifest. The application must download, scan, package, and deploy the whole directory tree, never only `SKILL.md`.

```powershell
python -m pip install -r indexer/requirements.txt
$env:GITHUB_TOKEN = "<token>"
python indexer/crawl.py
```
