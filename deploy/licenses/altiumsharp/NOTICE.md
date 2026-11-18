# AltiumSharp (OriginalCircuit.Altium)

The hub's Altium file writer (`native/host/src/altium/write/pcblib.rs`) follows AltiumSharp's PcbLib writer for the binary
layouts of footprint primitives (pads, tracks, arcs, component bodies), and carries its 141-byte pad record tail template
(Apache-2.0, `LICENSE` here). Source: https://github.com/issus/AltiumSharp, commit ce72437f30cd54f549601d4e0ca5846d21272150
(`src/OriginalCircuit.Altium/Serialization/Writers/PcbLibWriter.cs`). Changes: rewritten in Rust; per-pad GUIDs and the v7
layer id are written over the template, the rest of the record is built from Backplane's footprint model.
