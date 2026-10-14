//! Script templates: the formats for features, TV and the stage.
//!
//! A template is element settings (indents, spacing, capitals, Enter/Tab flow), a page setup
//! and, for TV and stage, a starting structure of act breaks. These are ScriptCraft's own,
//! written from widely published formatting conventions:
//!
//! - **Feature film**: the standard screenplay format.
//! - **TV, single camera** (one-hour drama, single-camera comedy): screenplay format; each act
//!   starts on a new page with a centred act heading (`TEASER`, `ACT ONE`…).
//! - **TV, multi-camera** (sitcom): double-spaced dialogue, action in capitals, every scene on a
//!   new page, scenes lettered A, B, C instead of numbered.
//! - **Stage play**: character names centred, dialogue across the page, stage directions
//!   indented, acts on new pages and scenes centred.
//!
//! Act headings are General paragraphs; in these templates General is centred, in capitals
//! and starts a new page. Each act's first scene also carries an Act structure point, so acts
//! show in the outline and story map. The two are not linked: renaming one doesn't rename the
//! other, and the printed heading is the last paragraph of the scene before it.

use crate::{Align, Element, ElementFormat, Flag, Para, Pos, Script, Settings, StructureKind, StructurePoint};

/// A script template.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    Feature,
    TvSingleCamera,
    TvMultiCamera,
    StagePlay,
}

/// Most acts a template starts with.
pub const MAX_ACTS: usize = 7;

impl Template {
    pub const ALL: [Template; 4] = [Template::Feature, Template::TvSingleCamera, Template::TvMultiCamera, Template::StagePlay];

    pub fn id(self) -> &'static str {
        match self {
            Template::Feature => "feature",
            Template::TvSingleCamera => "tv-single-camera",
            Template::TvMultiCamera => "tv-multi-camera",
            Template::StagePlay => "stage-play",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Template::Feature => "Feature Film",
            Template::TvSingleCamera => "TV Drama / Single Camera",
            Template::TvMultiCamera => "TV Sitcom / Multi-Camera",
            Template::StagePlay => "Stage Play",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Template::Feature => "Standard screenplay format.",
            Template::TvSingleCamera => "Screenplay format with a teaser and acts, each starting on a new page.",
            Template::TvMultiCamera => "Double-spaced dialogue, action in capitals, every scene on a new page, scenes lettered A, B, C.",
            Template::StagePlay => "Centred character names, dialogue across the page, indented stage directions, acts on new pages.",
        }
    }

    /// The template named `s` (its id or name, any case; `tv`, `sitcom`, `multicam`, `play`…).
    pub fn parse(s: &str) -> Option<Template> {
        let k: String = s.trim().to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        Template::ALL
            .into_iter()
            .find(|t| {
                t.id().replace('-', "") == k || t.name().to_ascii_lowercase().chars().filter(char::is_ascii_alphanumeric).collect::<String>() == k
            })
            .or(match k.as_str() {
                "film" | "screenplay" | "movie" => Some(Template::Feature),
                "tv" | "singlecam" | "singlecamera" | "drama" | "onehour" | "tvdrama" => Some(Template::TvSingleCamera),
                "multicam" | "multicamera" | "sitcom" | "tvsitcom" => Some(Template::TvMultiCamera),
                "play" | "stage" | "theatre" | "theater" => Some(Template::StagePlay),
                _ => None,
            })
    }

    /// The default number of acts and whether the script opens with a teaser / cold open.
    pub fn default_acts(self) -> (usize, bool) {
        match self {
            Template::Feature => (0, false),
            Template::TvSingleCamera => (4, true),
            Template::TvMultiCamera => (2, true),
            Template::StagePlay => (2, false),
        }
    }

    /// The opening section's name (TV).
    fn teaser_name(self) -> &'static str {
        if self == Template::TvMultiCamera { "COLD OPEN" } else { "TEASER" }
    }

    /// Put this template's element settings, page setup and numbering into `st` (other script
    /// settings are kept).
    pub fn apply(self, st: &mut Settings) {
        let d = Settings::default();
        st.elements = d.elements;
        st.page = d.page.clone();
        st.scene_letters = false;
        let mut set = |e: Element, f: &dyn Fn(&mut ElementFormat)| {
            let mut x = st.elements.get(e);
            f(&mut x);
            st.elements.set(e, x);
        };
        let act_heading = |f: &mut ElementFormat| {
            f.align = Align::Center;
            f.uppercase = true;
            f.new_page = true;
            f.enter = Element::SceneHeading;
        };
        match self {
            Template::Feature => {}
            Template::TvSingleCamera => {
                set(Element::General, &act_heading);
            }
            Template::TvMultiCamera => {
                set(Element::SceneHeading, &|f| f.new_page = true);
                set(Element::Action, &|f| f.uppercase = true);
                set(Element::Dialogue, &|f| f.spacing = 2);
                set(Element::Shot, &|f| f.uppercase = true);
                set(Element::General, &act_heading);
                st.scene_letters = true;
            }
            Template::StagePlay => {
                set(Element::SceneHeading, &|f| {
                    f.align = Align::Center;
                    f.before = 2;
                });
                set(Element::Action, &|f| {
                    f.left = 3.5;
                    f.right = 7.5;
                });
                set(Element::Character, &|f| {
                    f.left = 1.5;
                    f.right = 7.5;
                    f.align = Align::Center;
                });
                set(Element::Parenthetical, &|f| {
                    f.left = 2.5;
                    f.right = 6.5;
                });
                set(Element::Dialogue, &|f| {
                    f.left = 1.5;
                    f.right = 7.5;
                });
                set(Element::General, &act_heading);
            }
        }
        if matches!(self, Template::TvSingleCamera | Template::TvMultiCamera) {
            st.page.header.left = "{title}".into();
        }
    }

    /// The starting paragraphs: act headings (TV and stage) and an empty first scene.
    pub fn starter(self, acts: usize, teaser: bool) -> Vec<Para> {
        let acts = acts.min(MAX_ACTS);
        let heading = |t: &str| {
            let mut p = Para::new(Element::General, t);
            let n = p.text.len();
            let mut st = p.styles();
            for x in st.iter_mut().take(n) {
                *x = x.with(Flag::Underline, true);
            }
            let text = p.text.clone();
            p.set_text_styled(&text, &st, true);
            p
        };
        let scene = |t: &str| Para::new(Element::SceneHeading, t);
        // The act's first scene also opens an Act structure point, so the act shows in the
        // outline and story map (the centred heading is what prints).
        let act_scene = |t: &str, act: &str| {
            let mut p = scene(t);
            p.structure.push(StructurePoint::new(StructureKind::Act, &title_case(act), "", None));
            p
        };
        let mut out = Vec::new();
        match self {
            Template::Feature => {
                out.push(Para::new(Element::Transition, "FADE IN:"));
                out.push(scene(""));
            }
            Template::TvSingleCamera | Template::TvMultiCamera => {
                if teaser {
                    out.push(heading(self.teaser_name()));
                    out.push(act_scene("", self.teaser_name()));
                }
                for a in 1..=acts {
                    let name = format!("ACT {}", act_word(a));
                    out.push(heading(&name));
                    out.push(act_scene("", &name));
                }
                if out.is_empty() {
                    out.push(scene(""));
                }
            }
            Template::StagePlay => {
                for a in 1..=acts.max(1) {
                    let name = format!("ACT {}", act_word(a));
                    out.push(heading(&name));
                    out.push(act_scene("SCENE 1", &name));
                    out.push(Para::new(Element::Action, ""));
                }
            }
        }
        out
    }

    /// A new script from this template, with the caret's natural first position (the first
    /// empty paragraph).
    pub fn script(self, acts: Option<usize>, teaser: Option<bool>) -> (Script, Pos) {
        let (da, dt) = self.default_acts();
        let paras = self.starter(acts.unwrap_or(da), teaser.unwrap_or(dt));
        let mut s = Script { paras, ..Script::default() };
        self.apply(&mut s.settings);
        let _ = s.normalize();
        let first = s.paras.iter().position(|p| p.text.is_empty()).unwrap_or(0);
        let off = s.paras.get(first).map_or(0, Para::len);
        (s, Pos::new(first, off))
    }

    /// Which template a script's settings match, if any.
    pub fn detect(st: &Settings) -> Option<Template> {
        Template::ALL.into_iter().find(|t| {
            let mut x = st.clone();
            t.apply(&mut x);
            x.elements == st.elements && x.scene_letters == st.scene_letters
        })
    }
}

/// `ACT ONE` → `Act One`, `COLD OPEN` → `Cold Open`.
fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c.flat_map(char::to_lowercase)).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `1` → `ONE` … (`ACT 8` past seven).
fn act_word(n: usize) -> String {
    ["ONE", "TWO", "THREE", "FOUR", "FIVE", "SIX", "SEVEN"].get(n.wrapping_sub(1)).map_or_else(|| n.to_string(), |w| (*w).to_string())
}

/// Scene letter `n` (from 1): A … Z, then AA, BB … ZZ, then AAA…
pub fn scene_letter(n: usize) -> String {
    let n = n.max(1) - 1;
    let c = char::from(b'A' + (n % 26) as u8);
    std::iter::repeat_n(c, (n / 26 + 1).min(8)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters() {
        assert_eq!(scene_letter(1), "A");
        assert_eq!(scene_letter(26), "Z");
        assert_eq!(scene_letter(27), "AA");
        assert_eq!(scene_letter(53), "AAA");
        assert_eq!(scene_letter(0), "A");
        assert!(scene_letter(usize::MAX).len() <= 8);
    }

    #[test]
    fn parse_names() {
        for t in Template::ALL {
            assert_eq!(Template::parse(t.id()), Some(t));
            assert_eq!(Template::parse(t.name()), Some(t));
        }
        assert_eq!(Template::parse("Sitcom"), Some(Template::TvMultiCamera));
        assert_eq!(Template::parse("nope"), None);
    }

    #[test]
    fn starters_and_detection() {
        for t in Template::ALL {
            let (s, caret) = t.script(None, None);
            assert_eq!(Template::detect(&s.settings), Some(t), "{t:?}");
            assert!(s.paras.get(caret.para).is_some());
        }
        let (s, _) = Template::TvSingleCamera.script(Some(5), Some(true));
        let heads: Vec<&str> = s.paras.iter().filter(|p| p.element == Element::General).map(|p| p.text.as_str()).collect();
        assert_eq!(heads, ["TEASER", "ACT ONE", "ACT TWO", "ACT THREE", "ACT FOUR", "ACT FIVE"]);
        assert!(s.paras[0].spans.iter().any(|sp| sp.style.get(Flag::Underline)));
        // Each act (and the teaser) is also a structure point on its first scene.
        let acts: Vec<String> = s.structure_points().iter().map(|(_, p)| p.title.clone()).collect();
        assert_eq!(acts, ["Teaser", "Act One", "Act Two", "Act Three", "Act Four", "Act Five"]);
        assert!(s.structure_points().iter().all(|(_, p)| p.kind == StructureKind::Act));
        let (st, _) = Template::StagePlay.script(Some(2), None);
        assert_eq!(st.structure_points().len(), 2);
        assert!(Template::Feature.script(None, None).0.structure_points().is_empty());
        let (m, _) = Template::TvMultiCamera.script(Some(99), Some(false));
        assert_eq!(m.paras.iter().filter(|p| p.element == Element::General).count(), MAX_ACTS);
        assert!(m.settings.scene_letters && m.settings.elements.get(Element::Dialogue).spacing == 2);
        assert!(m.settings.elements.get(Element::Action).uppercase && m.settings.elements.get(Element::SceneHeading).new_page);
    }
}
