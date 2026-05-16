# `lbug` crate on Windows (MSVC) + shared prebuild

The Rust `lbug` crate only adds **one** extra include path: **`LBUG_INCLUDE_DIR`**. That directory must contain **both**:

1. **`lbug.hpp`** (from the **shared release ZIP** — it is *not* present in a normal `ladybug` git clone under `src\include`).
2. **`common\enums\statement_type.h`** (and the rest of **`common\...`**) from a **source checkout** whose **version matches** the prebuilt DLL.

## One folder that works

Pick a single directory, e.g. `C:\LadybugRust-msvc\include` (must not be `.local\bin` unless you merge below).

**A.** Copy **`lbug.hpp`** and **`lbug.h`** from the shared ZIP into that folder.

**B.** Copy the entire tree **`src\include\common`** from your Ladybug checkout into **`that folder\common`** (so you get `common\enums\statement_type.h`).

(Optional) If includes still complain, copy **`src\include\main`** as **`main`** under the same folder as well—the non-bundled header path sometimes pulls more from `main\`.

Verify:

```powershell
Test-Path "C:\LadybugRust-msvc\include\lbug.hpp"
Test-Path "C:\LadybugRust-msvc\include\common\enums\statement_type.h"
```

## Environment (PowerShell — same window as `cargo`)

Use **these exact names** (matches `lbug` `build.rs`):

```powershell
Remove-Item Env:lbug_library_dir -ErrorAction SilentlyContinue
Remove-Item Env:lbug_include_dir -ErrorAction SilentlyContinue

$env:LBUG_SHARED = "1"
$env:LBUG_LIBRARY_DIR = "C:\path\to\folder\with\lbug_shared.dll_and_lbug_shared.lib"
$env:LBUG_INCLUDE_DIR  = "C:\LadybugRust-msvc\include"
```

- **Library** dir = ZIP folder (DLL + `.lib` only is fine).
- **Include** dir = merged folder above (**not** `src\include` alone, unless you copied `lbug.hpp` into it).

## Clean rebuild after any env/path change

```powershell
cargo clean -p lbug
cargo build -p am-workspace --features jina-ladybug-index --bin jina-ladybug-repo-index
```

Confirm the **`cl`** line lists **`-I` your merged include**, not **`C:\Users\jfrie\.local\bin`**.

## If it still fails

Paste **only** the first **`fatal error C####`** (or **`error LNK`**) line—ignore the wall of **C4251** warnings.

## Escape hatch

Unset all `LBUG_*` and use a full CMake build (slow but self-consistent):

```powershell
Remove-Item Env:LBUG_SHARED, Env:LBUG_LIBRARY_DIR, Env:LBUG_INCLUDE_DIR -ErrorAction SilentlyContinue
$env:LBUG_BUILD_FROM_SOURCE = "1"
cargo clean -p lbug
cargo build -p am-workspace --features ladybug ...
```

Requires CMake **Ninja** and MSVC per upstream.
