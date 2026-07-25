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

GoghMode writes to one of two directories depending on how it was started.

Started from a terminal in that directory:

- `drawings/latest.json`
- `drawings/latest.svg`
- `drawings/latest.png`

Started from Spotlight, Raycast, or the app bundle:

- `~/Pictures/GoghMode/drawings/latest.json`
- `~/Pictures/GoghMode/drawings/latest.svg`
- `~/Pictures/GoghMode/drawings/latest.png`

Both sets can exist at the same time, and either one can be the stale copy.

## Steps

1. Compare the modification times of both candidates:

   ```bash
   stat -f "%Sm %N" drawings/latest.json ~/Pictures/GoghMode/drawings/latest.json 2>/dev/null
   ```

2. Use whichever is newer. Do not prefer the project-local copy just because it exists. A leftover
   `drawings/latest.*` from an earlier session is the usual way to end up confidently describing a
   drawing from weeks ago.
3. Read `latest.json` and `latest.svg` from the directory you chose. If image inspection is
   available, inspect `latest.png` from that same directory.
4. If the newest file is more than a few hours old, say so before describing it, so the user knows
   you are not looking at what they just drew.
5. Describe the drawing in plain language and connect it to the user's current question.

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
