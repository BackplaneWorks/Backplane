// ReplyImages (Backplane/ReplyImages.swift): which lines of a reply are its pictures, how they group into carousels, the viewer's start, count and zoom.
// Compiled with ReplyImages.swift alone, outside the app (as scripts/mobile/ios-unit-tests.sh runs the others).
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }
let M = ReplyImages.mark

// The reply the phone showed as links: a bold caption, then a path on each line, twice. `images` is the hub's list for it (mimg.rs
// captioned_runs_of_paths_are_all_pictures).
let d = "/home/user/proj/build/shots"
let shots = ["before-project", "after-project", "after-project-boards", "before-picker", "after-picker"].map { "\(d)/\($0).png" }
let reply = "Both look right.\n\n**Project page, before and after:**\n\(shots[0])\n\(shots[1])\n\(shots[2])\n\n**Model picker, before and after:**\n\(shots[3])\n\(shots[4])"
let l = ReplyImages.layout(reply, images: shots)
check(l.slots == [Array(shots[0...2]), Array(shots[3...4])], "two carousels, one per caption", l.slots)
check(l.loose.isEmpty, "every picture has its slot", l.loose)
check(!l.text.contains("/home/user"), "no path left as text (a link)", l.text)
check(l.text.contains("**Project page, before and after:**\n\n\(M)0\n") && l.text.contains("**Model picker, before and after:**\n\n\(M)1\n"), "each slot right under its caption, a paragraph of its own", l.text)
check(l.text.hasPrefix("Both look right."), "prose stays", l.text)

// no pictures: the text as it came
check(ReplyImages.layout(reply, images: []) == ReplyImages.Layout(text: reply, slots: [], loose: []), "no images: unchanged")

// every form a picture-only line comes in: backticks, list, numbered list, bold, parentheses, trailing punctuation, markdown image, file://, ~/
let f = ["/a/1.png", "/a/2.jpg", "/a/3.png", "/a/4.gif", "/a/5.webp", "/a/6.png", "/a/7.png", "/a/8.png", "~/9.png"]
let forms = "Shots:\n`/a/1.png`\n- /a/2.jpg\n1. /a/3.png\n- **/a/4.gif**\n(/a/5.webp)\n/a/6.png.\n![shot](/a/7.png)\nfile:///a/8.png\n~/9.png"
let lf = ReplyImages.layout(forms, images: f)
check(lf.slots == [f], "one run of nine forms is one carousel", lf.slots)
check(lf.text == "Shots:\n\n\(M)0\n", "only the caption stays", lf.text)

// two pictures on one line, and blank lines inside a run
let l2 = ReplyImages.layout("/a/1.png /a/2.png\n\n\n/a/3.png\n\nAfter.", images: ["/a/1.png", "/a/2.png", "/a/3.png"])
check(l2.slots == [["/a/1.png", "/a/2.png", "/a/3.png"]], "blank lines do not split a run", l2.slots)
check(l2.text.hasSuffix("After."), "text after the run stays", l2.text)

// a sentence naming a picture keeps its words; the picture is loose (drawn under that block)
let l3 = ReplyImages.layout("See /a/1.png for the board.\n/a/2.png", images: ["/a/1.png", "/a/2.png"])
check(l3.text.hasPrefix("See /a/1.png for the board."), "the sentence stays", l3.text)
check(l3.slots == [["/a/2.png"]] && l3.loose == ["/a/1.png"], "only the bare line is a slot", (l3.slots, l3.loose))

// a path the hub did not list (not a picture of this reply) keeps its line
let l4 = ReplyImages.layout("/a/1.png\n/a/other.png", images: ["/a/1.png"])
check(l4.slots == [["/a/1.png"]] && l4.text.contains("/a/other.png"), "an unlisted path is text", l4.text)

// the folder form: a line naming the folder, then bare names
let l5 = ReplyImages.layout("Shots in `/a/shots/`:\n- one.png\n- two.png\n- notes.txt", images: ["/a/shots/one.png", "/a/shots/two.png"])
check(l5.slots == [["/a/shots/one.png", "/a/shots/two.png"]], "names under a folder are its pictures", l5.slots)
check(l5.text.contains("Shots in `/a/shots/`:") && l5.text.contains("- notes.txt") && !l5.text.contains("one.png"), "the folder line and other files stay", l5.text)

// separate runs are separate carousels; a picture named again alone is not drawn twice
let l6 = ReplyImages.layout("/a/1.png\nThen:\n/a/2.png\n/a/1.png\nAgain:\n/a/1.png", images: ["/a/1.png", "/a/2.png"])
check(l6.slots == [["/a/1.png"], ["/a/2.png"]], "a run each, each picture once", l6.slots)
check(l6.text.components(separatedBy: String(M)).count == 3 && l6.text.hasSuffix("Again:"), "the third run has nothing new: no slot", l6.text)

// code: a fenced block keeps its lines
let l7 = ReplyImages.layout("```sh\nopen /a/1.png\n/a/1.png\n```\n/a/2.png", images: ["/a/2.png"])
check(l7.text.hasPrefix("```sh\nopen /a/1.png\n/a/1.png\n```") && l7.slots == [["/a/2.png"]], "fenced lines untouched", l7.text)

// the marks themselves: only a real slot's
check(ReplyImages.slot("\(M)1", count: 2) == 1 && ReplyImages.slot(" \(M)0\n", count: 1) == 0, "a slot's paragraph")
check(ReplyImages.slot("\(M)2", count: 2) == nil && ReplyImages.slot("\(M)x", count: 2) == nil && ReplyImages.slot("text", count: 2) == nil, "anything else is no slot")
check(!ReplyImages.layout("odd \(M)0 text\n/a/1.png", images: ["/a/1.png"]).text.contains("odd \(M)"), "a mark in the reply itself is dropped")

// names, counts, the viewer's start and the double tap
check(ReplyImages.name("/a/b/shot.png") == "shot.png" && ReplyImages.name("~/x.jpg") == "x.jpg" && ReplyImages.name("C:\\w\\y.png") == "y.png", "file names")
check(ReplyImages.counter(0, of: 5) == "1 / 5" && ReplyImages.counter(4, of: 5) == "5 / 5" && ReplyImages.counter(9, of: 5) == "5 / 5" && ReplyImages.counter(0, of: 1) == "", "counts")
check(ReplyImages.start(shots[3], in: shots) == 3 && ReplyImages.start("/nope.png", in: shots) == 0, "the viewer starts at the picture tapped")
check(ReplyImages.doubleTap(current: 1, min: 1, max: 6) == 2.5 && ReplyImages.doubleTap(current: 3, min: 1, max: 6) == 1 && ReplyImages.doubleTap(current: 0.5, min: 0.5, max: 1) == 1, "double tap in, out, capped")
check(ReplyImages.zoomLabel(current: 1, min: 1) == "1.0x" && ReplyImages.zoomLabel(current: 1.25, min: 0.5) == "2.5x" && ReplyImages.zoomLabel(current: 2, min: 0) == "1.0x", "zoom label")

print("\(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
