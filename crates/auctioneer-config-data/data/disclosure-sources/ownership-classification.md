FPD delegation explicitly owns `user.data`, `keywords`, `kwarray` and
`user.ext.data`; it is not a generic opaque-context exemption. These branches
survive ID-only denial if their owning permission allows them. `customdata`
is ID-gated. TID ownership is exactly native `source.tid`, its direct shadow,
`imp.ext.tid`, `ImpExt.other["tid"]`, and `Imp.other["ext"]["tid"]`.
Unrelated Source extensions and ordinary request/impression IDs are not TIDs.
