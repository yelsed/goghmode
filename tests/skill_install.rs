#[path = "../src/skill.rs"]
mod skill;

use skill::{install_skill, skill_path, SkillTarget};

#[test]
fn claude_skill_path_uses_home_claude_skills_directory() {
    let temp_home = tempfile::tempdir().unwrap();

    let path = skill_path(SkillTarget::Claude, temp_home.path());

    assert_eq!(
        path,
        temp_home
            .path()
            .join(".claude")
            .join("skills")
            .join("goghmode")
            .join("SKILL.md")
    );
}

#[test]
fn claude_skill_mentions_spotlight_app_fallback_directory() {
    let temp_home = tempfile::tempdir().unwrap();

    let path = install_skill(SkillTarget::Claude, temp_home.path()).unwrap();
    let contents = std::fs::read_to_string(path).unwrap();

    assert!(contents.contains("~/Pictures/GoghMode/drawings/latest.json"));
    assert!(contents.contains("Compare the modification times of both candidates"));
}

#[test]
fn install_skill_writes_claude_skill_contents() {
    let temp_home = tempfile::tempdir().unwrap();

    let path = install_skill(SkillTarget::Claude, temp_home.path()).unwrap();
    let contents = std::fs::read_to_string(path).unwrap();

    assert!(contents.contains("name: goghmode"));
    assert!(contents.contains("drawings/latest.json"));
    assert!(contents.contains("drawings/latest.svg"));
    assert!(contents.contains("drawings/latest.png"));
    assert!(contents.contains("Use whichever is newer"));
}

#[test]
fn claude_skill_picks_the_newest_drawing_rather_than_the_project_local_one() {
    let temp_home = tempfile::tempdir().unwrap();

    let path = install_skill(SkillTarget::Claude, temp_home.path()).unwrap();
    let contents = std::fs::read_to_string(path).unwrap();

    assert!(contents.contains("Use whichever is newer"));
    assert!(contents.contains("Do not prefer the project-local copy just because it exists"));
    assert!(
        !contents.contains("First try the project-local files"),
        "skill must not tell the agent to prefer project-local files by existence"
    );
}
