use super::parser::keywords;

pub const MINIMUM_SCORE: f64 = 3.0;
pub const MAX_RECOMMENDATIONS: usize = 5;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub when_to_use: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub frecency: f64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ScoredSkill {
    #[serde(flatten)]
    pub skill: SkillMetadata,
    pub score: f64,
}

pub fn score_skills(requirement: &str, skills: &[SkillMetadata]) -> Vec<ScoredSkill> {
    let terms = keywords(requirement);
    let mut results = skills
        .iter()
        .cloned()
        .map(|skill| {
            let score = raw_score(&terms, &skill);
            ScoredSkill { skill, score }
        })
        .filter(|result| result.score >= MINIMUM_SCORE)
        .collect::<Vec<_>>();
    results.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.skill.name.cmp(&right.skill.name))
    });
    results.truncate(MAX_RECOMMENDATIONS);
    results
}

fn raw_score(terms: &[String], skill: &SkillMetadata) -> f64 {
    let name = skill.name.to_lowercase();
    let description = skill.description.to_lowercase();
    let when_to_use = skill.when_to_use.to_lowercase();
    let tags = skill
        .tags
        .iter()
        .map(|tag| tag.to_lowercase())
        .collect::<Vec<_>>();
    let mut score = 0.0;
    for term in terms {
        if name.contains(term) {
            score += 3.0;
        }
        if when_to_use.contains(term) {
            score += 2.5;
        }
        if description.contains(term) {
            score += 2.0;
        }
        if tags.iter().any(|tag| tag.contains(term)) {
            score += 1.5;
        }
    }
    if score > 0.0 {
        score += skill.frecency.clamp(0.0, 1.0) * 0.5;
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    fn skill(
        id: &str,
        name: &str,
        description: &str,
        when: &str,
        tags: &[&str],
        frecency: f64,
    ) -> SkillMetadata {
        SkillMetadata {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            when_to_use: when.into(),
            tags: tags.iter().map(|tag| (*tag).into()).collect(),
            frecency,
        }
    }
    #[test]
    fn applies_field_weights_and_threshold() {
        let candidates = vec![
            skill("name", "pdf", "general files", "anything", &[], 0.0),
            skill(
                "when",
                "documents",
                "general files",
                "use for PDF work",
                &[],
                0.0,
            ),
            skill(
                "description",
                "documents",
                "process PDF files",
                "anything",
                &[],
                0.0,
            ),
        ];
        let terms = keywords("pdf");
        assert_eq!(raw_score(&terms, &candidates[0]), 3.0);
        assert_eq!(raw_score(&terms, &candidates[1]), 2.5);
        assert_eq!(raw_score(&terms, &candidates[2]), 2.0);
        let results = score_skills("pdf", &candidates);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].skill.id, "name");
    }
    #[test]
    fn applies_tag_and_frecency_bonus_without_bypassing_threshold() {
        let candidates = vec![
            skill("matched", "writer", "", "", &["blog"], 1.0),
            skill("popular-only", "writer", "", "", &[], 1.0),
        ];
        let terms = keywords("blog");
        assert_eq!(raw_score(&terms, &candidates[0]), 2.0);
        assert!(score_skills("blog", &candidates).is_empty());
    }
    #[test]
    fn keeps_only_top_five_relevant_skills() {
        let candidates = (0..7)
            .map(|number| {
                skill(
                    &number.to_string(),
                    &format!("pdf-{number}"),
                    "",
                    "",
                    &[],
                    0.0,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(score_skills("pdf", &candidates).len(), 5);
    }
}
