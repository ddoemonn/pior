# Pior

Dead code detector for JavaScript/TypeScript. Written in Rust.

## Install

```bash
cargo install pior
```

## Usage

```bash
pior                      # analyze current directory
pior ./path/to/project    # analyze specific path
pior --fix                # auto-remove unused deps/exports
pior --format json        # json output
pior --watch              # re-run on file changes
pior --cache              # cache parsed files
```

## What it finds

- Unused files
- Unused exports
- Unused dependencies
- Unlisted dependencies
- Unresolved imports

## Config

Create `pior.json`:

```json
{
  "entry": ["src/index.ts"],
  "project": ["src/**/*.ts"],
  "ignore": ["**/*.test.ts"],
  "ignoreDependencies": ["@types/node"]
}
```

## Monorepo

```bash
pior --workspaces         # list workspaces
pior --workspace pkg-name # analyze single workspace
```

## Output formats

`pretty` | `json` | `compact` | `github` | `codeclimate`

## Benchmark

Tested on [TanStack/query](https://github.com/TanStack/query) (835 files):

| Tool | Mean | Min | Max |
|:---|---:|---:|---:|
| pior | 65.1 ms | 64.6 ms | 65.5 ms |
| knip | 1568 ms | 1528 ms | 1689 ms |

**~24x faster**

Run your own:

```bash
hyperfine --warmup 2 'pior .' 'bunx knip'
```
