#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptTarget {
    Generic,
    Claude,
}

// These stay free of shell metacharacters on purpose, so they are safe to paste
// into any terminal AI tool. That is enforced by a test.
pub const GENERIC_PROMPT: &str = "Please inspect the newest GoghMode drawing. Two locations can exist at the same time: drawings/latest.svg, drawings/latest.json, drawings/latest.png and ~/Pictures/GoghMode/drawings/latest.svg, ~/Pictures/GoghMode/drawings/latest.json, ~/Pictures/GoghMode/drawings/latest.png. Compare their modification times and use whichever directory is newer, because a leftover project-local copy from an earlier session is often weeks old. Inspect the .svg and .json from that directory, and the .png too if you can inspect images. Tell me if the newest file is more than a few hours old, then describe what I drew.";

pub const CLAUDE_PROMPT: &str = "Please read the newest GoghMode drawing. Two locations can exist at the same time: drawings/latest.svg, drawings/latest.json, drawings/latest.png and ~/Pictures/GoghMode/drawings/latest.svg, ~/Pictures/GoghMode/drawings/latest.json, ~/Pictures/GoghMode/drawings/latest.png. Compare their modification times and use whichever directory is newer, because a leftover project-local copy from an earlier session is often weeks old. Read the .svg and .json from that directory, and inspect the .png if image inspection is available. Tell me if the newest file is more than a few hours old, then describe what I drew.";

pub fn prompt_text(target: PromptTarget) -> &'static str {
    match target {
        PromptTarget::Generic => GENERIC_PROMPT,
        PromptTarget::Claude => CLAUDE_PROMPT,
    }
}
