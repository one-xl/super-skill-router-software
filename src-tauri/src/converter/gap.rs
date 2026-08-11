use super::matcher::{score_skills, ScoredSkill, SkillMetadata};
use std::collections::HashSet;

pub fn find_gaps(
    requirement: &str,
    installed: &[SkillMetadata],
    index: &[SkillMetadata],
) -> Vec<ScoredSkill> {
    let installed_ids = installed
        .iter()
        .map(|skill| skill.id.as_str())
        .collect::<HashSet<_>>();
    let installed_names = installed
        .iter()
        .map(|skill| skill.name.to_lowercase())
        .collect::<HashSet<_>>();
    let candidates = index
        .iter()
        .filter(|skill| {
            !installed_ids.contains(skill.id.as_str())
                && !installed_names.contains(&skill.name.to_lowercase())
        })
        .cloned()
        .collect::<Vec<_>>();
    score_skills(requirement, &candidates)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(id: &str, name: &str) -> SkillMetadata {
        SkillMetadata {
            id: id.into(),
            name: name.into(),
            description: "PDF generation".into(),
            when_to_use: "when generating PDF documents".into(),
            tags: vec!["pdf".into()],
            frecency: 0.0,
        }
    }

    #[test]
    fn excludes_installed_skills_by_id_and_name() {
        let installed = vec![skill("installed", "existing-pdf")];
        let index = vec![
            skill("installed", "renamed-pdf"),
            skill("other-id", "existing-pdf"),
            skill("gap", "new-pdf"),
        ];

        let gaps = find_gaps("create a PDF", &installed, &index);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].skill.id, "gap");
    }
}
