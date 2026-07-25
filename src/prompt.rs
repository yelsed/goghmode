#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptTarget {
    Generic,
    Claude,
}

// These stay free of shell metacharacters on purpose, so they are safe to paste
// into any terminal AI tool. That is enforced by a test.
pub const GENERIC_PROMPT: &str = "Please inspect my latest GoghMode drawing at ~/Pictures/GoghMode/drawings/latest.svg and ~/Pictures/GoghMode/drawings/latest.json, then describe what I drew. If you can inspect images, also inspect ~/Pictures/GoghMode/drawings/latest.png. Older versions wrote to drawings/latest.svg, drawings/latest.json and drawings/latest.png in the current directory, so if those exist too, compare their modification times and use whichever is newer. Tell me if the drawing you read is more than a few hours old.";

pub const CLAUDE_PROMPT: &str = "Please read my latest GoghMode drawing at ~/Pictures/GoghMode/drawings/latest.svg and ~/Pictures/GoghMode/drawings/latest.json, then describe what I drew. If image inspection is available, also inspect ~/Pictures/GoghMode/drawings/latest.png. Older versions wrote to drawings/latest.svg, drawings/latest.json and drawings/latest.png in the current directory, so if those exist too, compare their modification times and use whichever is newer. Tell me if the drawing you read is more than a few hours old.";

pub fn prompt_text(target: PromptTarget) -> &'static str {
    match target {
        PromptTarget::Generic => GENERIC_PROMPT,
        PromptTarget::Claude => CLAUDE_PROMPT,
    }
}
