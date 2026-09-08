# lucuma-geo-wasm

A minimal polygon geometry kernel, Rust [`geo`](https://docs.rs/geo) compiled to WebAssembly,
built for lucuma-core's Scala.js `ShapeExpression` interpreter. It is not a port of JTS: it
exposes only what `ShapeExpression` needs and mirrors JTS point placement for ellipses and arcs so
results match lucuma-jts. Measured 31x to 37x faster than lucuma-jts on Scala.js for the AGS
geometry hot path (see lucuma-core `docs/ags-wasm-findings.md`).

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
| `area(h)`, `bbox(h)`, `coords(h)` | measurements; `bbox` is NaN-filled when empty |
| `contains_point(h,x,y)`, `intersects(a,b)` | predicates |
| `free(h)`, `live()` | memory management and leak detection |

Handles are recycled after `free`; using a freed handle panics (surfaces as a JS exception).

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
Repository variable `NPM_SCOPE` sets the npm scope (leave unset for an unscoped package);
secret `NPM_TOKEN` authenticates.
