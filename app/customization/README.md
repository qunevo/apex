# Customization packages

Each folder groups one synthetic or privately installed domain package. The
bundled packages are `demo` (the factory integration boundary) and
`dummy_customer` (the existing native scheduling extension example).

`package.json` contains a stable `id` matching the folder and a version token.
The server configuration explicitly enables folder IDs and selects a default;
placing another folder here does not activate it. See
[server configuration](../docs/server-configuration.md).

- `adapter/`: source extraction and repeatable mapping.
- `model/`: domain knowledge, executable declarations and optional native hooks.
- `skills/`: domain workflows extending the general application skills.
- `tests/`: synthetic mapping and semantic tests.
- `ui/`: optional views reusing the common UI.

These are responsibility boundaries, not automatic plugin discovery. Package
selection scopes server state and inputs. `Problem.customization` independently
selects a statically registered native extension; a folder does not register
Rust code. Skills must be loaded by the agent host. UI extensions need explicit
build/server registration. Real customer packages and data are not published.
