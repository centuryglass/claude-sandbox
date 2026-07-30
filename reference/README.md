# Original source

`MultiReflectionShader.cpp`/`.h`, written May 2013 for a college graphics
course. This is the actual shader behind the original kaleidoscope-fold
artifacts that `hit-chain-drift` (see `../src/render.rs`, `../src/glitch.rs`)
reverse-engineers and reproduces on purpose. Kept verbatim, bugs and all -
see the top-level README's "The real bug" section for the writeup of what's
actually going on in `getHitColor`.
