# Spikes

A spike is a time-boxed investigation. Its deliverable is an ADR in `docs/adr/`, plus raw notes
and measurements in its own folder here: `docs/spikes/sp-NN-<slug>/README.md`, next to any
throwaway code (`Cargo.toml`, `src/`, C probes). Spike code is standalone and is not part of the
product workspace.

Every spike states its go/no-go criteria up front, in the issue. See `ROADMAP.md` for the list.
