use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillTarget {
    Claude,
}

pub const CLAUDE_SKILL: &str = r#"---
name: goghmode
description: Use when the user asks to inspect a sketch, drawing, whiteboard, diagram, or latest GoghMode output.
---

# GoghMode

Use this skill when the user wants you to inspect the latest GoghMode sketch.

## Steps

1. First try the project-local files:
   - `drawings/latest.json`
   - `drawings/latest.svg`
   - `drawings/latest.png`
2. If the project-local files do not exist, try the Spotlight/Raycast app fallback files:
   - `~/Pictures/GoghMode/drawings/latest.json`
   - `~/Pictures/GoghMode/drawings/latest.svg`
   - `~/Pictures/GoghMode/drawings/latest.png`
3. Read the JSON and SVG. If image inspection is available, inspect the PNG.
4. Describe the drawing in plain language and connect it to the user's current question.

If neither location has the files, ask the user to open `goghmode`, draw once, and release the pointer or tap Send to Mac.
"#;

pub fn skill_path(target: SkillTarget, home_dir: &Path) -> PathBuf {
    match target {
        SkillTarget::Claude => home_dir
            .join(".claude")
            .join("skills")
            .join("goghmode")
            .join("SKILL.md"),
    }
}

pub fn install_skill(target: SkillTarget, home_dir: &Path) -> anyhow::Result<PathBuf> {
    let path = skill_path(target, home_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, CLAUDE_SKILL)?;
    Ok(path)
}
