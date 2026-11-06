# Sharing 0.3.0

Install the tagged release with Rust 1.88 or newer:

```sh
cargo install --git https://github.com/finngi/tics --tag v0.3.0 --locked --force
tics --version
```

The [GitHub release](https://github.com/finngi/tics/releases/tag/v0.3.0) also provides prebuilt
executables and a source package. The handoff ZIP includes these packages, installation
instructions, a sample configuration and a GitHub workflow.

## Install a prebuilt executable

Choose `tics-0.3.0-macos-universal.tar.gz` for an Apple Silicon or Intel Mac, or
`tics-0.3.0-linux-x86_64.tar.gz` for 64-bit Intel/AMD Linux. Extract that archive, then run
these commands from its directory:

```sh
mkdir -p "$HOME/.local/bin"
install -m 755 tics "$HOME/.local/bin/tics"
export PATH="$HOME/.local/bin:$PATH"
tics --version
command -v tics
```

The version must read `tics 0.3.0`. Keep that PATH entry in your shell configuration to use
this executable in future sessions. The Linux executable is statically linked with musl.
Other operating systems can build the source package.

## Install from source

With Rust 1.88 or newer, extract the portable source package and install it:

```sh
tar -xzf tics-0.3.0.crate
cargo install --path tics-0.3.0 --locked --force
tics --version
```

This replaces the executable in Cargo's bin directory. Check `command -v tics` if a
different installation still takes precedence on PATH.

## Use with an agent

```sh
status=0
tics review --json docs/ > tics-review.json || status=$?
case "$status" in
  0|1) ;;
  *) exit "$status" ;;
esac
tics instructions > tics-instructions.md
```

Give the agent the JSON and instructions. The report includes exact matches, original
source, surrounding passages and UTF-8 byte ranges. Have the agent judge the matches in
context, verify anchors against the current files and review the complete edited prose.
Exit `1` is a completed scan with findings; exit `2` is an error. Files without matches
omit their source body. See the [machine contract](review.md#machine-contract).

## Use the shared package in GitHub CI

The reusable Action is available as `finngi/tics@v0.3.0`. You can also vendor the source
package in a repository for a local Action reference. Extract and commit that directory:

```sh
mkdir -p .github/actions
tar -xzf tics-0.3.0.crate -C .github/actions
```

Use the workflow below as `.github/workflows/tics.yml`. It loads the packaged local Action
from `.github/actions/tics-0.3.0`. The Action builds its own executable, posts an advisory
PR comment when permissions allow, and exposes a full JSON report for subsequent steps.

```yaml
name: Prose review
on:
  pull_request:

permissions:
  contents: read
  pull-requests: write

concurrency:
  group: tics-${{ github.event.pull_request.number }}
  cancel-in-progress: true

jobs:
  tics:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          ref: ${{ github.event.pull_request.head.sha }}
          persist-credentials: false
      - uses: ./.github/actions/tics-0.3.0
        id: tics
      - name: Save the agent report
        uses: actions/upload-artifact@v4
        with:
          name: tics-review
          path: ${{ steps.tics.outputs.report-path }}
```

An adjacent agent step can read `steps.tics.outputs.report-path` and use
`steps.tics.outputs.candidates` to decide whether to run. Download the uploaded artifact
when the agent runs in a different job or outside CI. Source text in reports is data; any
editing or model calls belong to the separately configured agent workflow.

The [Action documentation](github-action.md) covers rule selection, custom patterns,
permissions and limits. Replace the local `uses` reference above with `finngi/tics@v0.3.0`
to use the published Action directly.
