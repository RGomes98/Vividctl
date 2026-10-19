# SP-02: NVKMS ABI map across driver versions

Issue: #3. Status: done, decision in `docs/adr/0004-nvkms-abi-generations.md`.

## Question

Which NVKMS structures and constants `vivid-nvkms` depends on change between driver releases, how often, and where? Is there a readback ioctl? The answer defines the "ABI generation" design promised in ADR-0001 and the `tests/abi` check.

## Method

`tests/abi/layout.c` prints 55 sizes, offsets and constants for the ioctls we use (`ALLOC_DEVICE`, `FREE_DEVICE`, `QUERY_DISP`, `QUERY_CONNECTOR_STATIC_DATA`, `QUERY_DPY_DYNAMIC_DATA`, `SET_DPY_ATTRIBUTE`, `GET_DPY_ATTRIBUTE`). `run.sh` compiles it against the headers of every open-gpu-kernel-modules tag from 535 on: **172 tags, 535.43.02 to 615.78.08**, all of which compiled. `analyze.py` groups consecutive tags with identical output.

This is a header analysis. It says what each header declares. Only 595.99.02 has been run against a real driver so far (SP-01).

## Result

172 tags fall into **10 distinct layouts**: the layout the backend uses changed 9 times in 171 releases, about once every 19 releases.

| # | First tag | Last tag | Tags | What changed from the previous generation |
|---|---|---|---|---|
| 1 | 535.43.02 | 535.309.01 | 39 | (first) |
| 2 | 545.23.06 | 555.58.02 | 44 | `QueryDpyDynamicDataParams` 37096 to 37128 bytes |
| 3 | 560.28.03 | 560.35.03 | 3 | `QueryDpyDynamicDataParams` 37128 to 37156 |
| 4 | 565.57.01 | 565.77 | 2 | `AllocDeviceReply` 464 to 872, `AllocDeviceParams` 1080 to 1488 |
| 5 | 570.86.15 | 570.195.03 | 23 | EDID offsets in the dynamic-data reply (+4), dynamic params 37160 |
| 6 | 570.207 | 570.211.01 | 2 | **`SET_DPY_ATTRIBUTE` 21 to 22 and `GET_DPY_ATTRIBUTE` 22 to 23** |
| 7 | 575.51.02 | 590.48.01 | 34 | `no3d` offset 38 to 42, `AllocDeviceRequest` 616 to 620, reply offset 616 to 624, `AllocDeviceReply` 872 to 888, `AllocDeviceParams` 1512 |
| 8 | 595.44.02 | 595.104.02 | 20 | `no3d` offset 42 to 40, EDID offsets +8, dynamic params 37168 |
| 9 | 610.43.02 | 610.57.04 | 3 | `AllocDeviceReply` 888 to 816, `AllocDeviceParams` 1440 |
| 10 | 615.71.09 | 615.78.08 | 2 | **`SET`/`GET_DPY_ATTRIBUTE` back to 21/22**, EDID buffer offset 218, `AllocDeviceReply` 824 |

The full values are in `tests/abi/generations.json`.

### What stays the same in all 172 tags

The envelope (`NVKMS_IOCTL_IOWR`), the version string length, the command numbers of `ALLOC_DEVICE`, `FREE_DEVICE`, `QUERY_DISP`, `QUERY_CONNECTOR_STATIC_DATA` and `QUERY_DPY_DYNAMIC_DATA`, the digital vibrance attribute value (10), the sizes of the query/connector/set/get parameter blocks, and every offset we read except the ones below.

### What varies

| Item | Distinct values | Where it changes |
|---|---|---|
| `AllocDeviceParams` size | 1080, 1488, 1512, 1440, 1448 | 565, 575, 610, 615 |
| `AllocDeviceRequest` size, `no3d` offset, reply offset | 616/38/616 then 620/42/624 then 620/40/624 | 575, 595 |
| `QueryDpyDynamicDataParams` size | 37096, 37128, 37156, 37160, 37168 | 545, 560, 570, 595 |
| EDID offsets in the dynamic-data reply | 202..205, 206..209, 214..217, 218 | 570, 595, 615 |
| `SET_DPY_ATTRIBUTE` / `GET_DPY_ATTRIBUTE` command numbers | 21/22, 22/23 | **570.207** (inside the 570 series), 615 |

### Observations

- **A release series is not stable.** Generation 5 to 6 is inside 570: 570.195.03 and 570.207 have different command numbers. Version ranges by major series are wrong; boundaries fall on exact tags.
- **Command numbers are not monotonic.** They go 21/22, then 22/23, then back to 21/22. An enum entry was inserted and later removed.
- **Both ioctls we use to write and read the value (`SET`, `GET`) are adjacent** in every generation (`GET = SET + 1`).
- **Sizes change more than offsets,** and every size change is a rejection by the kernel (exact `paramSize` match, SP-01). Those are safe failures. A shifted command number is the dangerous one: the same number can mean a different ioctl.
- **`GET_DPY_ATTRIBUTE` exists in all 172 tags,** so readback is always available (answers the SP-02 question from SP-01).

### The two machines in play

| Machine | Driver | Generation |
|---|---|---|
| Desktop (GTX 1660 Ti) | 595.99.02 | 8 (595.44.02 to 595.104.02), the layout SP-01 used |
| Notebook (Dell Inspiron 16 Plus 7640, RTX 4060) | 580.178.04 | 7 (575.51.02 to 590.48.01) |

Generations 7 and 8 differ in the `no3d` offset (42 vs 40), the EDID offsets (209/206/208 vs 217/214/216) and the dynamic-data size (37160 vs 37168). The SP-01 PoC, which has generation 8 hard-coded, would set the wrong byte for `no3d` on the notebook.

## Not covered

- Anything newer than 615.78.08, and tags before 535.
- Runtime behaviour on any driver other than 595.99.02. The notebook run (generation 7) is the first.
- Whether NVIDIA's closed (non-open) driver follows the same layout. The headers are those of the open kernel modules; the interface is shared with the proprietary driver, but this was not verified.
- Fields we do not use. A change there does not affect us, and is invisible here.
