```text
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⣄⠀⠀⠀⣦⣤⣾⣿⠿⠛⣋⣥⣤⣀⠀⠀⠀⠀
⠀⠀⠀⠀⡤⡀⢈⢻⣬⣿⠟⢁⣤⣶⣿⣿⡿⠿⠿⠛⠛⢀⣄⠀
⠀⠀⢢⣘⣿⣿⣶⣿⣯⣤⣾⣿⣿⣿⠟⠁⠄⠀⣾⡇⣼⢻⣿⣾      palimpsest by @parazeeknova
⣰⠞⠛⢉⣩⣿⣿⣿⣿⣿⣿⣿⣿⠋⣼⣧⣤⣴⠟⣠⣿⢰⣿⣿
⣶⡾⠿⠿⠿⢿⣿⣿⣿⣿⣿⣿⣿⣈⣩⣤⡶⠟⢛⣩⣴⣿⣿⡟      git already knows everything.
⣠⣄⠈⠀⣰⡦⠙⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣟⡛⠛⠛⠁      palimpsest just makes it visible.
⣉⠛⠛⠛⣁⡔⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠥⠀⠀      local, native, free.
⣭⣏⣭⣭⣥⣾⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡿⢠⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
```

<div align="center">
  <h3><strong>P A L I M P S E S T</strong> &nbsp;<sub><em>パリンプセスト &nbsp;·&nbsp; 古迹重写 &nbsp;·&nbsp; पालिम्पसेस्ट</em></sub></h3>
  <sub><em>"native blazing fast git client, no electron, no subscription, no cloud. just your repository."</em></sub>
  <br /><br />
  <!-- <img src="https://img.shields.io/badge/rust-1.78+-orange?style=flat-square&logo=rust" />
  <img src="https://img.shields.io/badge/egui-0.31-blue?style=flat-square" />
  <img src="https://img.shields.io/badge/license-MIT-green?style=flat-square" />
  <img src="https://img.shields.io/badge/binary-~5MB-purple?style=flat-square" />
  <img src="https://img.shields.io/badge/webview-zero-red?style=flat-square" /> -->
</div>

## you found this. welcome to the excavation.

a [palimpsest](https://en.wikipedia.org/wiki/Palimpsest) is a manuscript that was scraped clean and written over. monks did it with [parchment](https://en.wikipedia.org/wiki/Parchment) because parchment was expensive. the old text doesn't disappear though it bleeds through. centuries later, historians can still read what was erased.

your git repository is a palimpsest. every commit written over the last one. nothing truly gone. the whole history showing through if you know how to look.

this app is named after that. not because the name is clever okay, maybe a little because the name is clever but because it's the most honest description of what git actually is, and what this tool actually does: it makes the old text visible.

## why this exists

here's the thing about git GUIs. there are a lot of them. and most of them have the same problem: they're either too much or not enough for my need, and the ones that are just right eventually figure out that "just right" is a monetizable position.

the story of palimpsest starts with a very small, very specific annoyance the kind that sits quietly in the back of your head for months before you finally do something stupid about it, like writing a native desktop application in rust instead of just paying the subscription.

the annoyance was this: git already knows everything. it already computed the graph. it already stored the diffs, the tree, the blame, the log. all of that information exists, on your disk, right now, for free. git is one of the most information-rich tools ever built and it hands all of it to you through a command line that most people use for four commands.

so why does seeing a picture of that information cost money. the terminal is nice `git log --oneline --graph` exists, it works, nobody is arguing otherwise. but the moment you have three developers, six branches, two hotfixes, and a merge conflict that happened four days ago, the terminal stops being a tool and starts being a riddle. a graph you can actually look at, click around in, and drag branches across to merge or rebase that's not a luxury. that's just a reasonable thing to want from software in 2026.

not the information. the picture.

that question sat there long enough that the only reasonable response was to build the picture myself. native, local, fast, free. no subscription for a graph that git already drew i'm just rendering it.

palimpsest is what came out. a native git GUI built from scratch in rust, using egui for the interface and libgit2 for the git layer. no electron. no chromium bundled inside like a russian doll of ram usage. no cloud, no account, no emails. it weighs about 5MB. it reads your existing config. it gets out of your way.

## what it does

**commit graph** — your full branch history rendered as a live directed acyclic graph. every branch gets a lane, every merge gets connected, every commit is a node you can click. color-coded per branch. it looks like the thing you draw on whiteboards when you're explaining git to someone and trying to look calm about it.

**diff viewer** — click any commit and see exactly what changed. syntax highlighted across 200+ languages. line by line, added in green, removed in red, the way diffs have looked since before most of us were writing code. inline word-level highlighting for when you need to know not just which line changed but which three characters moved within it.

**file tree** — browse the full working tree with git status indicators on every file. modified, added, deleted, untracked, all visible at a glance. click a file to open its diff. the whole structure of a branch, readable without running a single command.

**drag-to-merge** — grab a branch label. drop it onto another. palimpsest fires the merge. conflicts surface immediately. clean merges complete silently. no wizard, no confirmation modal asking if you're really sure, no checkbox about squashing. just the operation you asked for.

**remote management** — fetch, pull, push. add and remove remotes. view remote-tracking branches alongside your local ones in the same graph. the full picture, local and remote, in one place. your ssh keys and credential config are read automatically, the setup you already did once is the setup palimpsest uses.

**reads your existing config** — ssh keys, gpg signing, your `.gitconfig` user info, your credential helper. palimpsest finds what's already there and uses it. you've set all of this up once. you shouldn't have to do it again for a GUI.

## how it works (the interesting part)

palimpsest doesn't shell out to git. it talks directly to libgit2, the same C library that powers github, gitlab, and most serious git tooling, through rust's `git2` crate, statically linked, no system dependency required.

the commit graph is laid out using a greedy lane assignment algorithm written from scratch. `repo.revwalk()` walks the DAG, assigns each branch to a lane, resolves crossings, and hands the result to egui's painter which draws it as circles and lines on a gpu-accelerated canvas. no SVG. no DOM. just geometry.

diffs come from libgit2 as raw unified patch strings, parsed by `diffy`, rendered line by line with `syntect` doing the syntax highlighting token pass. word-level inline diffs are a second pass with `similar`'s inline feature, which finds exactly which spans changed within a modified line.

the file tree is `egui-arbor` a blender-outliner-style tree widget with git status badges injected per node from the index state. drag and drop is egui-native: drag payload attached to a branch label widget, drop target detection on other branch labels, merge fired on `drop_released`.

icons are phosphor via `egui-phosphor`, embedded as font glyphs. the whole thing compiles to a single binary with no runtime dependencies beyond the system GPU driver.

## what it doesn't do (for now)

palimpsest does not have a code review interface. it does not manage pull requests, issues, or anything that lives on a server rather than in a `.git` folder. it does not have an insights tab, a team inbox, or an AI assistant that summarizes your commits in a way that makes them sound more impressive than they were.

it is a git visualizer and a local operation tool the graph, the diffs, the tree, the merges, the remotes. the part of a git GUI that should have always been free, extracted, made small, and handed back to you.

## contributing

open an issue if something is broken. open a pr if you fixed it. if you're adding a feature, keep it grounded in what the tool actually is, a local-first git GUI, not a platform.

the codebase is intentionally small. every module does one thing. if a PR makes a module do two things, we'll talk about it.

if you think the name is too pretentious for a git GUI: you're right. i named it at 2am after reading about medieval manuscript restoration. the bar for restraint was low. i have no regrets.

what i will say is this: git is yours. your history is yours. every commit you ever made is sitting on your disk right now, complete and permanent, asking nothing from you. a tool that lets you see it really see it, the shape of it, the layers of it should be too.
i've used a lot of git clients trying to find that. git-cola, gittyup, smartgit, gitfourchette each one got something right and something wrong. too minimal, too enterprise, too ugly, too slow, too much month-to-month for a graph view. none of them fit quite right. so in the grand tradition of developers who couldn't find the thing they wanted: i built it.
that's palimpsest. the one that fit.

— harsh / @parazeeknova

<div align="center">
  <sub>MIT licensed · open source · free to use </sub>
</div>
