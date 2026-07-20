# Slime2 Standard Library

## Usage

```slime
!link slime.std          // loads std/prelude.sm (core modules)
!link slime.std.math     // or link a single module

fn main() {
    print std.math.abs_i(-1);
}
```

Resolve order for `slime.std.*`:
1. `$SLIME_STD/`
2. `./std/` next to the source / project root
3. `std/` beside the `slime` executable

## Modules

| Module | Namespace | Contents |
|--------|-----------|----------|
| `math` | `std.math` | abs/min/max/clamp, sin/cos/sqrt/pow/floor/ceil/round, `Vec2` |
| `conv` | `std.conv` | int↔str, str→float, int→float |
| `string` | `std.string` | len, cmp, slice, eq, concat, from_int |
| `io` | `std.io` | writeln, write_int/float/bool |
| `time` | `std.time` | now() → mono clock |
| `hash` | `std.hash` | md5 |
| `dict` | `std.dict` | notes for `dict_*` builtins |
| `own` | `std.own` | ownership helpers / docs |
| `system` | `std.system` | small system helpers |

## Builtins (also usable without std)

`print`, `abs`, `min`, `max`, `clamp`, `sin`, `cos`, `sqrt`, `pow`, `floor`, `ceil`, `round`, `itoa`, `atoi`, `atof`, `float`, `strlen`/`len`, `strcmp`, `substr`, `md5`, `mono_now`, `dict_*`, ownership: `share_new`, `share_peer`, `joint_new`, `joint_peer`, `consent`, `drop`.

## Runtime (LLVM)

Native builds auto-link `rt/slime_rt.c` when present.
