#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptTarget {
    Generic,
    Claude,
}

pub const GENERIC_PROMPT: &str = "Please inspect drawings/latest.svg and drawings/latest.json, then describe what I drew. If those files do not exist, inspect ~/Pictures/GoghMode/drawings/latest.svg and ~/Pictures/GoghMode/drawings/latest.json instead. If you can inspect images, also inspect drawings/latest.png or ~/Pictures/GoghMode/drawings/latest.png.";

pub const CLAUDE_PROMPT: &str = "Please read drawings/latest.svg and drawings/latest.json, then describe what I drew. If those files do not exist, read ~/Pictures/GoghMode/drawings/latest.svg and ~/Pictures/GoghMode/drawings/latest.json instead. If image inspection is available, also inspect drawings/latest.png or ~/Pictures/GoghMode/drawings/latest.png.";

pub fn prompt_text(target: PromptTarget) -> &'static str {
    match target {
        PromptTarget::Generic => GENERIC_PROMPT,
        PromptTarget::Claude => CLAUDE_PROMPT,
    }
}
