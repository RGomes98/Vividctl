# tests/abi

Checks the NVKMS ABI that `vivid-nvkms` depends on against the headers of NVIDIA's
[open-gpu-kernel-modules](https://github.com/NVIDIA/open-gpu-kernel-modules) tags. See
`docs/adr/0004-nvkms-abi-generations.md` for why this exists.

| File | Role |
|---|---|
| `layout.c` | Prints one `key=value` line per size, offset and constant the backend uses, for the tag whose headers it is compiled against |
| `run.sh` | Checks out tags of a clone, compiles `layout.c` against each and writes `out/<tag>.txt` (or `out/<tag>.error`) |
| `analyze.py` | Groups consecutive tags with identical output into ABI generations; `--json` emits the data |
| `generations.json` | The generations found for tags 535.43.02 to 615.78.08, with every value. Source data for the Rust layout table |

```
git clone --filter=blob:none --no-checkout --sparse https://github.com/NVIDIA/open-gpu-kernel-modules.git /tmp/ogkm
tests/abi/run.sh /tmp/ogkm 580.178.04 595.99.02          # a few tags
python3 -I tests/abi/analyze.py tests/abi/out             # table
python3 -I tests/abi/analyze.py tests/abi/out --json > tests/abi/generations.json
```

`out/` is git-ignored. The NVIDIA headers are MIT licensed; nothing from them is copied here, the probe only prints numbers (see `NOTICE`).
