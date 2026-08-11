mod assembler;
pub mod gap;
pub mod matcher;
pub mod parser;

use matcher::{score_skills, ScoredSkill, SkillMetadata};
use parser::{classify, Scenario};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionRequest {
    pub requirement: String,
    #[serde(default)]
    pub installed: Vec<SkillMetadata>,
    #[serde(default)]
    pub index: Vec<SkillMetadata>,
    pub selected_ids: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct ConversionResult {
    pub scenario: Scenario,
    pub prompt: String,
    pub selected: Vec<ScoredSkill>,
    pub gaps: Vec<ScoredSkill>,
}

#[tauri::command]
pub fn convert_requirement(request: ConversionRequest) -> Result<ConversionResult, String> {
    if request.requirement.trim().is_empty() {
        return Err("请输入需要转换的需求。".into());
    }
    let scenario = classify(&request.requirement);
    let automatic = score_skills(&request.requirement, &request.installed);
    let selected = match request.selected_ids {
        None => automatic,
        Some(ids) => {
            let scores = score_skills(&request.requirement, &request.installed);
            let scores_by_id = scores
                .iter()
                .map(|score| (score.skill.id.as_str(), score.score))
                .collect::<HashMap<_, _>>();
            let installed_by_id = request
                .installed
                .iter()
                .map(|skill| (skill.id.as_str(), skill))
                .collect::<HashMap<_, _>>();
            let mut seen_ids = HashSet::new();
            ids.into_iter()
                .filter(|id| seen_ids.insert(id.clone()))
                .filter_map(|id| {
                    installed_by_id.get(id.as_str()).map(|skill| ScoredSkill {
                        skill: (*skill).clone(),
                        score: scores_by_id.get(id.as_str()).copied().unwrap_or(0.0),
                    })
                })
                .take(matcher::MAX_RECOMMENDATIONS)
                .collect()
        }
    };
    let gaps = gap::find_gaps(&request.requirement, &request.installed, &request.index);
    let prompt = assembler::assemble(&request.requirement, scenario, &selected);
    Ok(ConversionResult {
        scenario,
        prompt,
        selected,
        gaps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(id: &str, name: &str, description: &str, when_to_use: &str) -> SkillMetadata {
        SkillMetadata {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            when_to_use: when_to_use.into(),
            tags: Vec::new(),
            frecency: 0.0,
        }
    }

    #[test]
    fn manual_selection_overrides_auto_matches_and_preserves_user_order() {
        let request = ConversionRequest {
            requirement: "generate a PDF report".into(),
            installed: vec![
                skill(
                    "first",
                    "pdf-generator",
                    "Generate PDF files",
                    "when creating PDFs",
                ),
                skill(
                    "second",
                    "reviewer",
                    "Review code",
                    "when reviewing changes",
                ),
            ],
            index: Vec::new(),
            selected_ids: Some(vec!["second".into(), "first".into()]),
        };

        let result = convert_requirement(request).expect("conversion should succeed");
        assert_eq!(result.selected.len(), 2);
        assert_eq!(result.selected[0].skill.id, "second");
        assert_eq!(result.selected[0].score, 0.0);
        assert_eq!(result.selected[1].skill.id, "first");
    }

    #[test]
    fn manual_selection_deduplicates_ids_before_applying_the_limit() {
        let request = ConversionRequest {
            requirement: "PDF".into(),
            installed: vec![
                skill("first", "pdf-one", "PDF files", "when creating PDFs"),
                skill("second", "pdf-two", "PDF files", "when creating PDFs"),
            ],
            index: Vec::new(),
            selected_ids: Some(vec!["first".into(), "first".into(), "second".into()]),
        };

        let result = convert_requirement(request).expect("conversion should succeed");
        assert_eq!(result.selected.len(), 2);
        assert_eq!(result.selected[0].skill.id, "first");
        assert_eq!(result.selected[1].skill.id, "second");
    }
}
