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

/// The second skill, installed beside the first by the same command. It is a
/// separate skill on purpose: a sheet nobody spoke over is still the ordinary
/// case, and `/goghmode` has to keep reading it exactly as it always has.
pub const NARRATED_SKILL: &str = r#"---
name: goghmode-narrated
description: Use when the user asks what they said or explained while drawing, wants a GoghMode sketch walked through step by step, or refers to a narrated sheet or a whiteboard explanation.
---

# GoghMode, narrated

A narrated sheet is a drawing recorded together with what was said while it was drawn. The host
writes it next to the ordinary files, in the same directory the `goghmode` skill reads:

- `~/Pictures/GoghMode/drawings/latest.timeline.md`
- `~/Pictures/GoghMode/drawings/latest.steps/001.png`, `002.png`, and so on
- `~/Pictures/GoghMode/drawings/latest.png`, `latest.svg` and `latest.json`, as always

## Steps

1. Read `latest.timeline.md`. If it does not exist, the current sheet was not narrated: say so,
   and use the `goghmode` skill to read the plain sheet instead.
2. Judge the sheet's age from the `updatedAt` field inside `latest.json`, in unix milliseconds,
   exactly as the `goghmode` skill does, and name it if it is more than a few hours old.
3. Go through the steps in order. Each step quotes what was said and names a crop in
   `latest.steps/`. If image inspection is available, look at each crop as you reach it. A crop is
   a small window on the page showing only where ink was added during that step: the ink added in
   the step sits on a pale blue halo, every stroke keeps its own colour and width, and earlier ink
   inside the window is drawn unchanged. The step line says where the window sits on the page.
4. Only then look at `latest.png` for the page as a whole, and connect the sequence of words and
   ink to the user's question. The order matters: a whiteboard that is unreadable at the end is
   usually clear when it is read in the order it was drawn.

## If a project-local `drawings/` directory also exists

Same rule as the `goghmode` skill: compare the `updatedAt` fields of `drawings/latest.json` and
`~/Pictures/GoghMode/drawings/latest.json`, read from the directory whose stamp is larger, and say
which one you read. The timeline and the crops live beside whichever `latest.json` you chose.
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

pub fn narrated_skill_path(target: SkillTarget, home_dir: &Path) -> PathBuf {
    match target {
        SkillTarget::Claude => home_dir
            .join(".claude")
            .join("skills")
            .join("goghmode-narrated")
            .join("SKILL.md"),
    }
}

/// Writes both skills and returns the path of `/goghmode`, the one every
/// caller has always been handed; the narrated skill's path comes from
/// `narrated_skill_path`.
pub fn install_skill(target: SkillTarget, home_dir: &Path) -> anyhow::Result<PathBuf> {
    let path = skill_path(target, home_dir);
    write_skill(&path, CLAUDE_SKILL)?;
    write_skill(&narrated_skill_path(target, home_dir), NARRATED_SKILL)?;
    Ok(path)
}

fn write_skill(path: &Path, contents: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    Ok(())
}
