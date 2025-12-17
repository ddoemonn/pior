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

Tested on [TanStack/query](https://github.com/TanStack/query) (908 files):

| Tool | Time |
|:---|---:|
| pior | 117 ms |
| knip | 3.5 s |

**~30x faster**

Single package (@tanstack/query-core, 52 files):

| Tool | Time |
|:---|---:|
| pior | 9.4 ms |
| knip | 473 ms |

**~50x faster**

Run your own:

```bash
hyperfine --warmup 2 -i 'pior' 'bunx knip'
```
