# lucuma-wasm

A minimal polygon geometry kernel, Rust [`geo`](https://docs.rs/geo) compiled to WebAssembly,
built for lucuma-core's Scala.js `ShapeExpression` interpreter. It is not a port of JTS: it
exposes only what `ShapeExpression` needs and mirrors JTS point placement for ellipses and arcs so
results match lucuma-jts. Measured 30x to 100x faster than lucuma-jts on Scala.js for the AGS
geometry hot path.

## API

Geometries live in a wasm-side arena addressed by `u32` handles. Coordinates are `f64` in the
caller's units. All functions are exported by wasm-bindgen (`--target web`).

| function | purpose |
|---|---|
| `version()` | package semver, for compatibility checks |
| `empty_new()`, `poly_new(flatRing)`, `rect_new(x0,y0,x1,y1)` | constructors |
| `ellipse_new(x0,y0,x1,y1,npts)`, `arc_new(x0,y0,x1,y1,start,extent,npts)` | JTS `GeometricShapeFactory` semantics |
| `op(kind,a,b)` | 0 intersection, 1 union, 2 difference |
| `affine(h,m00,m01,m02,m10,m11,m12)` | general 2D affine transform |
| `area(h)`, `bbox(h)`, `coords(h)`, `rings(h)` | measurements; `bbox` is NaN-filled when empty, `rings` includes holes |
| `contains_point(h,x,y)`, `intersects(a,b)` | predicates |
| `free(h)`, `live()` | memory management and leak detection |

## Memory handling

Geometries never cross the wasm boundary. Every function that returns a handle allocates a new
arena slot, including `op` and `affine`, and the caller owns it: it must be released with
`free(h)`. The JS garbage collector cannot see the arena, so an unfreed handle is a permanent
leak.

- `free` is idempotent; freed slots are recycled by later constructors. Using a freed handle
  panics (a JS exception), but a stale handle whose slot was reused silently addresses the newer
  geometry, so never keep handles past `free`.
- `live()` returns the number of live handles; use it in tests to assert no leaks.
- Memory cost is set by how many geometries are alive at once, not by how many are created. A
  100-point ellipse is roughly 2 KB. Wasm linear memory grows but never shrinks, so the peak
  live set stays allocated until the module is reloaded.
- Recommended pattern: keep handles inside the interpreter, free intermediate results as soon as
  they are consumed, and return only scalars or coordinate arrays to the caller.

## Build

With nix (mirrors lucuma-core's flake-based dev shell; `direnv allow` picks up `.envrc`):

    nix develop
    check        # cargo fmt --check, clippy -D warnings, cargo test (what CI runs)
    build-wasm   # wasm-pack build --release --target web -> pkg/

Without nix:

    rustup target add wasm32-unknown-unknown
    cargo install wasm-pack
    cargo test
    wasm-pack build --release --target web

Loading in Node (no `fetch(file:)`): `init({ module_or_path: fs.readFileSync(pathToWasm) })`.
In the browser or Vite, `init()` with no arguments resolves the `.wasm` next to the JS glue.

## Release

Tag `vX.Y.Z` matching `Cargo.toml`; the Release workflow builds and runs `npm publish`.
It publishes as `@gemini-hlsw/lucuma-wasm` via npm trusted publishing (OIDC, no token);
the package's trusted publisher on npmjs.com must point at this repo and `release.yml`.
