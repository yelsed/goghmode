#[path = "../src/skill.rs"]
mod skill;

use skill::{install_skill, narrated_skill_path, skill_path, SkillTarget};

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
    assert!(contents.contains("GoghMode always writes to the same place"));
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
    assert!(contents.contains("Use whichever `updatedAt` is larger"));
}

#[test]
fn claude_skill_picks_the_newest_drawing_rather_than_the_project_local_one() {
    let temp_home = tempfile::tempdir().unwrap();

    let path = install_skill(SkillTarget::Claude, temp_home.path()).unwrap();
    let contents = std::fs::read_to_string(path).unwrap();

    assert!(contents.contains("GoghMode always writes to the same place"));
    assert!(contents.contains("Use whichever `updatedAt` is larger"));
    assert!(
        !contents.contains("First try the project-local files"),
        "skill must not tell the agent to prefer project-local files by existence"
    );
}

/// A stamped sheet rewrites `latest.*` without being redrawn, so file times say
/// a day-old sketch arrived a minute ago. The skill has to read the stamp the
/// exporter wrote instead.
#[test]
fn claude_skill_judges_age_by_the_stamp_rather_than_the_file_time() {
    let temp_home = tempfile::tempdir().unwrap();

    let path = install_skill(SkillTarget::Claude, temp_home.path()).unwrap();
    let contents = std::fs::read_to_string(path).unwrap();

    assert!(contents.contains("`updatedAt` field inside `latest.json`"));
    assert!(
        !contents.contains("stat -f"),
        "comparing modification times is the mistake this replaced"
    );
}

/// Narration is a second skill rather than a longer first one: a sheet nobody
/// spoke over is still the ordinary case, and `/goghmode` keeps reading it as
/// it always has.
#[test]
fn install_skill_also_writes_the_narrated_skill_beside_the_plain_one() {
    let temp_home = tempfile::tempdir().unwrap();

    let plain_path = install_skill(SkillTarget::Claude, temp_home.path()).unwrap();
    let narrated_path = narrated_skill_path(SkillTarget::Claude, temp_home.path());

    assert_eq!(
        narrated_path,
        temp_home
            .path()
            .join(".claude")
            .join("skills")
            .join("goghmode-narrated")
            .join("SKILL.md")
    );
    let plain = std::fs::read_to_string(plain_path).unwrap();
    let narrated = std::fs::read_to_string(narrated_path).unwrap();

    assert!(plain.contains("name: goghmode\n"));
    assert!(!plain.contains("timeline"), "the plain skill must not grow");
    assert!(narrated.contains("name: goghmode-narrated"));
    assert!(narrated.contains("~/Pictures/GoghMode/drawings/latest.timeline.md"));
    assert!(narrated.contains("latest.steps/"));
    assert!(narrated.contains("`updatedAt` field inside `latest.json`"));
    assert!(narrated.contains("drawings/latest.json"));
    assert!(narrated.contains("use the `goghmode` skill"));
    assert!(narrated.contains("pale blue halo"));
}
