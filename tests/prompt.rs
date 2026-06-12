#[path = "../src/prompt.rs"]
mod prompt;

use prompt::{prompt_text, PromptTarget};

#[test]
fn prompts_include_spotlight_app_fallback_directory() {
    for prompt in [
        prompt_text(PromptTarget::Generic),
        prompt_text(PromptTarget::Claude),
    ] {
        assert!(prompt.contains("~/Pictures/GoghMode/drawings/latest.svg"));
        assert!(prompt.contains("~/Pictures/GoghMode/drawings/latest.json"));
        assert!(prompt.contains("~/Pictures/GoghMode/drawings/latest.png"));
    }
}

#[test]
fn generic_prompt_names_all_drawing_files() {
    let prompt = prompt_text(PromptTarget::Generic);

    assert!(prompt.contains("drawings/latest.svg"));
    assert!(prompt.contains("drawings/latest.json"));
    assert!(prompt.contains("drawings/latest.png"));
}

#[test]
fn claude_prompt_uses_claude_specific_wording() {
    let prompt = prompt_text(PromptTarget::Claude);

    assert!(prompt.contains("Please read drawings/latest.svg"));
}

#[test]
fn prompts_do_not_contain_shell_execution_metacharacters() {
    for prompt in [
        prompt_text(PromptTarget::Generic),
        prompt_text(PromptTarget::Claude),
    ] {
        for forbidden in [";", "&&", "|", "`", "$(", "<", ">"] {
            assert!(
                !prompt.contains(forbidden),
                "prompt contained {forbidden}: {prompt}"
            );
        }
    }
}
