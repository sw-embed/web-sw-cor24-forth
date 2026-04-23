//! Global Help dialog with three inner tabs: User Guide, Reference, Tutorial.
//!
//! Dismissal (X button, Esc key, click-outside) is handled by the parent
//! `App` component; this module is render-only.
//!
//! Content is embedded from `docs/*.md` via `include_str!`, parsed at render
//! time by `pulldown-cmark`, and injected as trusted HTML via
//! `Html::from_html_unchecked`. The markdown files are the source of truth
//! and remain browsable on GitHub. Styling for headings / tables / lists /
//! code-spans lives in `debugger.css` under the `.help-md` scope.

use pulldown_cmark::{Options, Parser, html};
use yew::prelude::*;
use yew::virtual_dom::VNode;

const USER_GUIDE_MD: &str = include_str!("../docs/user-guide.md");
pub(crate) const REFERENCE_MD: &str = include_str!("../docs/reference.md");
const TUTORIAL_MD: &str = include_str!("../docs/tutorial.md");

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HelpInner {
    UserGuide,
    Reference,
    Tutorial,
}

#[derive(Properties, PartialEq)]
pub struct HelpProps {
    /// Closing the overlay / clicking X / Esc all fire this.
    pub on_close: Callback<()>,
}

#[function_component(Help)]
pub fn help(props: &HelpProps) -> Html {
    let inner = use_state(|| HelpInner::UserGuide);

    let close_click = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };
    let stop_click = Callback::from(|e: MouseEvent| e.stop_propagation());

    let pick = |tab: HelpInner| {
        let inner = inner.clone();
        Callback::from(move |_: MouseEvent| inner.set(tab))
    };

    let active = *inner;
    let content_md = match active {
        HelpInner::UserGuide => USER_GUIDE_MD,
        HelpInner::Reference => REFERENCE_MD,
        HelpInner::Tutorial => TUTORIAL_MD,
    };
    let content_html = render_markdown(content_md);

    let tab_class = |tab: HelpInner| {
        if active == tab {
            "help-tab active"
        } else {
            "help-tab"
        }
    };

    html! {
        <div class="about-overlay" onclick={close_click.clone()}>
            <div class="help-dialog" onclick={stop_click}>
                <button class="about-close" aria-label="Close"
                        onclick={close_click.clone()}>{"\u{00d7}"}</button>
                <div class="help-tabs">
                    <button class={tab_class(HelpInner::UserGuide)}
                            onclick={pick(HelpInner::UserGuide)}>
                        {"User Guide"}
                    </button>
                    <button class={tab_class(HelpInner::Reference)}
                            onclick={pick(HelpInner::Reference)}>
                        {"Reference"}
                    </button>
                    <button class={tab_class(HelpInner::Tutorial)}
                            onclick={pick(HelpInner::Tutorial)}>
                        {"Tutorial"}
                    </button>
                </div>
                <div class="help-md">{ content_html }</div>
            </div>
        </div>
    }
}

/// Render CommonMark (+ tables) to HTML. We trust our own embedded docs,
/// so the parser output goes straight into the DOM via
/// `Html::from_html_unchecked`.
fn render_markdown(src: &str) -> Html {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(src, opts);
    let mut out = String::with_capacity(src.len() * 2);
    html::push_html(&mut out, parser);
    VNode::from_html_unchecked(AttrValue::from(out))
}

#[cfg(test)]
mod tests {
    use super::REFERENCE_MD;
    use crate::demos::{FIF_CORE_FILES, FIF_KERNEL_SRC, FOF_CORE_FILES, FOF_KERNEL_SRC};

    /// Extract Forth word names from a kernel.s source by walking
    /// `entry_XXX:` blocks and decoding their `.byte <flags_len>` /
    /// `.byte <comma-separated-ascii-bytes>` pair. Matches the dict
    /// header layout documented at the top of `forth-in-forth/kernel.s`.
    fn kernel_words(src: &str) -> Vec<String> {
        let mut words = Vec::new();
        let lines: Vec<&str> = src.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with("entry_") && line.ends_with(':') {
                // Scan up to ~6 following lines for the two `.byte` lines
                // that follow the `.word <link>` line.
                let mut byte_lines: Vec<&str> = Vec::new();
                for look in lines.iter().skip(i + 1).take(6) {
                    let t = look.trim();
                    if let Some(payload) = t.strip_prefix(".byte") {
                        byte_lines.push(payload.trim());
                        if byte_lines.len() == 2 {
                            break;
                        }
                    } else if t.starts_with("do_") && t.ends_with(':') {
                        break;
                    }
                }
                if byte_lines.len() == 2 {
                    let payload0 = strip_comment(byte_lines[0]);
                    if let Ok(flags_len) = parse_byte(payload0) {
                        let name_len = (flags_len & 0x3F) as usize;
                        let payload1 = strip_comment(byte_lines[1]);
                        let decoded: Option<Vec<u8>> = payload1
                            .split(',')
                            .map(|s| parse_byte(s.trim()))
                            .collect::<Result<Vec<_>, _>>()
                            .ok();
                        if let Some(bytes) = decoded
                            && bytes.len() >= name_len
                        {
                            let name: String =
                                bytes[..name_len].iter().map(|&b| b as char).collect();
                            words.push(name);
                        }
                    }
                }
            }
            i += 1;
        }
        words
    }

    fn strip_comment(s: &str) -> &str {
        match s.find(';') {
            Some(idx) => s[..idx].trim_end(),
            None => s.trim_end(),
        }
    }

    fn parse_byte(s: &str) -> Result<u8, std::num::ParseIntError> {
        let s = s.trim();
        if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            u8::from_str_radix(hex, 16)
        } else {
            s.parse::<u8>()
        }
    }

    /// Extract colon-defined words from a `core/*.fth` source by scanning
    /// for lines starting with `: <WORD>` (Forth convention — defs begin
    /// at column 0 in the core tier files).
    fn core_words(src: &str) -> Vec<String> {
        let mut words = Vec::new();
        for line in src.lines() {
            // Skip comments — the kernel's `\` parser treats them as such.
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix(':')
                && rest.starts_with(char::is_whitespace)
                && let Some(name) = rest.split_whitespace().next()
            {
                words.push(name.to_string());
            }
        }
        words
    }

    /// A word counts as "documented" if it appears in reference.md inside
    /// backticks (either as the first token of an entry: `` `WORD ( ... ) `` ,
    /// or fully bracketed like `` `WORD` ``). Loose enough to survive
    /// incidental formatting changes, tight enough to avoid matching common
    /// English substrings of a word like `AND`.
    fn is_documented(word: &str) -> bool {
        let needle = format!("`{word}");
        REFERENCE_MD.match_indices(&needle).any(|(i, _)| {
            let after = &REFERENCE_MD[i + needle.len()..];
            matches!(
                after.chars().next(),
                Some(' ') | Some('`') | Some('\t') | Some('\n') | None
            )
        })
    }

    #[test]
    fn every_kernel_word_is_documented() {
        let mut all: Vec<String> = Vec::new();
        all.extend(kernel_words(FIF_KERNEL_SRC));
        all.extend(kernel_words(FOF_KERNEL_SRC));
        for (_name, src) in FIF_CORE_FILES {
            all.extend(core_words(src));
        }
        for (_name, src) in FOF_CORE_FILES {
            all.extend(core_words(src));
        }
        all.sort();
        all.dedup();

        // Guard against the extractor itself silently breaking — if we
        // can't find a dozen well-known words in the kernels, the rest
        // of the test is meaningless.
        assert!(
            all.len() > 50,
            "extractor collected only {} words; expected > 50 across fif+fof kernels+core",
            all.len()
        );

        let missing: Vec<&String> = all.iter().filter(|w| !is_documented(w)).collect();
        assert!(
            missing.is_empty(),
            "{} word(s) missing from docs/reference.md — add entries for: {:?}",
            missing.len(),
            missing
        );
    }
}
