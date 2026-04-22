# Settings & Configuration Management

<cite>
**Referenced Files in This Document**
- [SettingsApp.tsx](file://src/modules/settings/SettingsApp.tsx)
- [SettingsShell.tsx](file://src/modules/settings/components/SettingsShell.tsx)
- [SettingsSidebar.tsx](file://src/modules/settings/components/SettingsSidebar.tsx)
- [SettingsSidebarItem.tsx](file://src/modules/settings/components/SettingsSidebarItem.tsx)
- [data.ts](file://src/modules/settings/data.ts)
- [types.ts](file://src/modules/settings/types.ts)
- [GeneralSettingsPage.tsx](file://src/modules/settings/pages/GeneralSettingsPage.tsx)
- [ModelSettingsPage.tsx](file://src/modules/settings/pages/ModelSettingsPage.tsx)
- [MemorySettingsPage.tsx](file://src/modules/settings/pages/MemorySettingsPage.tsx)
- [SkillsSettingsPage.tsx](file://src/modules/settings/pages/SkillsSettingsPage.tsx)
- [ConnectionsSettingsPage.tsx](file://src/modules/settings/pages/ConnectionsSettingsPage.tsx)
- [TtsSettingsPage.tsx](file://src/modules/settings/pages/TtsSettingsPage.tsx)
- [SttConfigPage.tsx](file://src/modules/settings/pages/SttConfigPage.tsx)
</cite>

## Table of Contents
1. [Introduction](#introduction)
2. [Project Structure](#project-structure)
3. [Core Components](#core-components)
4. [Architecture Overview](#architecture-overview)
5. [Detailed Component Analysis](#detailed-component-analysis)
6. [Dependency Analysis](#dependency-analysis)
7. [Performance Considerations](#performance-considerations)
8. [Troubleshooting Guide](#troubleshooting-guide)
9. [Conclusion](#conclusion)
10. [Appendices](#appendices)

## Introduction
This document describes the Settings & Configuration Management system in the application. It covers the SettingsShell architecture, sidebar navigation, and page routing. It documents the settings pages for General, Model, Memory, Skills, Connections, TTS, and STT configurations, along with the settings data flow, preference persistence, validation mechanisms, synchronization, and extension points for adding new settings pages and custom configuration options.

## Project Structure
The settings system is organized around a central SettingsApp shell that hosts a sidebar-driven navigation and a content area rendering the active settings page. Pages are grouped under a dedicated pages directory and share common UI components and types.

```mermaid
graph TB
subgraph "Settings Shell"
A["SettingsApp.tsx"]
B["SettingsShell.tsx"]
C["SettingsSidebar.tsx"]
D["SettingsSidebarItem.tsx"]
end
subgraph "Pages"
P1["GeneralSettingsPage.tsx"]
P2["ModelSettingsPage.tsx"]
P3["MemorySettingsPage.tsx"]
P4["SkillsSettingsPage.tsx"]
P5["ConnectionsSettingsPage.tsx"]
P6["TtsSettingsPage.tsx"]
P7["SttConfigPage.tsx"]
end
subgraph "Shared"
S1["data.ts"]
S2["types.ts"]
end
A --> B
B --> C
C --> D
A --> P1
A --> P2
A --> P3
A --> P4
A --> P5
A --> P6
A --> P7
A --> S1
A --> S2
```

**Diagram sources**
- [SettingsApp.tsx:36-227](file://src/modules/settings/SettingsApp.tsx#L36-L227)
- [SettingsShell.tsx:27-69](file://src/modules/settings/components/SettingsShell.tsx#L27-L69)
- [SettingsSidebar.tsx:31-92](file://src/modules/settings/components/SettingsSidebar.tsx#L31-L92)
- [SettingsSidebarItem.tsx:11-39](file://src/modules/settings/components/SettingsSidebarItem.tsx#L11-L39)
- [data.ts:17-108](file://src/modules/settings/data.ts#L17-L108)
- [types.ts:3-63](file://src/modules/settings/types.ts#L3-L63)

**Section sources**
- [SettingsApp.tsx:36-227](file://src/modules/settings/SettingsApp.tsx#L36-L227)
- [SettingsShell.tsx:27-69](file://src/modules/settings/components/SettingsShell.tsx#L27-L69)
- [SettingsSidebar.tsx:31-92](file://src/modules/settings/components/SettingsSidebar.tsx#L31-L92)
- [SettingsSidebarItem.tsx:11-39](file://src/modules/settings/components/SettingsSidebarItem.tsx#L11-L39)
- [data.ts:17-108](file://src/modules/settings/data.ts#L17-L108)
- [types.ts:3-63](file://src/modules/settings/types.ts#L3-L63)

## Core Components
- SettingsApp: Central orchestrator managing active section, shared state/actions, and page rendering. It wires up the SettingsShell and renders the appropriate page based on the active section.
- SettingsShell: Provides the layout, header with dynamic icon/label, and integrates the sidebar and content area.
- SettingsSidebar: Renders the navigation list from SETTINGS_SECTIONS and handles window drag and close actions.
- SettingsSidebarItem: Individual clickable navigation item with active state styling.
- Shared Types and Data: Defines SettingsSectionId, SettingsState, SettingsActions, and the SETTINGS_SECTIONS metadata used by the sidebar.

Key responsibilities:
- Routing: SettingsApp switch-case determines the active page.
- State: Shared state (theme, fontMode, language, etc.) is passed down to pages via props.
- Persistence: Pages trigger backend invocations (e.g., TTS/STT settings, memory config) via tauri commands.
- Validation: Pages validate inputs and show user feedback via toast messages.

**Section sources**
- [SettingsApp.tsx:36-227](file://src/modules/settings/SettingsApp.tsx#L36-L227)
- [SettingsShell.tsx:27-69](file://src/modules/settings/components/SettingsShell.tsx#L27-L69)
- [SettingsSidebar.tsx:31-92](file://src/modules/settings/components/SettingsSidebar.tsx#L31-L92)
- [SettingsSidebarItem.tsx:11-39](file://src/modules/settings/components/SettingsSidebarItem.tsx#L11-L39)
- [data.ts:17-108](file://src/modules/settings/data.ts#L17-L108)
- [types.ts:3-63](file://src/modules/settings/types.ts#L3-L63)

## Architecture Overview
The settings architecture follows a shell-and-pages pattern:
- SettingsApp holds state and routes to the active page.
- SettingsShell composes the layout and header.
- SettingsSidebar reads metadata from SETTINGS_SECTIONS to render navigation.
- Pages implement domain-specific configuration UIs and persist changes via tauri commands.

```mermaid
sequenceDiagram
participant U as "User"
participant SA as "SettingsApp"
participant SS as "SettingsShell"
participant SB as "SettingsSidebar"
participant PG as "Selected Settings Page"
U->>SB : Click navigation item
SB->>SA : onSectionChange(sectionId)
SA->>SA : Update activeSection
SA->>SS : Render with activeSection
SS->>PG : Render content for activeSection
U->>PG : Interact with controls
PG->>PG : Validate input
PG->>PG : Persist via tauri commands
PG-->>U : Show toast feedback
```

**Diagram sources**
- [SettingsApp.tsx:170-216](file://src/modules/settings/SettingsApp.tsx#L170-L216)
- [SettingsShell.tsx:33-60](file://src/modules/settings/components/SettingsShell.tsx#L33-L60)
- [SettingsSidebar.tsx:80-88](file://src/modules/settings/components/SettingsSidebar.tsx#L80-L88)
- [data.ts:17-108](file://src/modules/settings/data.ts#L17-L108)

## Detailed Component Analysis

### SettingsShell and Sidebar Navigation
- SettingsShell dynamically sets the page header icon and description based on the active section and renders the sidebar and content area.
- SettingsSidebar renders the full navigation list from SETTINGS_SECTIONS and supports window dragging and closing.
- SettingsSidebarItem renders individual items with active highlighting and click handlers.

```mermaid
classDiagram
class SettingsShell {
+activeSection : SettingsSectionId
+onSectionChange(section)
+onClose()
+render()
}
class SettingsSidebar {
+activeSection : SettingsSectionId
+onSectionChange(section)
+onClose()
+render()
}
class SettingsSidebarItem {
+icon : ComponentType
+label : string
+active : boolean
+onClick()
}
SettingsShell --> SettingsSidebar : "contains"
SettingsSidebar --> SettingsSidebarItem : "renders list"
```

**Diagram sources**
- [SettingsShell.tsx:27-69](file://src/modules/settings/components/SettingsShell.tsx#L27-L69)
- [SettingsSidebar.tsx:31-92](file://src/modules/settings/components/SettingsSidebar.tsx#L31-L92)
- [SettingsSidebarItem.tsx:11-39](file://src/modules/settings/components/SettingsSidebarItem.tsx#L11-L39)

**Section sources**
- [SettingsShell.tsx:27-69](file://src/modules/settings/components/SettingsShell.tsx#L27-L69)
- [SettingsSidebar.tsx:31-92](file://src/modules/settings/components/SettingsSidebar.tsx#L31-L92)
- [SettingsSidebarItem.tsx:11-39](file://src/modules/settings/components/SettingsSidebarItem.tsx#L11-L39)

### Settings Routing and Page Rendering
- SettingsApp maintains activeSection and constructs the content via a switch statement over SETTINGS_SECTION ids.
- It passes a shared SettingsState and SettingsActions to each page.

```mermaid
flowchart TD
Start(["SettingsApp mount"]) --> Init["Initialize state and actions"]
Init --> Route["Switch on activeSection"]
Route --> Render["Render SettingsShell with activeSection"]
Render --> Page["Render selected page component"]
Page --> End(["User interacts with page"])
```

**Diagram sources**
- [SettingsApp.tsx:36-227](file://src/modules/settings/SettingsApp.tsx#L36-L227)

**Section sources**
- [SettingsApp.tsx:36-227](file://src/modules/settings/SettingsApp.tsx#L36-L227)

### General Settings Page
- Manages user profile, account info, preferences (language, startup mode, theme, density, font), and behavior toggles (auto-scroll, notifications).
- Uses shared state/actions to update values and persists locally (e.g., fontMode in localStorage).

Validation and persistence highlights:
- Preference updates are immediate and reflected in the UI.
- Font mode is persisted to localStorage and applies a body class for serif mode.

**Section sources**
- [GeneralSettingsPage.tsx:26-226](file://src/modules/settings/pages/GeneralSettingsPage.tsx#L26-L226)
- [SettingsApp.tsx:121-131](file://src/modules/settings/SettingsApp.tsx#L121-L131)

### Model Settings Page
- Configures LLM role assignments (chat, utility, utility_large, summarizer, compiler) with a dropdown picker grouped by provider.
- Manages the local fastembed vectorization model name and HuggingFace mirror source.
- Persists via tauri commands and validates inputs (non-empty model name).

Key behaviors:
- Loads available models, role configs, and embedded model config on mount.
- Supports saving role configs individually and bulk operations.
- Validates total token budget distribution and shows warnings.

**Section sources**
- [ModelSettingsPage.tsx:222-579](file://src/modules/settings/pages/ModelSettingsPage.tsx#L222-L579)

### Memory Settings Page
- Controls token budget allocation across memory slots (system, episodic, semantic, working).
- Enables Memory Control Plane V1 feature flags (recall_mode, policy_enforce_mode).
- Manages promotion thresholds for memory upgrades across session/project/global scopes.
- Provides trajectory export and a two-step destructive “Clear all memories” operation.
- Integrates with compiled memory viewer and narrative timeline viewers.

Validation and safety:
- Enforces 100% total percentage across slots.
- Two-step confirmation for destructive actions with auto-disarming.

**Section sources**
- [MemorySettingsPage.tsx:45-629](file://src/modules/settings/pages/MemorySettingsPage.tsx#L45-L629)

### Skills Settings Page
- Provides three tabs: Installed, Market, and Proposals.
- Supports filtering by category, source, and search; toggling skills; and performing lifecycle actions (review, approve, rollback).
- Integrates with skills hub (GitHub and skills.sh) for discovery and installation.
- Implements caching and pagination for market listings.

Operational flow:
- Loads skills list and refreshes on tab change.
- Handles installation from market and manual distribution.
- Uses toasts for user feedback and error handling.

**Section sources**
- [SkillsSettingsPage.tsx:110-800](file://src/modules/settings/pages/SkillsSettingsPage.tsx#L110-L800)

### Connections Settings Page
- Presents a catalog of communication channels (e.g., Feishu, WeChat, QQ, DingTalk, Telegram, Discord, Slack, Teams, LINE, Signal, iMessage, WhatsApp, Mattermost, Matrix).
- Shows connection status and provides “Connect” or “Manage” actions per channel.
- Organizes channels by priority (high/low).

**Section sources**
- [ConnectionsSettingsPage.tsx:189-229](file://src/modules/settings/pages/ConnectionsSettingsPage.tsx#L189-L229)

### TTS Settings Page
- Manages text-to-speech runtime parameters persisted to a TOML file.
- Sections: Speed (playback_rate), Quality (three presets mapping to server-side sampling parameters), and Advanced (power-user parameters).
- Persists on every change and broadcasts updates to other windows for immediate effect.

Validation and UX:
- Range sliders and numeric inputs clamp values to safe bounds.
- Reset to defaults confirmation dialog.
- Real-time feedback via toasts.

**Section sources**
- [TtsSettingsPage.tsx:73-384](file://src/modules/settings/pages/TtsSettingsPage.tsx#L73-L384)

### STT Config Page
- Manages the SenseVoice (OpenFlow) speech-to-text model: status checks, download progress events, and one-click download.
- Broadcasts readiness to other windows upon successful download.

UX:
- Progress bar and percentage display during download.
- Clear status indicators and instructions for first-time microphone usage.

**Section sources**
- [SttConfigPage.tsx:29-202](file://src/modules/settings/pages/SttConfigPage.tsx#L29-L202)

## Dependency Analysis
- SettingsApp depends on:
  - SETTINGS_SECTIONS metadata for navigation.
  - Shared SettingsState/Actions for page props.
  - Tauri commands for persistence in specialized pages (e.g., TTS, STT, memory).
- Pages depend on:
  - Shared UI components (SettingsSurface, SettingsRow, CompactInput, etc.).
  - Tauri APIs for backend operations.
  - Cross-window sync utilities for broadcasting changes.

```mermaid
graph LR
SA["SettingsApp.tsx"] --> DS["data.ts"]
SA --> TY["types.ts"]
SA --> SH["SettingsShell.tsx"]
SH --> SB["SettingsSidebar.tsx"]
SB --> SI["SettingsSidebarItem.tsx"]
subgraph "Pages"
GP["GeneralSettingsPage.tsx"]
MP["ModelSettingsPage.tsx"]
MemP["MemorySettingsPage.tsx"]
SKP["SkillsSettingsPage.tsx"]
ConnP["ConnectionsSettingsPage.tsx"]
TTSP["TtsSettingsPage.tsx"]
STTP["SttConfigPage.tsx"]
end
SA --> GP
SA --> MP
SA --> MemP
SA --> SKP
SA --> ConnP
SA --> TTSP
SA --> STTP
```

**Diagram sources**
- [SettingsApp.tsx:1-32](file://src/modules/settings/SettingsApp.tsx#L1-L32)
- [data.ts:1-15](file://src/modules/settings/data.ts#L1-L15)
- [types.ts:1-63](file://src/modules/settings/types.ts#L1-L63)
- [SettingsShell.tsx:1-6](file://src/modules/settings/components/SettingsShell.tsx#L1-L6)
- [SettingsSidebar.tsx:1-5](file://src/modules/settings/components/SettingsSidebar.tsx#L1-L5)
- [SettingsSidebarItem.tsx:1-9](file://src/modules/settings/components/SettingsSidebarItem.tsx#L1-L9)

**Section sources**
- [SettingsApp.tsx:1-32](file://src/modules/settings/SettingsApp.tsx#L1-L32)
- [data.ts:1-15](file://src/modules/settings/data.ts#L1-L15)
- [types.ts:1-63](file://src/modules/settings/types.ts#L1-L63)

## Performance Considerations
- Minimize re-renders by memoizing derived values (e.g., filtered lists in SkillsSettingsPage).
- Debounce or batch frequent updates (e.g., numeric inputs in TTS/STT pages) to reduce unnecessary tauri calls.
- Use lazy loading for heavy pages (e.g., Skills Market) and cache results where appropriate.
- Keep UI updates synchronous for immediate feedback while deferring expensive operations.

## Troubleshooting Guide
Common issues and remedies:
- Settings not persisting:
  - Verify tauri command invocations succeed and toast errors are visible.
  - For TTS/STT, ensure cross-window sync is functioning and broadcasts are received.
- Validation failures:
  - Memory settings require 100% total slot allocation; adjust sliders until valid.
  - Model settings require a non-empty model name; ensure input is trimmed and validated.
- Destructive operations:
  - Two-step confirmation for clearing all memories; ensure the second click occurs within the disarm window.
- Network-dependent pages:
  - Skills Market relies on external APIs; use cached data or retry after network stabilization.

**Section sources**
- [MemorySettingsPage.tsx:116-143](file://src/modules/settings/pages/MemorySettingsPage.tsx#L116-L143)
- [ModelSettingsPage.tsx:311-330](file://src/modules/settings/pages/ModelSettingsPage.tsx#L311-L330)
- [SkillsSettingsPage.tsx:149-171](file://src/modules/settings/pages/SkillsSettingsPage.tsx#L149-L171)

## Conclusion
The settings system is a cohesive shell-and-pages architecture with strong separation of concerns. It provides a scalable foundation for adding new configuration pages, integrating with backend persistence, and ensuring a responsive user experience through validation and immediate feedback.

## Appendices

### Adding a New Settings Page
Steps:
1. Define a new SettingsSectionMeta in SETTINGS_SECTIONS with an id, label, description, and icon.
2. Create a new page component under pages/.
3. Extend SettingsState/Actions in types.ts if needed.
4. Wire the new page in SettingsApp’s switch statement and pass state/actions.
5. Optionally integrate tauri commands for persistence and cross-window sync.

Example references:
- [data.ts:17-108](file://src/modules/settings/data.ts#L17-L108)
- [types.ts:3-63](file://src/modules/settings/types.ts#L3-L63)
- [SettingsApp.tsx:170-216](file://src/modules/settings/SettingsApp.tsx#L170-L216)

**Section sources**
- [data.ts:17-108](file://src/modules/settings/data.ts#L17-L108)
- [types.ts:3-63](file://src/modules/settings/types.ts#L3-L63)
- [SettingsApp.tsx:170-216](file://src/modules/settings/SettingsApp.tsx#L170-L216)

### Custom Configuration Options
- Use SettingsRow/SettingsSurface components for consistent layouts.
- For complex forms, implement controlled inputs with validation and clamping.
- Persist via tauri commands and broadcast changes to keep other windows in sync.

**Section sources**
- [TtsSettingsPage.tsx:95-111](file://src/modules/settings/pages/TtsSettingsPage.tsx#L95-L111)
- [SttConfigPage.tsx:47-61](file://src/modules/settings/pages/SttConfigPage.tsx#L47-L61)

### User Preference Management
- General preferences (theme, language, density, font) are stored in component state and persisted locally where applicable.
- Apply preferences immediately (e.g., fontMode affects body class) for instant feedback.

**Section sources**
- [GeneralSettingsPage.tsx:121-131](file://src/modules/settings/pages/GeneralSettingsPage.tsx#L121-L131)
- [SettingsApp.tsx:121-131](file://src/modules/settings/SettingsApp.tsx#L121-L131)

### Settings Synchronization
- Cross-window synchronization is used to propagate TTS/STT changes without reopening the settings window.
- Use broadcastChange for settings that affect runtime behavior.

**Section sources**
- [TtsSettingsPage.tsx:95-111](file://src/modules/settings/pages/TtsSettingsPage.tsx#L95-L111)
- [SttConfigPage.tsx:76-84](file://src/modules/settings/pages/SttConfigPage.tsx#L76-L84)

### Backup and Restore Functionality
- Memory trajectory export enables exporting runtime traces for diagnostics and potential restoration workflows.
- No explicit backup/restore API is exposed in the current settings pages; consider extending tauri commands for broader configuration backups.

**Section sources**
- [MemorySettingsPage.tsx:173-185](file://src/modules/settings/pages/MemorySettingsPage.tsx#L173-L185)

### Configuration Migration Strategies
- For model settings, validate defaults and migrate embedded model names on first load.
- For TTS/STT, maintain backward compatibility by falling back to defaults when persisted values are missing or invalid.
- Use feature flags (e.g., Memory Control Plane V1) to gate new behavior and allow gradual rollout.

**Section sources**
- [ModelSettingsPage.tsx:263-271](file://src/modules/settings/pages/ModelSettingsPage.tsx#L263-L271)
- [TtsSettingsPage.tsx:153-156](file://src/modules/settings/pages/TtsSettingsPage.tsx#L153-L156)
- [MemorySettingsPage.tsx:362-386](file://src/modules/settings/pages/MemorySettingsPage.tsx#L362-L386)