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

GoghMode always writes to the same place, whether it was started from a terminal, Spotlight,
Raycast, or a Linux application launcher:

- `~/Pictures/GoghMode/drawings/latest.json`
- `~/Pictures/GoghMode/drawings/latest.svg`
- `~/Pictures/GoghMode/drawings/latest.png`

## Steps

1. Read `latest.json` and `latest.svg` from that directory. If image inspection is available,
   inspect `latest.png` too.
2. Judge the drawing's age from the `updatedAt` field inside `latest.json`, in unix milliseconds —
   not from the file's modification time. The files are rewritten whenever a sheet is stamped, so
   an mtime from a minute ago can belong to a sketch drawn yesterday.

   ```bash
   python3 -c "import json,datetime;print(datetime.datetime.fromtimestamp(int(json.load(open('$HOME/Pictures/GoghMode/drawings/latest.json'))['updatedAt'])/1000))"
   ```

   If that time is more than a few hours ago, name it before describing the drawing, so the user
   knows you are not looking at what they just drew. A sketch that is much older than the user
   expects usually means a paired device has stopped reaching the host — say so, and point them at
   the Devices view, which names when each device was last seen and why the last attempt was
   turned away.
3. Describe the drawing in plain language and connect it to the user's current question.

## If a project-local `drawings/` directory also exists

Older versions wrote to `drawings/` relative to the terminal's working directory, and
`--drawings-dir` can still redirect output on purpose. So a stale `drawings/latest.*` may be sitting
in the project. Never assume it is the current one — compare their `updatedAt` fields, for the same
reason mtimes are not trusted above:

```bash
python3 -c "
import json,os
for path in ('drawings/latest.json', os.path.expanduser('~/Pictures/GoghMode/drawings/latest.json')):
    try:
        print(json.load(open(path))['updatedAt'], path)
    except OSError:
        pass
"
```

Use whichever `updatedAt` is larger, and say which one you read.

If neither location has the files, ask the user to open `goghmode`, draw once, and release the pointer or tap Send to desktop.
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
