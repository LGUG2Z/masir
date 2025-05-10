# masir

Focus Follows Mouse for Windows.

<p>
  <a href="https://techforpalestine.org/learn-more">
    <img alt="Tech for Palestine" src="https://badge.techforpalestine.org/default">
  </a>
  <img alt="GitHub Workflow Status" src="https://img.shields.io/github/actions/workflow/status/LGUG2Z/masir/.github/workflows/windows.yaml">
  <img alt="GitHub all releases" src="https://img.shields.io/github/downloads/LGUG2Z/masir/total">
  <img alt="GitHub commits since latest release (by date) for a branch" src="https://img.shields.io/github/commits-since/LGUG2Z/masir/latest">
  <a href="https://discord.gg/mGkn66PHkx">
    <img alt="Discord" src="https://img.shields.io/discord/898554690126630914">
  </a>
  <a href="https://github.com/sponsors/LGUG2Z">
    <img alt="GitHub Sponsors" src="https://img.shields.io/github/sponsors/LGUG2Z">
  </a>
  <a href="https://ko-fi.com/lgug2z">
    <img alt="Ko-fi" src="https://img.shields.io/badge/kofi-tip-green">
  </a>
  <a href="https://notado.app/feeds/jado/software-development">
    <img alt="Notado Feed" src="https://img.shields.io/badge/Notado-Subscribe-informational">
  </a>
  <a href="https://www.youtube.com/channel/UCeai3-do-9O4MNy9_xjO6mg?sub_confirmation=1">
    <img alt="YouTube" src="https://img.shields.io/youtube/channel/subscribers/UCeai3-do-9O4MNy9_xjO6mg">
  </a>
</p>

_masir_ is a focus follows mouse daemon for Microsoft Windows 11 and above.

_masir_ allows you to focus an application window by moving your mouse over it, without requiring mouse clicks or
touchpad taps.

_masir_ does not have a dependency on any specific external software or tiling window manager.

_masir_ has an additional integration with [komorebi](https://github.com/LGUG2Z/komorebi) to ensure that only windows
managed by the tiling window manager are eligible to be focused. Integrations with other tiling window managers are
welcome (["Integrations"](#integrations).)

_masir_ is a free and educational source project, and one that encourages you to make charitable donations if you find
the software to be useful and have the financial means.

I encourage you to make a charitable donation to
the [Palestine Children's Relief Fund](https://pcrf1.app.neoncrm.com/forms/gaza-recovery) or contributing to
a [Gaza Funds campaign](https://gazafunds.com) before you consider sponsoring me on GitHub.

[GitHub Sponsors is enabled for this project](https://github.com/sponsors/LGUG2Z). Unfortunately I don't have anything
specific to offer besides my gratitude and shout outs at the end of _komorebi_ live development videos and tutorials.

If you would like to tip or sponsor the project but are unable to use GitHub Sponsors, you may also sponsor
through [Ko-fi](https://ko-fi.com/lgug2z).

# Installation

```powershell
winget install LGUG2Z.masir
```

`masir` can be run as a standalone process, or launched alongside `komorebi`
using the `komorebic start` command with the `--masir` flag.

# Contribution Guidelines

If you would like to contribute to `masir` please take the time to carefully read the guidelines below.

## Commit hygiene

- Flatten all `use` statements
- Run `cargo +stable clippy` and ensure that all lints and suggestions have been addressed before committing
- Run `cargo +nightly fmt --all` to ensure consistent formatting before committing
- Use `git cz` with
  the [Commitizen CLI](https://github.com/commitizen/cz-cli#conventional-commit-messages-as-a-global-utility) to prepare
  commit messages
- Provide **at least** one short sentence or paragraph in your commit message body to describe your thought process for the
  changes being committed

## License

`masir` is licensed under the [Komorebi 2.0.0 license](./LICENSE.md), which
is a fork of the [PolyForm Strict 1.0.0
license](https://polyformproject.org/licenses/strict/1.0.0). On a high level
this means that you are free to do whatever you want with `masir` for
personal use other than redistribution, or distribution of new works (i.e.
hard-forks) based on the software.

Anyone is free to make their own fork of `masir` with changes intended
either for personal use or for integration back upstream via pull requests.

The [Komorebi 2.0.0 License](./LICENSE.md) does not permit any kind of
commercial use.

### Contribution licensing

Contributions are accepted with the following understanding:

- Contributed content is licensed under the terms of the 0-BSD license
- Contributors accept the terms of the project license at the time of contribution

By making a contribution, you accept both the current project license terms, and that all contributions that you have
made are provided under the terms of the 0-BSD license.

#### Zero-Clause BSD

```
Permission to use, copy, modify, and/or distribute this software for
any purpose with or without fee is hereby granted.

THE SOFTWARE IS PROVIDED “AS IS” AND THE AUTHOR DISCLAIMS ALL
WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES
OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE
FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY
DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN
AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT
OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

# Integrations

If you would like to create an integration between _masir_ and a tiling window manager, there is only one requirement:

- An updated list of window HWNDs known to and managed by the tiling window manager written to a file in a known
  location.

The path to this file of HWNDs can be passed with the `--hwnds` flag for testing purposes.

_masir_ will check for the presence of the HWND under the mouse cursor in this file when deciding if the window is
eligible to be focused.

Once testing is complete, native support for checking this file without requiring the `--hwnds` argument can be added
directly to `masir` (see `TODO: We can add checks for other window managers here`
in [`main.rs`](https://github.com/LGUG2Z/masir/blob/a35754a4a29538323bf248b4491f726e366f68bd/src/main.rs#L53)).
