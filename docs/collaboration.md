# Real-time collaboration: design

Status: implemented in `crates/collab` and `crates/engine/src/collab.rs` (#25). Commands are
`collab.host`, `collab.join`, `collab.leave`, `collab.status` and `collab.peers`. In the window they
are under File › Collaborate….

## What has to be synced

A ScriptCraft document is a `Script`: title page fields, per-script `Settings` (element
formats, page setup, revision sets, locks…), and an ordered `Vec<Para>`. Each paragraph has an
element, its text (no newlines, ever), and data anchored to byte ranges of that text: style spans,
script notes, production tags, revision marks. Alternates, scene numbers, summaries, card colours and
locks also live on paragraphs, and new features keep adding fields (v0.4 adds structure points,
beats, bookmarks and tracked changes in parallel with this work).

Everything that changes the document is a command (`Session::run`), but there are more than 130
commands and the list grows. **Sync therefore works on the document, not on commands.** Two scripts
are compared and merged; no part of collaboration knows what `scene.omit` or `tag.add` mean.

## OT, CRDT or something else?

**Operational transform (OT)** needs every operation type, and a transform function for each
pair of types: insert/delete text, split, join, set element, move scene, set note, and so on. That
doesn't scale to 130 commands written by several people, and a missing transform pair silently
corrupts documents.

**A CRDT** (e.g. an RGA/Fugue sequence of characters with tombstones, plus LWW registers for
attributes) gives convergence without a server, but the paragraph model would have to be rebuilt on
top of it. Every byte-offset feature would have to use CRDT positions. Undo, revision marks, layout
and file formats all assume plain `String`s and `Vec<Para>`. It also means unbounded tombstone
growth, or garbage collection that needs coordination anyway. It's the right tool for offline-first,
peer-to-peer editing. ScriptCraft's goal here is live sessions on one network.

**Chosen: differential synchronisation with a central sequencer.** This follows Neil Fraser's
*Differential Synchronization*, 2009, and is how many "host a session" editors work:

- One window **hosts**. Its script is the authoritative document, with a version number. Every
  change gets the next version, so there is a single total order and no merge ever has to be undone.
- Each **client** keeps a *shadow*: the last host version it received. When its own script differs
  from the shadow, it sends a **patch** (paragraph-level diff: replace paragraphs `[at, at+del)`
  with these paragraphs, plus title or settings if they changed) against that version.
- The host rebuilds the client's script (`shadow + patch`) and **three-way merges** it into the
  current document. The base is the version the client edited. The host gives the result the next
  version and broadcasts the diff from the previous version to everyone, the sender included.
- Clients apply the broadcast to their shadow and rebase their own unsent typing onto it, with the
  same three-way merge. A client has **at most one edit in flight**. Further typing waits until the
  host's update that contains it arrives. That stops an edit from being sent twice.

Because only the host merges, and every client's script is the host's document plus that client's
not-yet-acknowledged typing, all windows **converge**. When nobody is typing, every script equals
the host's.

### The three-way merge (`collab::merge3(base, ours, theirs)`)

`ours` is the host's document and `theirs` is the incoming edit.

1. **Title page and settings** are merged field by field (through their JSON form). A field changed
   on one side only takes that side's value. If both sides changed it, the host's value wins. New
   fields added later are merged with no code change. Structure points after the last scene merge
   the same way.
   The **beat board** merges beat by beat (by id): each beat field by field, a beat deleted on one
   side and untouched on the other is deleted, and new beats from both sides are kept. When both
   sides add a beat under the same new id, the incoming one is renumbered and its scene links
   follow. New `Script` fields fail to compile in `diff`/`merge3` until they are carried.
2. **Paragraphs** are diffed `base→ours` and `base→theirs` (LCS on whole paragraphs after trimming
   the common start and end). Changes that touch different paragraphs are both applied. Two
   insertions at the same point are both kept, the host's first.
3. Where both sides changed **the same paragraphs**, the region is merged as text. Each region is
   written as its paragraphs joined by `\n` (paragraph text never contains `\n`). The
   character-level edits `theirs` made are then replayed onto `ours` through the normal edit
   primitives (`delete_range`, `insert_styled`, `split`), so styles, notes, tags and revision marks
   shift exactly as they do when you type. Results:
   - Two people typing in one paragraph: both texts are kept.
   - One splits a paragraph while the other types in it: both changes are kept.
   - Overlapping deletes: the text is deleted once.
   - Text the host deleted is never resurrected by a concurrent delete.
   - A concurrent insertion inside a deleted range survives.
4. The other paragraph fields, where both edited the same paragraphs (element, scene number,
   alternates…), are merged field by field like settings.
5. If one side **deleted whole paragraphs** that the other side edited, the deletion wins. This is
   the one case where a concurrent edit can be lost.

The merge is deterministic and has a fast path: if one side is unchanged, the other side is returned
untouched. Text work is limited to the changed region. Large rewrites fall back to replacing the
region instead of running quadratic diffs.

## Undo

Undo stays **local**: it undoes your own edits, not other people's. The session keeps whole-script
snapshots for undo. When a remote change is applied, every snapshot is rebased by the same merge,
`merge3(before, after, snapshot)`. Undo then restores your earlier text with everyone else's later
changes still in it. While a session is shared, history is capped at 100 steps to bound that work.
The undo itself is an ordinary local change and is sent like typing. A remote change arriving
mid-word ends the current typing run, so while others are typing an undo step can be shorter than
usual.

## Carets, selections and presence

- **Remote changes:** your caret and selection are carried through the change. Characters inserted
  before them push them along; a split moves them to the new paragraph.
- **Presence:** every window sends its name and selection. The host relays the list of peers (id,
  name, colour, selection).
- **What you see:** other people's carets and selections are drawn in the page view in their
  colour, with a name tag.
- **Where it shows:** the status bar shows how many people are in the session, and the Collaborate
  window lists them.

## Transport and security

- **Connection:** newline-delimited JSON over TCP. Lines are capped at 32 MB; anything longer, or
  anything malformed, is dropped and never panics.
- **Binding:** the host binds `127.0.0.1` by default. `lan: true` binds every interface so others on
  the same network can join.
- **The key:** the host generates a random 128-bit **session key** (hex). A joiner's first message
  must carry it, compared in constant time. Any other first message, or a wrong key, closes the
  connection. The host shares the key with collaborators by any channel it trusts.
- **Reconnect:** if the host goes away, clients keep their script and drop to "disconnected"; nothing
  is lost. Rejoining replaces the local script with the host's copy (after a warning in the window
  if it has unsaved changes). There is no automatic reconnect.
- **Missing history:** the host keeps the last 64 versions. An edit based on an older version is
  refused with a **resync**: the client receives the whole document, rebases its unsent typing
  onto it, and sends it again.
- **Browser build:** it can't open TCP sockets, so the `collab.*` commands report that collaboration
  isn't available there.

## Out of scope (for now)

- An internet relay or NAT traversal. Use a VPN, an SSH tunnel or a LAN.
- Encryption. The key keeps strangers out, but traffic is plain text on the network. Use only trusted
  networks or a tunnel.
- Offline editing and merging later. A client that disconnects keeps working alone, and its changes
  are not merged back when it rejoins.
- Character-level ownership or per-author colours in the text. Track changes (#24) covers authorship.
- Host migration: if the host leaves, the session ends.
