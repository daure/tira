# Tira Constitution

This document defines the rules Tira must preserve as it grows into a Ratatui Jira TUI. Features may change, but these invariants must not be broken without amending this constitution first.

## 1. Keybindings Are Configuration

Every keybinding must be configurable.

No feature may depend on a hard-coded key as its only path. Built-in defaults are allowed, but they must flow through the same keybinding system as user overrides. Input handling should resolve keys into named actions, and application code should depend on those actions rather than terminal key literals. The help action is named `open_help`; its built-in default binding is `?`.

Keybindings live in `~/.tira/keybindings.toml`. Common navigation keys are defined once in a shared `[nav]` section rather than repeated per context; per-context sections may override them.

The keyboard help dialog (bound to `open_help`) reflectively displays these keybindings. To keep keybinding configuration decoupled from dynamic UI widgets, context-based help items are centralized and managed in `src/keymap.rs::help_items`. When adding a configurable keybinding, its help text must be manually added to that function. Help text must render from resolved bindings via `KeySpec::label()`/`join_labels`; literal navigation key strings (such as `"gg / G / Home / End"`) are prohibited.
## 2. Color and Style Are Theme Data

All colors must be reconfigurable.

The TUI may provide built-in themes, but every semantic color used by the interface must be overridable. Code should ask for semantic roles such as focused border, selected row, warning text, muted text, success text, and error text. Code should not scatter raw color values through widgets.

Theme selection and per-color overrides live in `~/.tira/tui.toml`.

## 3. The Status Bar Is Always Available in Normal and Input Modes

The footer/status bar must always be visible while the user is in normal mode or input mode.

The status bar must show:

- the current mode;
- the active panel or section;
- relevant context keybindings for the current focus;
- important transient state such as loading, errors, saved changes, or pending operations.

When there are more relevant shortcuts than fit, the status bar must show the active configured binding for `open_help`. Pressing that binding opens a help dialog. The built-in default is `?`. That dialog lists context-relative keys first and global keys second.

## 4. The App Is Keyboard-First

Tira must be fully usable from the keyboard.

Mouse support may exist, but it must not be required for any workflow. Every command, navigation path, modal action, form submission, and dismiss action must have a keyboard path.

## 5. Modes, Focus, and Modals Are Explicit State

Input behavior must be driven by explicit mode, focus, and modal state.

The app has three input modes: normal, input, and visual. Visual mode is for multi-selecting items. The app should know which mode is active, which panel has focus, and which modal, if any, owns input. Scattered booleans are not enough once they can conflict.

## 6. Jira Writes Are Intentional

Jira is a remote source of truth. Tira must not perform hidden destructive or surprising writes.

Creating, updating, transitioning, assigning, commenting, or deleting Jira data must be visible to the user. Destructive or hard-to-reverse operations require confirmation. Failed writes must become visible UI state, not silent log entries.

## 7. The UI Must Not Block on Jira

Network work must not freeze rendering or input.

Jira requests should run as background work and return typed results to the app. Loading, stale, failed, and cancelled states must be represented explicitly. Stale responses must not overwrite newer user intent.

## 8. Rendering Does Not Own Domain Logic

Widgets render prepared state and emit actions.

UI modules must not call Jira directly, parse service payloads, mutate durable persistence, or perform expensive domain computations. Domain state, view state, effects, and rendering should stay separate enough to test independently.

## 9. Configuration Has a Stable Contract

User configuration must be readable, explainable, and safe to change by hand.

Tira should keep durable user configuration under `~/.tira/`. Missing configuration should produce clear startup errors or documented defaults. Secrets must not be printed in logs, status bars, panic output, or debug views.

## 10. Icons Enhance, Text Carries Meaning

Tira should use Nerd Font icons where they improve scanability, but icons must never be the only carrier of meaning.

The app must tolerate terminals without Nerd Font support. Every icon needs either a sensible ASCII/Unicode fallback or the option to be omitted when no clear fallback exists. Labels, status text, accessible help, and important Jira state must remain understandable without icons.

## 11. Terminal State Must Be Restored

Every path that enters terminal mode must restore the terminal before returning control to the shell.

Normal exit, errors, panics, and shutdown signals should leave raw mode, alternate screen, cursor visibility, mouse mode, and paste mode in a sane state.

## 12. Tests Are Minimal but Load-Bearing

Tests should prove behavior that can break the app, not implementation details.

The default test should exercise integration between modules through public APIs. Prefer testing a full path such as configured key input to action, action to state update, state to rendered status/help content, or Jira response to visible UI state. Unit tests are reserved for code with intrinsic branching risk: parsers, state machines, conflict resolution, stale response handling, and validation.

Tests should be added when behavior crosses a boundary:

- config files into runtime settings;
- key events into named actions;
- actions into app state transitions;
- app state into important rendered content;
- Jira HTTP payloads into domain models;
- Jira failures into visible UI errors;
- terminal lifecycle into restored shell state.

Tests should not assert every terminal cell, every default value, getters, pass-through wrappers, or framework behavior. A small number of behavior-focused integration tests is better than a large suite of brittle unit tests.