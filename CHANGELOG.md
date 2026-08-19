## [unreleased]

### 🚀 Features

- *(startup)* Add daemon retry screen
- *(doctor)* Add catalog stream health

### ⚙️ Miscellaneous Tasks

- Pin contract test daemon
## [v0.1.0-beta.1] - 2026-08-18

### 🚀 Features

- Add compozy client contract types
- Add compozy client transport endpoints
- *(client)* Add reconnecting transcript stream
- Add batuta doctor and sessions
- Add live transcript tail
- *(compozy-client)* Add session write endpoints
- *(compozy-client)* Add loop and observability surfaces
- *(tui)* Add panel core runtime and layouts
- *(tui)* Implement sessions runs and attention panels
- *(tui)* Add session conversation detail
- *(tui)* Add run detail and workspace overlays
- Wire batuta application entrypoint

### 🐛 Bug Fixes

- *(tui)* Read offline threshold from ReconnectPolicy instead of hardcoding 5
- *(compozy-client)* Timeout SSE connect before response headers arrive
- *(tui)* Clamp scrolled selection after a shrinking transcript reset

### 📚 Documentation

- Batuta-cli design spec and project rules
- Do not assume a loop filter on the loop-runs route
- Record batuta CLI spike findings
- Close batuta MVP documentation

### 🧪 Testing

- Add disposable daemon contract harness
- *(tail)* Cover UT-103/UT-105 with daemon-independent unit tests

### ⚙️ Miscellaneous Tasks

- Establish workspace foundation
