# Typing parity: Final Draft-style element flow

Status: v0.1 · pinned by `crates/engine/src/tests_typing.rs` (keys go through the same commands as
the keyboard: `text.insert`, `text.enter`, `text.tab`, `text.shiftTab`, `text.backspace`).
The default tables live in `ElementFormat::default_for` (`crates/doc/src/format.rs`); every
script can change its Enter, Tab and empty-Tab targets per element in **Format › Element
Settings** (`settings.element`). Change behaviour, this doc and the tests together.

Final Draft's behaviour was not observed directly for this release (it is not installed on the dev
machine). The flow below follows its widely documented conventions; rows marked *ours* are
ScriptCraft's own choice where the convention is unclear.

## Enter

Enter at the **end** of a non-empty paragraph starts a new paragraph:

| In | New paragraph |
|---|---|
| Scene Heading | Action |
| Action | Action |
| Character | Dialogue |
| Parenthetical | Dialogue |
| Dialogue | Action |
| Transition | Scene Heading |
| Shot | Action |
| General | General |

- Enter **in the middle** of a paragraph splits it; the tail keeps the same element.
- Enter at the **start** of a non-empty paragraph inserts an empty paragraph of the same element above.
- Enter just before a parenthetical's closing `)` leaves the parenthetical (new Dialogue).
- Enter in an **empty** paragraph changes its element instead of adding a paragraph (*ours*;
  Final Draft shows an element list here): Action → Scene Heading, Scene Heading → Action,
  Character → Action, Dialogue → Action, Parenthetical → Dialogue, Transition → Scene Heading,
  Shot → Action, General → Action.

## Tab

| In | Tab at the end of a non-empty paragraph | Tab in an empty paragraph (or mid-text) |
|---|---|---|
| Action | new **Character** | becomes **Character** |
| Character | new Parenthetical | becomes **Transition** |
| Transition | new Scene Heading | becomes Scene Heading |
| Scene Heading | new Action | becomes Action |
| Dialogue | new **Parenthetical** | becomes Parenthetical |
| Parenthetical | new Dialogue | becomes Dialogue |
| Shot | new Action | becomes Action |
| General | new Character | becomes Action |

**Decision (#4):** an empty Action + Tab goes to **Character** (Final Draft's documented
convention), not Transition. Scripts that prefer Transition set Action's *Tab when empty* to
Transition in Element Settings (`settings.element {element: "action", tabEmpty: "transition"}`).
Issue #32 will re-check this against Final Draft 13.

So the classic exchange is typed as: *Action* → Tab → *CHARACTER* → Enter → *Dialogue* → Enter →
(empty Action) → Tab → *CHARACTER*… and a transition from an empty Action is Tab, Tab
(Action → Character → Transition).

Shift+Tab changes the element backwards: Character → Action, Transition → Character, Scene Heading
→ Transition, Parenthetical → Dialogue, Dialogue → Character, Action → Scene Heading.

## Automatic formatting

- **Capitals:** Scene Heading, Character, Transition and Shot are typed (and converted) in capitals.
- **Parentheses:** a paragraph that becomes an empty Parenthetical gets `()` with the caret inside.
  Typing `(` right after the automatic `(` or `)` right before the automatic `)` moves over it.
  Backspace in `()` clears both.
- **Scene-heading detection:** typing `int. `, `ext. `, `int./ext. `, `i/e ` or `est. ` at the start
  of an Action paragraph turns it into a Scene Heading.
- **(CONT'D):** when the same character speaks again after action in the same scene, the cue shows
  `(CONT'D)` (display only; Document › Automatic Character (CONT'D)).

## SmartType

Suggestions appear under the caret while the caret is at the end of a non-empty paragraph:

| Element | Offers |
|---|---|
| Character | names already used in the script; after ` (`: `(V.O.)`, `(O.S.)`, `(O.C.)`, `(CONT'D)` |
| Scene Heading | `INT. `, `EXT. `, `INT./EXT. `, `EST. `, `I/E. `; then locations already used; after ` - `: times of day (DAY, NIGHT, CONTINUOUS, …) and times already used |
| Transition | CUT TO:, DISSOLVE TO:, SMASH CUT TO:, MATCH CUT TO:, … |

↑/↓ choose, Enter accepts (for a Character it also moves on to Dialogue), Esc dismisses until the
next keystroke. Commands: `smarttype.suggest`, `smarttype.accept {index}`.

## Undo

Consecutive typing is one undo step; any other command ends it. Every other edit is its own step.
